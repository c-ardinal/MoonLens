//! Tauri command handlers exposed to the Svelte frontend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::tr;
use crate::{
    app::AppState,
    capture::{self, CaptureRect},
    clipboard, hotkey, intent,
    lens::{self, geometry::LensInsets, LensDescriptor},
    lens_fit, lens_text, overlay,
    pipeline::{CaptureFlowResult, TaskResult, LOCAL_ASSIST_ID, LOCAL_LLM_ID},
    plugins::PluginDescriptor,
    providers::{
        asr_local::LOCAL_ASR_ID, llm_models, model_store, ocr_ocrs, whisper_models, Capability,
        TaskInput,
    },
    region_watcher::ReadMode,
    settings::schema::Settings,
    user_error,
};

// Individual Tauri commands. App wiring registers them via
// `tauri::generate_handler![...]`; keep that list in sync when adding a
// new command below.

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok((*state.settings.snapshot()).clone())
}

#[tauri::command]
pub async fn set_settings(
    new: Settings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.settings.save(new).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    // Hotkey bindings might have changed; re-register atomically.
    hotkey::reapply(&app).map_err(|e| e.to_string())?;
    // Windows learn about the change from the settings subscriber in app.rs
    // (moonlens://settings-changed).
    Ok(())
}

/// The bundled defaults (what "初期値に復元" restores).
#[tauri::command]
pub async fn get_default_settings() -> Result<Settings, String> {
    Ok(crate::settings::store::defaults())
}

#[tauri::command]
pub async fn get_settings_schema() -> Result<serde_json::Value, String> {
    Ok(crate::settings::SettingsStore::json_schema())
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<PluginDescriptor>, String> {
    Ok(state.plugins.list())
}

#[tauri::command]
pub async fn set_plugin_enabled(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .plugins
        .set_enabled(&id, enabled)
        .map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(())
}

#[derive(Deserialize)]
pub struct TranslateArgs {
    pub text: String,
    pub source_language: String,
    pub target_language: String,
    /// Earlier `(source, translation)` pairs of the same document (the
    /// text translation page sends the lines already translated), read
    /// by translators that take [`crate::providers::TranslateContext`].
    #[serde(default)]
    pub history: Vec<(String, String)>,
}

#[tauri::command]
pub async fn run_translate(
    args: TranslateArgs,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: args.text,
                source_language: args.source_language,
                target_language: args.target_language,
                context: translate_context(args.history),
            },
        )
        .await
        .map_err(|e| user_error::provider(&e, Capability::Translate))
}

/// History for one request, cut to the same budget the capture flow uses
/// so a long document cannot overflow the model's context window.
fn translate_context(mut history: Vec<(String, String)>) -> crate::providers::TranslateContext {
    crate::translate_plan::trim_history(&mut history, crate::translate_plan::HISTORY);
    crate::providers::TranslateContext {
        history,
        reference: None,
        app: None,
    }
}

#[derive(Deserialize)]
pub struct RecordTranslationArgs {
    pub source_text: String,
    pub result_text: String,
    pub provider: Option<String>,
    #[serde(default)]
    pub via_cloud: bool,
}

/// Save one text-translation page result to history (honours the
/// keep-history switch and retention settings).
#[tauri::command]
pub async fn record_translation(
    args: RecordTranslationArgs,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let source = args.source_text.trim();
    let result = args.result_text.trim();
    if source.is_empty() || result.is_empty() {
        return Ok(());
    }
    state.pipeline.record_history(
        crate::pipeline::history_kind::TEXT,
        Some(source),
        Some(result),
        args.provider.as_deref(),
        args.via_cloud,
    );
    Ok(())
}

#[tauri::command]
pub async fn run_ocr(
    image_png: Vec<u8>,
    languages: Vec<String>,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::Ocr,
            TaskInput::Ocr {
                image_png,
                languages,
            },
        )
        .await
        .map_err(|e| user_error::provider(&e, Capability::Ocr))
}

#[tauri::command]
pub async fn run_intent(image_png: Vec<u8>) -> Result<String, String> {
    let img = image::load_from_memory(&image_png).map_err(|e| e.to_string())?;
    Ok(format!("{:?}", intent::classify(&img)))
}

#[tauri::command]
pub async fn run_qr(image_png: Vec<u8>, state: State<'_, AppState>) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::QrHandler,
            TaskInput::IntentClassifier { image_png },
        )
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct ExplainArgs {
    pub prompt: String,
    pub image_png: Option<Vec<u8>>,
}

#[tauri::command]
pub async fn run_explain(
    args: ExplainArgs,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::Explain,
            TaskInput::Explain {
                prompt: args.prompt,
                image_png: args.image_png,
            },
        )
        .await
        .map_err(|e| user_error::provider(&e, Capability::Explain))
}

#[tauri::command]
pub async fn run_reverse_image_search(
    image_url: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    let result = state
        .pipeline
        .run(
            Capability::ReverseImageSearch,
            TaskInput::ReverseImageSearch { image_url },
        )
        .await
        .map_err(|e| e.to_string())?;
    // The provider returns the URL that should be visited — actually
    // open it in the default browser so the user sees results instead
    // of just a raw string in the overlay.
    if let crate::providers::TaskOutput::OpenedExternalUrl(url) = &result.output {
        tauri_plugin_opener::OpenerExt::opener(&app)
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())?;
    }
    Ok(result)
}

#[derive(Deserialize, Clone)]
pub struct RegionArgs {
    pub monitor_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize)]
pub struct CapturedRegion {
    pub image_png: Vec<u8>,
}

#[tauri::command]
pub async fn capture_region_cmd(
    args: RegionArgs,
    _app: AppHandle,
) -> Result<CapturedRegion, String> {
    let img = tokio::task::spawn_blocking(move || {
        capture::capture_rect(CaptureRect {
            monitor_index: args.monitor_index,
            x: args.x,
            y: args.y,
            w: args.w,
            h: args.h,
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let image_png = capture::png_bytes(&img).map_err(|e| e.to_string())?;
    Ok(CapturedRegion { image_png })
}

#[tauri::command]
pub async fn capture_and_process(
    args: Option<RegionArgs>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureFlowResult, String> {
    overlay::emit(
        &app,
        overlay::OverlayEvent::Processing {
            message: tr!("ipc:capturing"),
        },
    )
    .ok();

    let args_for_capture = args.clone();
    let png = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let img = match args_for_capture {
            Some(a) => capture::capture_rect(CaptureRect {
                monitor_index: a.monitor_index,
                x: a.x,
                y: a.y,
                w: a.w,
                h: a.h,
            })?,
            None => capture::capture_primary_full()?,
        };
        capture::png_bytes(&img)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let mut bundle =
        capture_flow(&state.pipeline, png, crate::pipeline::history_kind::CAPTURE).await?;
    // Record the rect we actually captured so the overlay can pin the
    // region watcher onto it. Full-monitor captures resolve to the
    // primary monitor's bounds.
    bundle.source_rect = Some(match args {
        Some(a) => crate::pipeline::CaptureFlowRect {
            monitor_index: a.monitor_index,
            x: a.x,
            y: a.y,
            w: a.w,
            h: a.h,
        },
        None => {
            let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
            let m = monitors.first().ok_or("no monitor")?;
            crate::pipeline::CaptureFlowRect {
                monitor_index: 0,
                x: 0,
                y: 0,
                w: m.width().map_err(|e| e.to_string())?,
                h: m.height().map_err(|e| e.to_string())?,
            }
        }
    });

    apply_clipboard_policy(&app, &state, &bundle);

    overlay::show_centered(&app).ok();
    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            intent: Some(bundle.intent),
            result: bundle.result.clone(),
            source_rect: bundle.source_rect.clone(),
        },
    )
    .ok();
    Ok(bundle)
}

/// Honour the clipboard policy: translation for text captures, QR
/// payload for QR captures. Source text is only copied when the user
/// opted in (default off). All of this is no-op safe when the user
/// disabled the relevant setting.
fn apply_clipboard_policy(app: &AppHandle, state: &AppState, bundle: &CaptureFlowResult) {
    let clipboard_cfg = state.settings.snapshot().clipboard.clone();
    if let Some(src) = &bundle.source_text {
        clipboard::auto_copy(app, clipboard::Kind::Source, src, &clipboard_cfg);
    }
    match &bundle.result.output {
        crate::providers::TaskOutput::Text(t) => {
            let kind = match bundle.intent {
                crate::providers::Intent::Qr => clipboard::Kind::Qr,
                _ => clipboard::Kind::Translation,
            };
            clipboard::auto_copy(app, kind, t, &clipboard_cfg);
        }
        crate::providers::TaskOutput::Ocr { lines } => {
            let joined = lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            clipboard::auto_copy(app, clipboard::Kind::Source, &joined, &clipboard_cfg);
        }
        _ => {}
    }
}

/// [`crate::pipeline::Pipeline::run_capture_flow`] with errors worded for
/// the step that failed (OCR vs translation).
async fn capture_flow(
    pipeline: &crate::pipeline::Pipeline,
    png: Vec<u8>,
    history_kind: &str,
) -> Result<CaptureFlowResult, String> {
    let mut bundle = pipeline
        .analyze_capture(png)
        .await
        .map_err(|e| user_error::provider(&e, Capability::Ocr))?;
    if bundle.needs_translation() {
        pipeline
            .translate_capture_recorded_as(&mut bundle, history_kind)
            .await
            .map_err(|e| user_error::provider(&e, Capability::Translate))?;
    }
    Ok(bundle)
}

/// Events of one "translate an image file" run, sent to the window that
/// asked (`moonlens://image-analysis` / `moonlens://image-progress`).
/// `run` echoes the caller's id so a page can drop events of a run it
/// has abandoned.
#[derive(serde::Serialize, Clone)]
struct ImageRunEvent<T: serde::Serialize> {
    run: u32,
    #[serde(flatten)]
    payload: T,
}

/// Translate an image file (PNG / JPEG / WebP) the user opened, pasted
/// or dropped on the image translation page. The raw file bytes are the
/// request body; headers carry `run` (caller's id, echoed in events)
/// and optionally `target` (target language, default: settings).
///
/// Emits the positioned, untranslated blocks right after OCR and a
/// progress event before each translation request,
/// then returns the final bundle. Nothing is copied to the clipboard.
#[tauri::command]
pub async fn translate_image(
    request: tauri::ipc::Request<'_>,
    webview: tauri::Webview,
    state: State<'_, AppState>,
) -> Result<CaptureFlowResult, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(user_error::coded("error", tr!("ipc:no_image_data")));
    };
    let header = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let run: u32 = header("run").and_then(|v| v.parse().ok()).unwrap_or(0);
    let target = header("target")
        .unwrap_or_else(|| state.settings.snapshot().subtitle.target_language.clone());
    if bytes.len() > capture::MAX_IMAGE_FILE_BYTES {
        return Err(user_error::coded("error", image_too_large()));
    }
    let bytes = bytes.clone();
    let png = tokio::task::spawn_blocking(move || capture::image_file_png(&bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| user_error::coded("error", tr!("ipc:could_not_read_the_image_png_jpeg")))?;
    let mut bundle = state
        .pipeline
        .analyze_capture(png)
        .await
        .map_err(|e| user_error::provider(&e, Capability::Ocr))?;
    if bundle.needs_translation() {
        let label = webview.label().to_string();
        state.pipeline.prepare_translation(&mut bundle, &target);
        webview
            .emit_to(
                label.as_str(),
                "moonlens://image-analysis",
                ImageRunEvent {
                    run,
                    payload: &bundle,
                },
            )
            .ok();
        let emitter = webview.clone();
        let progress = move |p: &crate::pipeline::TranslationProgress| {
            emitter
                .emit_to(
                    label.as_str(),
                    "moonlens://image-progress",
                    ImageRunEvent { run, payload: p },
                )
                .ok();
        };
        state
            .pipeline
            .translate_capture_as(
                &mut bundle,
                &target,
                Some(&progress),
                crate::pipeline::history_kind::IMAGE,
            )
            .await
            .map_err(|e| user_error::provider(&e, Capability::Translate))?;
    }
    Ok(bundle)
}

/// Raw bytes of an image file dropped on the image translation page
/// (the webview gets only its path). Limited to image extensions and
/// [`capture::MAX_IMAGE_FILE_BYTES`].
#[tauri::command]
pub async fn read_image_file(path: String) -> Result<tauri::ipc::Response, String> {
    let p = std::path::PathBuf::from(&path);
    if !is_image_path(&p) {
        return Err(user_error::coded(
            "error",
            tr!("common:unsupported_format_png_jpeg_and_webp_only"),
        ));
    }
    let len = std::fs::metadata(&p)
        .map_err(|e| user_error::coded("error", tr!("ipc:could_not_read_the_file_e", e = e)))?
        .len();
    if len as usize > capture::MAX_IMAGE_FILE_BYTES {
        return Err(user_error::coded("error", image_too_large()));
    }
    let bytes = tokio::fs::read(&p)
        .await
        .map_err(|e| user_error::coded("error", tr!("ipc:could_not_read_the_file_e", e = e)))?;
    Ok(tauri::ipc::Response::new(bytes))
}

fn is_image_path(p: &std::path::Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
        .unwrap_or(false)
}

#[derive(serde::Serialize, Clone)]
pub struct ModelProgress {
    pub filename: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    /// 0.0..=1.0 when the server advertised Content-Length, else null.
    /// Pre-computed so the UI does not have to guard against division
    /// by zero in a template.
    pub fraction: Option<f32>,
}

/// Reports download progress as `moonlens://ocr-model-progress`.
fn ocr_progress(app: &AppHandle) -> Box<ocr_ocrs::ProgressCb> {
    let app_for_cb = app.clone();
    Box::new(move |filename, downloaded, total| {
        let fraction = total.and_then(|t| {
            if t == 0 {
                None
            } else {
                Some((downloaded as f32 / t as f32).clamp(0.0, 1.0))
            }
        });
        let payload = ModelProgress {
            filename: filename.to_string(),
            downloaded,
            total,
            fraction,
        };
        let _ = app_for_cb.emit("moonlens://ocr-model-progress", payload);
    })
}

#[tauri::command]
pub async fn install_ocr_models(app: AppHandle) -> Result<String, String> {
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    let cb = ocr_progress(&app);
    // PP-OCR (Japanese / Chinese / English) first: it is the default
    // OCR engine; ocrs stays as the Latin-only fallback.
    #[cfg(feature = "ocrs-engine")]
    crate::providers::ocr_ppocr::install_models_with_progress(&dir, Some(&*cb))
        .await
        .map_err(|e| user_error::download(&e))?;
    ocr_ocrs::install_models_with_progress(&dir, Some(&*cb))
        .await
        .map_err(|e| user_error::download(&e))?;
    // Last: OCR already works without the faster runtime, so a failure
    // here leaves a working install that reports what is missing.
    #[cfg(feature = "ocr-onnx")]
    crate::providers::ocr_onnx::install(&dir, Some(&*cb))
        .await
        .map_err(|e| user_error::download(&e))?;
    Ok(dir.display().to_string())
}

/// Whether the OCR models are on disk, for the setup / settings screens.
#[derive(serde::Serialize)]
pub struct OcrModelsStatus {
    /// PP-OCR (Japanese / Chinese / English), the default engine.
    pub ppocr: bool,
    /// The PP-OCR models are the generation this version installs
    /// (false: an earlier version's, which read less well).
    pub current: bool,
    /// ocrs (Latin only), the fallback engine.
    pub ocrs: bool,
    /// The faster runtime for PP-OCR (ONNX Runtime) is installed, or
    /// this platform has none to install.
    pub fast: bool,
    pub dir: String,
}

#[tauri::command]
pub async fn ocr_models_status() -> Result<OcrModelsStatus, String> {
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    Ok(OcrModelsStatus {
        ppocr: crate::providers::ocr_ppocr::models_installed(&dir),
        current: crate::providers::ocr_ppocr::models_current(&dir),
        ocrs: ocr_ocrs::model_paths(&dir).is_some(),
        fast: !crate::providers::ocr_onnx::supported()
            || crate::providers::ocr_onnx::installed(&dir),
        dir: dir.display().to_string(),
    })
}

/// A writing system with a recognition model of its own
/// (`providers::ocr_scripts`), for the model settings.
#[derive(serde::Serialize)]
pub struct OcrScriptStatus {
    pub id: &'static str,
    /// Download size in bytes.
    pub size: u64,
    pub installed: bool,
}

#[tauri::command]
pub async fn ocr_scripts_status() -> Result<Vec<OcrScriptStatus>, String> {
    use crate::providers::ocr_scripts;
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    Ok(ocr_scripts::SCRIPTS
        .iter()
        .map(|s| OcrScriptStatus {
            id: s.id,
            size: s.bytes,
            installed: s.model.present(&dir),
        })
        .collect())
}

/// Download the recognition model of one script. Progress arrives as
/// `moonlens://ocr-model-progress` with the model's file name.
#[tauri::command]
pub async fn install_ocr_script(app: AppHandle, id: String) -> Result<(), String> {
    use crate::providers::ocr_scripts;
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    let script = ocr_scripts::by_id(&id).ok_or_else(|| format!("unknown script {id}"))?;
    let cb = ocr_progress(&app);
    script
        .model
        .install(&dir, Some(&*cb))
        .await
        .map_err(|e| user_error::download(&e))
}

#[tauri::command]
pub async fn delete_ocr_script(id: String) -> Result<(), String> {
    use crate::providers::ocr_scripts;
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    let script = ocr_scripts::by_id(&id).ok_or_else(|| format!("unknown script {id}"))?;
    ocr_scripts::remove(&dir, script).map_err(|e| e.to_string())
}

/// Local LLM models the app can download, with install state, plus the
/// one currently selected for the built-in engine.
#[derive(serde::Serialize)]
pub struct LlmModels {
    /// Model used for translation.
    pub selected: String,
    /// Model used for explain / summarize.
    pub assistant: String,
    pub dir: String,
    pub models: Vec<llm_models::ModelStatus>,
    /// Licence ids the user already accepted (see `CatalogModel::consent`).
    pub accepted_licenses: Vec<String>,
}

fn llm_dir() -> Result<PathBuf, String> {
    llm_models::default_models_dir().ok_or_else(|| "no data dir".to_string())
}

fn selected_llm(settings: &Settings, provider_id: &str) -> String {
    settings
        .providers
        .configs
        .get(provider_id)
        .and_then(|c| c.get("model"))
        .and_then(|v| v.as_str())
        .filter(|id| llm_models::find(id).is_some())
        .unwrap_or(crate::providers::translate_llama::default_model_for(
            provider_id,
        ))
        .to_string()
}

#[tauri::command]
pub async fn list_llm_models(state: State<'_, AppState>) -> Result<LlmModels, String> {
    let dir = llm_dir()?;
    let s = state.settings.snapshot();
    Ok(LlmModels {
        selected: selected_llm(&s, LOCAL_LLM_ID),
        assistant: selected_llm(&s, LOCAL_ASSIST_ID),
        dir: dir.display().to_string(),
        models: llm_models::status(&dir),
        accepted_licenses: s.onboarding.accepted_licenses.clone(),
    })
}

/// Record that the user accepted licence `id` (a `CatalogModel::consent`
/// id). Idempotent; persisted so the prompt never repeats.
#[tauri::command]
pub async fn accept_license(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut s = (*state.settings.snapshot()).clone();
    if llm_models::accept_license(&mut s.onboarding.accepted_licenses, &id)? {
        state.settings.save(s).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(serde::Serialize, Clone)]
pub struct LlmProgress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
    pub fraction: f32,
}

/// Download (or resume) a catalog model. Progress is emitted as
/// `moonlens://llm-model-progress`. Loads it in the background when it
/// is the selected model.
#[tauri::command]
pub async fn install_llm_model(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if let Some(m) = llm_models::find(&id) {
        let accepted = state
            .settings
            .snapshot()
            .onboarding
            .accepted_licenses
            .clone();
        if llm_models::require_consent(m, &accepted).is_err() {
            return Err(user_error::coded(
                "license_consent",
                crate::tr!("ipc:you_need_to_agree_to_the_licence"),
            ));
        }
    }
    let dir = llm_dir()?;
    let app_for_cb = app.clone();
    let cb: Box<llm_models::ProgressCb> = Box::new(move |id, downloaded, total| {
        let fraction = if total == 0 {
            0.0
        } else {
            (downloaded as f32 / total as f32).clamp(0.0, 1.0)
        };
        let _ = app_for_cb.emit(
            "moonlens://llm-model-progress",
            LlmProgress {
                id: id.to_string(),
                downloaded,
                total,
                fraction,
            },
        );
    });
    if let Some(m) = llm_models::find(&id) {
        let need = llm_models::bytes_still_needed(&dir, m);
        if let Some(free) = llm_models::free_space(&dir) {
            // Keep a margin so the download does not fill the disk to
            // the last byte.
            if free < need + llm_models::FREE_SPACE_MARGIN {
                return Err(user_error::coded(
                    "disk_full",
                    not_enough_space(need + llm_models::FREE_SPACE_MARGIN, free),
                ));
            }
        }
    }
    let path = llm_models::install(&dir, &id, Some(&*cb))
        .await
        .map_err(|e| {
            if e.to_string() == "cancelled" {
                "cancelled".to_string()
            } else {
                user_error::download(&e)
            }
        })?;
    if selected_llm(&state.settings.snapshot(), LOCAL_LLM_ID) == id {
        state.pipeline.preload_local_llm();
    }
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn cancel_llm_download(id: String) -> Result<bool, String> {
    Ok(llm_models::cancel(&id))
}

#[tauri::command]
pub async fn delete_llm_model(id: String) -> Result<(), String> {
    let dir = llm_dir()?;
    tokio::task::spawn_blocking(move || llm_models::remove(&dir, &id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Use catalog model `id` for `role` ("translate", the default, or
/// "assistant" for explain / summarize). Persisted to config.toml.
#[tauri::command]
pub async fn select_llm_model(
    id: String,
    role: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let m = llm_models::find(&id).ok_or_else(|| format!("unknown model {id}"))?;
    let provider_id = match role.as_deref() {
        None | Some("translate") => LOCAL_LLM_ID,
        Some("assistant") => {
            if m.style != llm_models::PromptStyle::ChatTemplate {
                return Err(format!("{} can only translate", m.label));
            }
            LOCAL_ASSIST_ID
        }
        Some(other) => return Err(format!("unknown role {other}")),
    };
    let mut next = (*state.settings.snapshot()).clone();
    let cfg = next
        .providers
        .configs
        .entry(provider_id.to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !cfg.is_object() {
        *cfg = serde_json::json!({});
    }
    cfg["model"] = serde_json::Value::String(id);
    if let Some(o) = cfg.as_object_mut() {
        // A catalog choice replaces a hand-set custom file.
        o.remove("model_path");
    }
    state.settings.save(next).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    state.pipeline.preload_local_llm();
    Ok(())
}

/// Speech recognition models for live subtitles, with install state,
/// plus the one the built-in engine uses.
#[derive(serde::Serialize)]
pub struct AsrModels {
    pub selected: String,
    pub dir: String,
    pub models: Vec<whisper_models::AsrModelStatus>,
}

fn asr_dir() -> Result<PathBuf, String> {
    whisper_models::default_models_dir().ok_or_else(|| "no data dir".to_string())
}

fn asr_config(settings: &Settings) -> serde_json::Value {
    settings
        .providers
        .configs
        .get(LOCAL_ASR_ID)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

#[tauri::command]
pub async fn list_asr_models(state: State<'_, AppState>) -> Result<AsrModels, String> {
    let dir = asr_dir()?;
    let s = state.settings.snapshot();
    Ok(AsrModels {
        selected: whisper_models::selected(&asr_config(&s)).id.to_string(),
        dir: dir.display().to_string(),
        models: whisper_models::status(&dir),
    })
}

/// Download (or resume) a speech model. Progress is emitted as
/// `moonlens://asr-model-progress`.
#[tauri::command]
pub async fn install_asr_model(id: String, app: AppHandle) -> Result<String, String> {
    let dir = asr_dir()?;
    let m = whisper_models::find(&id).ok_or_else(|| format!("unknown model {id}"))?;
    let need = model_store::bytes_still_needed(&dir, &m.pinned());
    if let Some(free) = model_store::free_space(&dir) {
        if free < need + model_store::FREE_SPACE_MARGIN {
            return Err(user_error::coded(
                "disk_full",
                not_enough_space(need + model_store::FREE_SPACE_MARGIN, free),
            ));
        }
    }
    let cb: Box<model_store::ProgressCb> = Box::new(move |id, downloaded, total| {
        let fraction = if total == 0 {
            0.0
        } else {
            (downloaded as f32 / total as f32).clamp(0.0, 1.0)
        };
        let _ = app.emit(
            "moonlens://asr-model-progress",
            LlmProgress {
                id: id.to_string(),
                downloaded,
                total,
                fraction,
            },
        );
    });
    let path = whisper_models::install(&dir, &id, Some(&*cb))
        .await
        .map_err(|e| {
            if e.to_string() == "cancelled" {
                "cancelled".to_string()
            } else {
                user_error::download(&e)
            }
        })?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn cancel_asr_download(id: String) -> Result<bool, String> {
    Ok(model_store::cancel(&id))
}

#[tauri::command]
pub async fn delete_asr_model(id: String) -> Result<(), String> {
    let dir = asr_dir()?;
    tokio::task::spawn_blocking(move || whisper_models::remove(&dir, &id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Use speech model `id` for live subtitles. Persisted to config.toml;
/// takes effect from the next chunk.
#[tauri::command]
pub async fn select_asr_model(id: String, state: State<'_, AppState>) -> Result<(), String> {
    whisper_models::find(&id).ok_or_else(|| format!("unknown model {id}"))?;
    let mut next = (*state.settings.snapshot()).clone();
    let cfg = next
        .providers
        .configs
        .entry(LOCAL_ASR_ID.to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !cfg.is_object() {
        *cfg = serde_json::json!({});
    }
    cfg["model"] = serde_json::Value::String(id);
    state.settings.save(next).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(())
}

/// Pick a `.moonlens-plugin` archive (or a plugin directory) with the
/// platform's native file picker and install it. Returns the new plugin
/// root on success; returns Ok(None) when the user cancels the dialog.
/// Query each plugin's `update_url` concurrently and return the subset
/// that advertises a newer version than installed.
#[tauri::command]
pub async fn check_plugin_updates(
    state: State<'_, AppState>,
) -> Result<Vec<crate::plugins::updates::UpdateAvailable>, String> {
    let manifests = state.plugins.manifests();
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let futures = manifests.iter().filter_map(|m| {
        m.plugin.update_url.as_ref().map(|_| {
            let m = m.clone();
            let http = http.clone();
            async move { crate::plugins::updates::check_one(&http, &m).await }
        })
    });
    let results = futures::future::join_all(futures).await;
    let mut updates = Vec::new();
    for res in results {
        match res {
            Ok(Some(u)) => updates.push(u),
            Ok(None) => {}
            Err(e) => tracing::warn!("update check failed: {e}"),
        }
    }
    Ok(updates)
}

#[tauri::command]
pub async fn pick_and_install_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .add_filter("MoonLens Plugin", &["moonlens-plugin", "zip"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(file) = chosen else { return Ok(None) };
    let path = file
        .into_path()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let installed = install_plugin_from_path(path, app, state).await?;
    Ok(Some(installed))
}

#[tauri::command]
pub async fn install_plugin_from_path(
    archive_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("plugins");
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let src = PathBuf::from(&archive_path);
    let target = if src.is_dir() {
        let name = src
            .file_name()
            .ok_or("invalid archive path")?
            .to_string_lossy()
            .to_string();
        let target = dest.join(&name);
        copy_dir(&src, &target).map_err(|e| e.to_string())?;
        target
    } else if src.is_file() {
        crate::plugins::installer::extract_zip(&src, &dest).map_err(|e| e.to_string())?
    } else {
        return Err("archive_path is neither a file nor a directory".into());
    };
    state.plugins.rescan().map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(target.display().to_string())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn show_main(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_overlay(app: AppHandle) -> Result<(), String> {
    overlay::hide(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_history(
    limit: Option<usize>,
    query: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::HistoryEntry>, String> {
    let cap = limit.unwrap_or(200);
    // Search the whole history, not just the newest `cap` rows.
    match query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => state.storage.recent(cap),
        Some(q) => state.storage.search(q, cap),
    }
    .map_err(|e| e.to_string())
}

/// 個別削除の結果。`record` は音声翻訳の記録（削除時点の全行）：取消時に
/// `restore_history_entry` へ引渡し。
#[derive(Serialize)]
pub struct DeletedHistoryEntry {
    /// 削除行数（0：該当 id なし）。
    pub deleted: usize,
    pub record: Option<crate::storage::SubtitleRecord>,
}

#[tauri::command]
pub async fn delete_history_entry(
    id: i64,
    state: State<'_, AppState>,
) -> Result<DeletedHistoryEntry, String> {
    let (deleted, record) = state
        .storage
        .delete_keeping_record(id)
        .map_err(|e| e.to_string())?;
    Ok(DeletedHistoryEntry { deleted, record })
}

/// 個別削除の取消：削除済みの行を元の id・日時で復元。
#[tauri::command]
pub async fn restore_history_entry(
    entry: crate::storage::HistoryEntry,
    record: Option<crate::storage::SubtitleRecord>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    state
        .storage
        .restore_with_record(&entry, record.as_ref())
        .map_err(|e| e.to_string())
}

/// 音声翻訳の履歴（時刻付き行）。記録なしの行は null。
#[tauri::command]
pub async fn get_subtitle_record(
    id: i64,
    state: State<'_, AppState>,
) -> Result<Option<crate::storage::SubtitleRecord>, String> {
    state.storage.subtitle_record(id).map_err(|e| e.to_string())
}

/// 記録の全行を置換（再翻訳とその取消）。`revision` は行の取得時点の値：
/// 取得後に記録が変化した場合は失敗（記録中の行を保護）。戻り値は新しい値。
#[tauri::command]
pub async fn replace_subtitle_lines(
    id: i64,
    revision: i64,
    lines: Vec<crate::storage::SubtitleLine>,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    state
        .storage
        .replace_subtitle_lines(id, revision, &lines)
        .map_err(|e| format!("{e:#}"))
}

/// 記録を SRT・テキストで保存。保存先ダイアログの取消は `None`。
#[tauri::command]
pub async fn export_subtitle_record(
    id: i64,
    format: crate::subtitle_record::ExportFormat,
    content: crate::subtitle_record::ExportContent,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let record = state
        .storage
        .subtitle_record(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::tr!("ipc:subtitle_record_not_found"))?;
    let created_at = state
        .storage
        .entry(id)
        .map_err(|e| e.to_string())?
        .map_or(0, |e| e.created_at);
    let text = crate::subtitle_record::render(&record, format, content);
    let name = crate::subtitle_record::export_file_name(created_at, format);
    let label = match format {
        crate::subtitle_record::ExportFormat::Srt => crate::tr!("ipc:srt_subtitles"),
        crate::subtitle_record::ExportFormat::Txt => crate::tr!("common:text"),
    };
    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .set_file_name(name)
            .add_filter(label, &[format.extension()])
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(file) = chosen else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// Extensions offered by the file dialog of file subtitles. Other
/// files can still be dropped: the content decides, not the name.
const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "webm", "mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "aiff",
    "aif", "caf",
];

/// 字幕生成の対象ファイルを選択。取消は `None`。
#[tauri::command]
pub async fn pick_media_file(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().clone();
    let label = crate::tr!("file-subtitle:video_and_audio_files");
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .add_filter(label, MEDIA_EXTENSIONS)
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(file) = chosen else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// ファイルの字幕生成を開始。進行は `moonlens://file-subtitle` で通知。
#[tauri::command]
pub async fn start_file_subtitle(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::file_subtitle::Status, String> {
    state.subtitle.preflight().await?;
    let on_update = app.clone();
    let notify = std::sync::Arc::new(move |update: crate::file_subtitle::Update| {
        let app = &on_update;
        if update.status.phase != crate::file_subtitle::Phase::Running {
            release_speech_model(app);
        }
        let _ = app.emit(crate::file_subtitle::EVENT, update);
    });
    let started = state
        .file_subtitle
        .start(std::path::Path::new(&path), notify);
    // A file that cannot be used ends no job: the model the preflight
    // loaded goes here, unless a job that was already running holds it.
    if started.is_err()
        && state.file_subtitle.status().phase != crate::file_subtitle::Phase::Running
    {
        release_speech_model(&app);
    }
    started
}

/// The file job ended: give back the ~1 GB the speech model holds,
/// unless live subtitles are recognizing with it. Off this thread: it
/// waits for an aborted decode to wind down.
fn release_speech_model(app: &AppHandle) {
    if !app.state::<AppState>().subtitle.is_running() {
        std::thread::spawn(crate::providers::asr_local::release);
    }
}

#[tauri::command]
pub async fn cancel_file_subtitle(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::file_subtitle::Status, String> {
    let status = state.file_subtitle.cancel();
    release_speech_model(&app);
    Ok(status)
}

#[tauri::command]
pub async fn file_subtitle_status(
    state: State<'_, AppState>,
) -> Result<crate::file_subtitle::Status, String> {
    Ok(state.file_subtitle.status())
}

/// 生成済みの字幕を SRT・テキストで保存（中止・失敗時は途中まで）。
/// 保存先ダイアログの取消は `None`。
#[tauri::command]
pub async fn export_file_subtitle(
    format: crate::subtitle_record::ExportFormat,
    content: crate::subtitle_record::ExportContent,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let export = state.file_subtitle.export(format, content);
    let label = match format {
        crate::subtitle_record::ExportFormat::Srt => crate::tr!("ipc:srt_subtitles"),
        crate::subtitle_record::ExportFormat::Txt => crate::tr!("common:text"),
    };
    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        let mut dialog = dialog
            .file()
            .set_file_name(export.name)
            .add_filter(label, &[format.extension()]);
        if let Some(folder) = export.folder {
            dialog = dialog.set_directory(folder);
        }
        dialog.blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(file) = chosen else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, export.text).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<usize, String> {
    state.storage.clear_all().map_err(|e| e.to_string())
}

/// Size of the translation cache on disk, in total and per app.
#[tauri::command]
pub async fn translation_cache_usage(
    state: State<'_, AppState>,
) -> Result<crate::translation_cache::Usage, String> {
    let pipeline = state.pipeline.clone();
    tokio::task::spawn_blocking(move || pipeline.translation_cache_usage())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Delete cached translations: those of one app (`""` = text that
/// belongs to no app), or all of them when `app` is absent. Returns the
/// number of entries removed.
#[tauri::command]
pub async fn clear_translation_cache(
    app: Option<String>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let pipeline = state.pipeline.clone();
    tokio::task::spawn_blocking(move || pipeline.clear_translation_cache(app.as_deref()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    if state.subtitle.has_windows() {
        state.subtitle.stop(&app);
        return Ok(false);
    }
    start_subtitle(app, state).await.map(|_| true)
}

#[tauri::command]
pub async fn subtitle_state(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.subtitle.is_running())
}

/// Block `phrase` in live subtitles from now on: the user approved a
/// suggestion in the subtitle window. Persisted to config.toml
/// (`subtitle.blocked_phrases`); takes effect from the next chunk.
#[tauri::command]
pub async fn block_subtitle_phrase(
    phrase: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut next = (*state.settings.snapshot()).clone();
    if crate::subtitle_filter::add_phrase(&mut next.subtitle.blocked_phrases, &phrase) {
        state.settings.save(next).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Stop and close every subtitle window.
#[tauri::command]
pub async fn stop_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.subtitle.stop(&app);
    Ok(())
}

#[tauri::command]
pub async fn start_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // The speech recognition engine is checked before a subtitle window
    // opens, instead of failing every few seconds once it is open.
    state.subtitle.start(&app).await?;
    crate::notices::clear("subtitle_unavailable");
    Ok(())
}

/// Start a face-to-face conversation (microphone, translated in both
/// directions) in a window of its own.
#[tauri::command]
pub async fn start_conversation(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let (subtitle, handle) = (state.subtitle.clone(), app.clone());
    // Opening the microphone may wait for the device; off the IPC thread.
    tauri::async_runtime::spawn(async move { subtitle.start_conversation(&handle).await })
        .await
        .map_err(|e| e.to_string())?
}

fn subtitle_failure(e: anyhow::Error) -> String {
    user_error::coded(
        "error",
        tr!(
            "common:could_not_start_audio_capture_e",
            e = format!("{:#}", e)
        ),
    )
}

/// One more subtitle window, opened from the window `window` (its ＋
/// button) and placed next to it. Returns the new window's label.
#[tauri::command]
pub async fn open_subtitle_window(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    source: Option<crate::audio::AudioSource>,
) -> Result<String, String> {
    state.subtitle.preflight().await?;
    let beside = crate::subtitle::is_window(window.label()).then(|| window.label());
    state
        .subtitle
        .open(&app, source.unwrap_or_default(), beside)
        .map_err(subtitle_failure)
}

/// Stop and close the calling subtitle window.
#[tauri::command]
pub async fn close_subtitle_window(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.subtitle.close(&app, window.label());
    Ok(())
}

/// Switch the calling subtitle window to another audio source, or
/// restart its current one after it stopped.
#[tauri::command]
pub async fn set_subtitle_source(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    source: crate::audio::AudioSource,
) -> Result<(), String> {
    state.subtitle.preflight().await?;
    let (subtitle, label) = (state.subtitle.clone(), window.label().to_string());
    // Opening a source waits for the capture thread: off the async
    // runtime.
    tauri::async_runtime::spawn_blocking(move || subtitle.set_source(&app, &label, source))
        .await
        .map_err(|e| e.to_string())?
        .map_err(subtitle_failure)
}

/// Source and state of the calling subtitle window.
#[tauri::command]
pub async fn subtitle_session(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<crate::subtitle::SessionInfo>, String> {
    Ok(state.subtitle.session(window.label()))
}

/// The applications a subtitle window can listen to.
#[tauri::command]
pub async fn list_audio_apps() -> Result<crate::audio_apps::AudioApps, String> {
    tauri::async_runtime::spawn_blocking(crate::audio_apps::list)
        .await
        .map_err(|e| e.to_string())
}

/// Describes which first-run guidance the UI should show. Exposed as a
/// single struct so the frontend can decide in one round-trip.
#[derive(serde::Serialize)]
pub struct OnboardingStatus {
    pub platform: &'static str,
    pub needs_macos_permissions_review: bool,
    /// First run: show the setup wizard (hardware check, recommended
    /// models, bulk download).
    pub needs_setup: bool,
    /// Live per-permission snapshot. The modal renders each status as
    /// a badge next to its System Settings link.
    pub macos_permissions: crate::macos_permissions::PermissionReport,
}

#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, AppState>) -> Result<OnboardingStatus, String> {
    let snapshot = state.settings.snapshot();
    let platform = std::env::consts::OS;
    let perms = crate::macos_permissions::snapshot();
    // Surface the modal only when we are on macOS AND the user has
    // not dismissed it AND at least one detectable permission is
    // still denied. Fully-granted sessions skip onboarding entirely.
    let needs_macos_permissions_review = cfg!(target_os = "macos")
        && !snapshot.onboarding.macos_permissions_reviewed
        && (matches!(
            perms.screen_capture,
            crate::macos_permissions::PermissionStatus::Denied
        ) || matches!(
            perms.accessibility,
            crate::macos_permissions::PermissionStatus::Denied
        ));
    // Users who already prepared the translation and OCR models (before
    // the wizard existed) are not sent through it.
    let needs_setup = !snapshot.onboarding.setup_completed && !core_models_ready(&snapshot);
    Ok(OnboardingStatus {
        platform: platform_label(platform),
        needs_macos_permissions_review,
        needs_setup,
        macos_permissions: perms,
    })
}

/// Translation model and PP-OCR on disk: MoonLens can translate.
fn core_models_ready(settings: &Settings) -> bool {
    // Resolve the path the engine would load (honours a custom
    // `model_path` / `models_dir`), not only the default catalog folder.
    let cfg = settings
        .providers
        .configs
        .get(LOCAL_LLM_ID)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let llm = crate::providers::translate_llama::LlamaTranslator::new(LOCAL_LLM_ID, &cfg, false)
        .model_path()
        .is_file();
    let ocr = ocr_ocrs::default_models_dir()
        .is_some_and(|d| crate::providers::ocr_ppocr::models_installed(&d));
    llm && ocr
}

/// The PC's hardware and the model each feature should use on it, for
/// the setup wizard and the recommended marks in the model settings.
#[derive(serde::Serialize)]
pub struct SystemReport {
    pub profile: crate::setup::SystemProfile,
    /// The GPU inference runs on (None: CPU).
    pub accelerator: Option<crate::gpu::GpuDevice>,
    pub recommended: crate::setup::Recommendation,
}

#[tauri::command]
pub async fn get_system_profile(state: State<'_, AppState>) -> Result<SystemReport, String> {
    let use_gpu = state.settings.snapshot().performance.use_gpu_if_available;
    // GPU enumeration starts the Vulkan / Metal driver: keep it off the
    // async runtime.
    let profile = tokio::task::spawn_blocking(move || crate::setup::SystemProfile::detect(use_gpu))
        .await
        .map_err(|e| e.to_string())?;
    let recommended = crate::setup::recommend(&profile);
    Ok(SystemReport {
        accelerator: profile.accelerator().cloned(),
        profile,
        recommended,
    })
}

/// The setup wizard was finished or skipped: do not show it again.
#[tauri::command]
pub async fn complete_setup(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = (*state.settings.snapshot()).clone();
    s.onboarding.setup_completed = true;
    state.settings.save(s).map_err(|e| e.to_string())
}

/// Re-query the live permission snapshot. The onboarding UI calls this
/// after the user visits a System Settings pane so the badges flip
/// green without requiring an app restart.
#[tauri::command]
pub async fn macos_permissions_snapshot(
) -> Result<crate::macos_permissions::PermissionReport, String> {
    Ok(crate::macos_permissions::snapshot())
}

/// Trigger the macOS "Allow MoonLens to record your screen?" prompt.
/// No-op on non-macOS targets; returns the post-prompt status.
#[tauri::command]
pub async fn macos_request_screen_capture(
) -> Result<crate::macos_permissions::PermissionStatus, String> {
    Ok(crate::macos_permissions::request_screen_capture())
}

#[tauri::command]
pub async fn mark_macos_permissions_reviewed(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = (*state.settings.snapshot()).clone();
    s.onboarding.macos_permissions_reviewed = true;
    state.settings.save(s).map_err(|e| e.to_string())
}

/// Open a specific macOS privacy pane via the `x-apple.systempreferences`
/// URL scheme. No-op on other platforms (returns Ok).
#[tauri::command]
pub async fn open_macos_privacy_pane(app: AppHandle, pane: String) -> Result<(), String> {
    let url = match pane.as_str() {
        "screen_capture" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        "accessibility" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        "microphone" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        "input_monitoring" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }
        other => return Err(format!("unknown pane: {other}")),
    };
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn platform_label(os: &str) -> &'static str {
    match os {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        _ => "other",
    }
}

/// Open a window-translation window. With `rect` (monitor-relative
/// physical pixels) its body covers that range; without, it opens
/// under the mouse pointer. `target_language` defaults to the
/// configured target. `beside` (a window id, without `rect`): next to
/// that window, with its size, language and reading mode. `read_mode`
/// applies with `rect` (default: subtitles).
#[tauri::command]
pub async fn open_lens(
    rect: Option<RegionArgs>,
    target_language: Option<String>,
    read_mode: Option<ReadMode>,
    beside: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let placement = rect.map(|r| lens::Placement {
        rect: CaptureRect {
            monitor_index: r.monitor_index,
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        },
        target_language: clean_language(target_language.clone()),
        read_mode: read_mode.unwrap_or_default(),
    });
    // Window creation must not run on the IPC worker on Windows (it
    // deadlocks with the event loop); hand it to the main thread.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let lenses = state.lenses.clone();
    let app_main = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(lens::create(
            &app_main,
            &lenses,
            placement,
            beside.as_deref(),
        ));
    })
    .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Change the language a lens window translates into (`None` / empty:
/// the configured target). A continuous lens translates again at once.
#[tauri::command]
pub async fn set_lens_language(
    id: String,
    target_language: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .lenses
        .set_target_language(&id, clean_language(target_language));
    state.lenses.kick(&id);
    Ok(())
}

/// Change how a lens window reads its range (see [`ReadMode`]). Applies
/// from the next reading; what is shown stays.
#[tauri::command]
pub async fn set_lens_read_mode(
    id: String,
    mode: ReadMode,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_read_mode(&id, mode);
    Ok(())
}

/// A lens window's operate mode as the user set it (for the operate
/// hotkey, which flips all windows).
#[tauri::command]
pub async fn set_lens_operate(
    id: String,
    on: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_operate(&id, on);
    Ok(())
}

/// Whether a lens window's body takes the mouse (operate mode or the
/// text view) instead of letting it through.
#[tauri::command]
pub async fn set_lens_interactive(
    id: String,
    on: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_interactive(&id, on);
    Ok(())
}

/// Show a lens window's text view outside of it, docked at `dock`
/// (`top` / `bottom` / `left` / `right`) or placed freely (`free`);
/// `None` closes the outside view (the text is shown inside the window).
#[tauri::command]
pub async fn set_lens_text_dock(
    id: String,
    dock: Option<lens_text::Dock>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Window creation must run on the main thread (see `open_lens`).
    let (tx, rx) = tokio::sync::oneshot::channel();
    let lenses = state.lenses.clone();
    let app_main = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(lens_text::set_dock(&app_main, &lenses, &id, dock));
    })
    .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

fn clean_language(lang: Option<String>) -> Option<String> {
    lang.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[tauri::command]
pub async fn close_lens_window(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    lens::close(&app, &state.lenses, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_lens_windows(state: State<'_, AppState>) -> Result<Vec<LensDescriptor>, String> {
    Ok(state.lenses.list())
}

#[tauri::command]
pub async fn set_lens_insets(
    id: String,
    top: u32,
    bottom: u32,
    left: u32,
    right: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_insets(
        &id,
        LensInsets {
            top,
            bottom,
            left,
            right,
        },
    );
    Ok(())
}

/// Switch continuous reading of a lens window on or off. While on, the
/// lens window gets `moonlens://lens-live` events (see `lens::start_watch`).
#[tauri::command]
pub async fn set_lens_continuous(
    id: String,
    on: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    lens::set_continuous(
        &app,
        &state.lenses,
        state.pipeline.clone(),
        state.settings.clone(),
        &id,
        on,
    );
    Ok(())
}

/// Pause every lens window, or let them read again. Returns whether
/// they are paused afterwards (false without a window).
#[tauri::command]
pub async fn toggle_lens_pause(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    Ok(lens::toggle_pause(&app, &state.lenses))
}

/// Whether extra triggers (mouse buttons, double press, gamepad) work on
/// this platform.
#[tauri::command]
pub async fn trigger_supported() -> Result<bool, String> {
    Ok(crate::trigger_watch::SUPPORTED)
}

/// Wait for the user to perform a trigger and return its text form
/// (settings screen). `None`: nothing in time, or cancelled.
#[tauri::command]
pub async fn capture_trigger(app: AppHandle) -> Result<Option<String>, String> {
    Ok(crate::trigger_watch::capture(&app)
        .await
        .map(|t| t.to_string()))
}

#[tauri::command]
pub async fn cancel_trigger_capture(app: AppHandle) -> Result<(), String> {
    crate::trigger_watch::cancel_capture(&app);
    Ok(())
}

/// State of the all-window controls, asked by a window as it opens.
#[derive(Debug, Serialize)]
pub struct LensControls {
    pub paused: bool,
    /// The show-original hotkey is held.
    pub original: bool,
}

#[tauri::command]
pub async fn lens_controls(state: State<'_, AppState>) -> Result<LensControls, String> {
    Ok(LensControls {
        paused: state.lenses.paused(),
        original: state.lenses.original_held(),
    })
}

/// Read a continuously reading lens now, even when nothing changed.
/// Returns false when the lens is not reading continuously (the caller
/// then uses `capture_lens_now`).
#[tauri::command]
pub async fn refresh_lens(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.lenses.kick(&id))
}

/// Shrink a lens window to `band`, a rect inside the range it observes
/// (`[x, y, w, h]`, physical pixels from the range's top-left corner).
/// Returns where the window was, for `restore_lens`.
#[tauri::command]
pub async fn fit_lens(
    id: String,
    band: [i32; 4],
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<lens_fit::Before, String> {
    lens_fit::fit(&app, &state.lenses, &id, band).map_err(|e| e.to_string())
}

/// Put a lens window back where it was before `fit_lens`.
#[tauri::command]
pub async fn restore_lens(
    id: String,
    before: lens_fit::Before,
    app: AppHandle,
) -> Result<(), String> {
    lens_fit::restore(&app, &id, before).map_err(|e| e.to_string())
}

/// Capture the observed rect of a lens window and run the full pipeline.
/// Emits overlay events to the regular overlay window so the result can
/// be displayed either inside the lens body (via the lens page) or over
/// it (legacy overlay).
#[tauri::command]
pub async fn capture_lens_now(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureFlowResult, String> {
    let lenses = state.lenses.clone();
    let pipeline = state.pipeline.clone();
    let app_clone = app.clone();
    let id_clone = id.clone();
    let margin = state.settings.snapshot().performance.scan_margin_px;
    let (png, visible) = tokio::task::spawn_blocking(move || {
        lens::capture_lens(&app_clone, &lenses, &id_clone, margin)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let mut bundle = pipeline
        .analyze_capture_in(png, Some(visible))
        .await
        .map_err(|e| user_error::provider(&e, Capability::Ocr))?;
    if bundle.needs_translation() {
        let target = state
            .lenses
            .target_language(&id)
            .unwrap_or_else(|| state.settings.snapshot().subtitle.target_language.clone());
        pipeline
            .translate_capture_as(
                &mut bundle,
                &target,
                None,
                crate::pipeline::history_kind::LENS,
            )
            .await
            .map_err(|e| user_error::provider(&e, Capability::Translate))?;
    }
    // Lens captures must obey the same clipboard policy as the
    // regular ⌘⇧Space flow. Without this the user's auto_copy settings
    // would apply to ad-hoc captures but silently drop lens results.
    let clipboard_cfg = state.settings.snapshot().clipboard.clone();
    if let Some(src) = &bundle.source_text {
        clipboard::auto_copy(&app, clipboard::Kind::Source, src, &clipboard_cfg);
    }
    if let crate::providers::TaskOutput::Text(t) = &bundle.result.output {
        let kind = match bundle.intent {
            crate::providers::Intent::Qr => clipboard::Kind::Qr,
            _ => clipboard::Kind::Translation,
        };
        clipboard::auto_copy(&app, kind, t, &clipboard_cfg);
    }
    // Push the result to the lens-specific event channel so the lens
    // renderer updates in place; also emit the generic overlay event so
    // the floating panel can mirror it if opened.
    let _ = app.emit_to(
        tauri::EventTarget::WebviewWindow { label: id.clone() },
        "moonlens://lens-result",
        bundle.clone(),
    );
    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            intent: Some(bundle.intent),
            result: bundle.result.clone(),
            source_rect: bundle.source_rect.clone(),
        },
    )
    .ok();
    Ok(bundle)
}

/* --------------------------- lifecycle / housekeeping --------------------------- */

#[tauri::command]
pub async fn list_notices() -> Result<Vec<crate::notices::Notice>, String> {
    Ok(crate::notices::list())
}

#[tauri::command]
pub async fn dismiss_notice(id: String) -> Result<(), String> {
    crate::notices::clear(&id);
    Ok(())
}

/// Problems found in config.toml, each with its proposed fix.
#[tauri::command]
pub async fn list_config_issues() -> Result<Vec<crate::settings::validate::Issue>, String> {
    Ok(crate::settings::validate::list())
}

/// Apply the proposed fixes of the given issues (all when `ids` is
/// empty). Returns how many changed config.toml.
#[tauri::command]
pub async fn apply_config_fixes(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let n = state.settings.apply_fixes(&ids).map_err(|e| {
        crate::user_error::coded(
            "error",
            tr!("ipc:could_not_fix_the_settings_e", e = format!("{:#}", e)),
        )
    })?;
    // A reload that left the settings unchanged does not broadcast;
    // re-check the chains anyway.
    state.pipeline.rebuild_providers();
    Ok(n)
}

/// Open an `http(s)` link (a decoded QR code) in the default browser. Only
/// ever called from a user click; nothing else is sent anywhere.
#[tauri::command]
pub async fn open_external_url(url: String, app: AppHandle) -> Result<(), String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(user_error::coded(
            "error",
            tr!("ipc:only_http_and_https_links_are_supported"),
        ));
    }
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Show the main window on an in-app page, e.g. the model downloads from
/// the "モデル準備" button.
#[tauri::command]
pub async fn open_main_route(path: String, app: AppHandle) -> Result<(), String> {
    if !path.starts_with('/') || path.contains("//") {
        return Err("invalid route".into());
    }
    crate::app::route_main(&app, &path);
    Ok(())
}

/// The main window's close button: hide to the tray, keep running.
#[tauri::command]
pub async fn hide_main(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

/// Where MoonLens keeps its files, for the settings screen and the
/// uninstall notes. Sizes are in bytes.
#[derive(Serialize)]
pub struct DataLocations {
    /// App version and build commit.
    pub version: String,
    pub config_dir: String,
    pub data_dir: String,
    pub models_dir: String,
    /// Log files (`moonlens.log`); attach them to bug reports.
    pub log_dir: String,
    pub models_bytes: u64,
    pub data_bytes: u64,
}

#[tauri::command]
pub async fn data_locations(app: AppHandle) -> Result<DataLocations, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let models_dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    let (m, d) = (models_dir.clone(), data_dir.clone());
    let (models_bytes, data_bytes) =
        tokio::task::spawn_blocking(move || (dir_size(&m), dir_size(&d)))
            .await
            .map_err(|e| e.to_string())?;
    Ok(DataLocations {
        version: crate::build_version(),
        config_dir: config_dir.display().to_string(),
        data_dir: data_dir.display().to_string(),
        models_dir: models_dir.display().to_string(),
        log_dir: crate::log_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        models_bytes,
        // The models folder may live inside the data folder.
        data_bytes: if models_dir.starts_with(&data_dir) {
            data_bytes.saturating_sub(models_bytes)
        } else {
            data_bytes
        },
    })
}

fn dir_size(p: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else {
        return 0;
    };
    rd.filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Model files left behind by earlier versions (no longer in the catalog)
/// or by an abandoned download of one.
#[tauri::command]
pub async fn list_orphan_llm_files() -> Result<Vec<llm_models::OrphanFile>, String> {
    Ok(llm_models::orphans(&llm_dir()?))
}

#[tauri::command]
pub async fn delete_orphan_llm_file(name: String) -> Result<(), String> {
    let dir = llm_dir()?;
    tokio::task::spawn_blocking(move || llm_models::remove_orphan(&dir, &name))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_args_history_is_optional() {
        let a: TranslateArgs =
            serde_json::from_str(r#"{"text":"x","source_language":"auto","target_language":"ja"}"#)
                .unwrap();
        assert!(a.history.is_empty());
        let a: TranslateArgs = serde_json::from_str(
            r#"{"text":"x","source_language":"auto","target_language":"ja","history":[["Theme","テーマ"]]}"#,
        )
        .unwrap();
        assert_eq!(a.history, vec![("Theme".to_string(), "テーマ".to_string())]);
    }

    #[test]
    fn translate_context_keeps_the_latest_history_within_budget() {
        let line = "a".repeat(1000);
        let history: Vec<(String, String)> = (0..10)
            .map(|i| (format!("{i}{line}"), line.clone()))
            .collect();
        let ctx = translate_context(history);
        assert!(ctx.reference.is_none());
        let cost: usize = ctx
            .history
            .iter()
            .map(|(s, t)| crate::translate_plan::cost(s) + crate::translate_plan::cost(t))
            .sum();
        assert!(cost <= crate::translate_plan::HISTORY);
        assert!(ctx.history.last().unwrap().0.starts_with('9'));
        assert!(translate_context(vec![]).is_empty());
    }
}

#[cfg(test)]
mod image_file_tests {
    use super::is_image_path;
    use std::path::Path;

    #[test]
    fn only_image_extensions_are_read() {
        assert!(is_image_path(Path::new("/tmp/shot.PNG")));
        assert!(is_image_path(Path::new("C:\\photos\\menu.jpeg")));
        assert!(is_image_path(Path::new("a.webp")));
        assert!(!is_image_path(Path::new("/home/u/.ssh/id_ed25519")));
        assert!(!is_image_path(Path::new("notes.txt")));
        assert!(!is_image_path(Path::new("png")));
    }
}

fn image_too_large() -> String {
    tr!("common:image_too_large_limit_40_mb")
}

/// Disk-full message for a download that needs `need` bytes with `free` left.
fn not_enough_space(need: u64, free: u64) -> String {
    tr!(
        "ipc:not_enough_free_space_need_gb_needed",
        need = format!("{:.1}", need as f64 / 1e9),
        free = format!("{:.1}", free as f64 / 1e9)
    )
}
