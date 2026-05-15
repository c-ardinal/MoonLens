//! Tauri command handlers exposed to the Svelte frontend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    app::AppState,
    capture::{self, CaptureRect},
    clipboard, hotkey, intent,
    lens::{self, geometry::LensInsets, LensDescriptor},
    overlay,
    pipeline::{CaptureFlowResult, TaskResult},
    plugins::PluginDescriptor,
    providers::{ocr_ocrs, Capability, TaskInput},
    selector,
    settings::schema::Settings,
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
    // Hotkey bindings might have changed; re-register best-effort and
    // surface per-shortcut failures via the `moonlens://hotkey-failure`
    // event rather than rejecting the whole save.
    let report = hotkey::reapply(&app).map_err(|e| e.to_string())?;
    hotkey::notify_failures(&app, &report);
    Ok(())
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

#[tauri::command]
pub async fn uninstall_plugin(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.plugins.uninstall(&id).map_err(|e| e.to_string())?;
    // Removing a plugin can drop providers that the active chains
    // referenced; refresh the pipeline so the next request resolves to
    // whatever is still installed.
    state.pipeline.rebuild_providers();
    Ok(())
}

/// Open the on-disk directory backing a plugin in the OS file
/// manager. We delegate to `tauri-plugin-opener` so the same call
/// works on macOS (Finder), Windows (Explorer), and Linux (xdg-open).
#[tauri::command]
pub async fn reveal_plugin_folder(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = state
        .plugins
        .path_of(&id)
        .ok_or_else(|| format!("plugin {id} not found"))?;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct TranslateArgs {
    pub text: String,
    pub source_language: String,
    pub target_language: String,
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
                glossary: Vec::new(),
            },
        )
        .await
        .map_err(|e| e.to_string())
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
        .map_err(|e| e.to_string())
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
        .map_err(|e| e.to_string())
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

/// End-to-end: capture a region (or the whole primary monitor if `args`
/// is `None`), run intent → OCR → translate, and return the full flow
/// result. Emits overlay events so the floating panel updates live.
/// Start the region watcher on the last captured rect so subsequent
/// pixel changes refresh the overlay automatically. Matches the
/// "pinned overlay" UX. Idempotent: calling it twice just replaces
/// the target.
#[tauri::command]
pub async fn pin_overlay(
    args: RegionArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = crate::region_watcher::WatchTarget {
        rect: CaptureRect {
            monitor_index: args.monitor_index,
            x: args.x,
            y: args.y,
            w: args.w,
            h: args.h,
        },
    };
    state
        .region_watcher
        .start(app, state.pipeline.clone(), state.settings.clone(), target);
    Ok(())
}

#[tauri::command]
pub async fn unpin_overlay(state: State<'_, AppState>) -> Result<(), String> {
    state.region_watcher.stop();
    Ok(())
}

/// Surface a backend failure on the overlay event stream so every
/// page that subscribes (capture, overlay window) can show it. The
/// error is also returned to the caller so the original `invoke`
/// promise rejects cleanly. Centralising this here means new capture
/// entry points only need to wrap their fallible body in `?` after
/// calling this.
fn emit_capture_error(app: &AppHandle, message: impl Into<String>) -> String {
    let msg = message.into();
    overlay::emit(
        app,
        overlay::OverlayEvent::Error {
            message: msg.clone(),
        },
    )
    .ok();
    msg
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
            message: "キャプチャ中…".into(),
        },
    )
    .ok();

    let args_for_capture = args.clone();
    let png = match tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
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
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            return Err(emit_capture_error(&app, format!("キャプチャ失敗: {e}")));
        }
        Err(e) => {
            return Err(emit_capture_error(
                &app,
                format!("キャプチャタスク失敗: {e}"),
            ));
        }
    };

    let mut bundle = match state.pipeline.run_capture_flow(png).await {
        Ok(b) => b,
        Err(e) => {
            return Err(emit_capture_error(&app, format!("解析失敗: {e}")));
        }
    };
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

    // Honour the clipboard policy: translation for text captures, QR
    // payload for QR captures. Source text is only copied when the user
    // opted in (default off). All of this is no-op safe when the user
    // disabled the relevant setting.
    let clipboard_cfg = state.settings.snapshot().clipboard.clone();
    if let Some(src) = &bundle.source_text {
        clipboard::auto_copy(&app, clipboard::Kind::Source, src, &clipboard_cfg);
    }
    match &bundle.result.output {
        crate::providers::TaskOutput::Text(t) => {
            let kind = match bundle.intent {
                crate::providers::Intent::Qr => clipboard::Kind::Qr,
                _ => clipboard::Kind::Translation,
            };
            clipboard::auto_copy(&app, kind, t, &clipboard_cfg);
        }
        crate::providers::TaskOutput::Ocr { lines } => {
            let joined = lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            clipboard::auto_copy(&app, clipboard::Kind::Source, &joined, &clipboard_cfg);
        }
        _ => {}
    }

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

/// Outcome of a probe against the configured ASR endpoint. Reported
/// back to the audio page so the user can diagnose "no transcription"
/// without diving into logs or settings.
#[derive(Debug, Clone, Serialize)]
pub struct AsrEndpointReport {
    /// `true` when the HTTP probe completed (any status code, even 4xx).
    /// `false` when the connection itself failed (DNS / refused / timeout).
    pub reachable: bool,
    /// HTTP status code returned by the probe, when one was received.
    pub status: Option<u16>,
    /// The URL we attempted to reach (so the user can copy it).
    pub url: String,
    /// Provider id picked from the active asr chain (e.g. `whisper_cpp.large_v3_turbo`).
    pub provider: String,
    /// Free-form error / status message displayed alongside the result.
    pub message: String,
}

/// Probe the active ASR provider's endpoint with a short HEAD/GET.
/// Designed to be cheap and side-effect-free — never sends audio data
/// or initialises a model.
#[tauri::command]
pub async fn test_asr_endpoint(state: State<'_, AppState>) -> Result<AsrEndpointReport, String> {
    let snap = state.settings.snapshot();
    let provider = snap
        .providers
        .asr
        .chain
        .first()
        .cloned()
        .unwrap_or_default();
    if provider.is_empty() {
        return Ok(AsrEndpointReport {
            reachable: false,
            status: None,
            url: String::new(),
            provider: String::new(),
            message: "ASR プロバイダのチェーンが空です。設定 → プロバイダ → asr で provider id を追加してください。".into(),
        });
    }

    let url = snap
        .providers
        .configs
        .get(&provider)
        .and_then(|cfg| cfg.get("endpoint"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let Some(url) = url else {
        return Ok(AsrEndpointReport {
            reachable: false,
            status: None,
            url: String::new(),
            provider,
            message:
                "プロバイダの endpoint 設定が見当たりません。providers.configs.<provider>.endpoint を構成してください。"
                    .into(),
        });
    };

    let probe_url = url.trim_end_matches('/').to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    match client.get(&probe_url).send().await {
        Ok(resp) => Ok(AsrEndpointReport {
            reachable: true,
            status: Some(resp.status().as_u16()),
            url: probe_url,
            provider,
            message: format!(
                "接続成功 (HTTP {}). 実際の文字起こしには /inference エンドポイントが応答する必要があります。",
                resp.status().as_u16()
            ),
        }),
        Err(e) => Ok(AsrEndpointReport {
            reachable: false,
            status: None,
            url: probe_url,
            provider,
            message: format!(
                "接続失敗: {}. ASRサーバが起動していない可能性があります。",
                e
            ),
        }),
    }
}

/// Lightweight description of a capturable OS window. Returned by
/// `list_windows` so the frontend can render a picker without
/// each row needing a synchronous backend call.
#[derive(Debug, Clone, Serialize)]
pub struct WindowDescriptor {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub is_minimized: bool,
}

/// Enumerate every visible top-level window in z-order. Minimised
/// windows are included so the user can still choose them and we will
/// capture whatever pixels the OS reports (most platforms still keep a
/// snapshot for the taskbar preview). Windows whose title and
/// app_name are both empty are filtered — those tend to be invisible
/// helper windows that cannot be meaningfully captured.
#[tauri::command]
pub async fn list_windows() -> Result<Vec<WindowDescriptor>, String> {
    let descriptors = tokio::task::spawn_blocking(|| -> Result<Vec<WindowDescriptor>, String> {
        let windows = xcap::Window::all().map_err(|e| e.to_string())?;
        let mut out: Vec<WindowDescriptor> = Vec::with_capacity(windows.len());
        for w in &windows {
            let title = w.title().unwrap_or_default();
            let app_name = w.app_name().unwrap_or_default();
            if title.trim().is_empty() && app_name.trim().is_empty() {
                continue;
            }
            let id = match w.id() {
                Ok(id) => id,
                Err(_) => continue,
            };
            out.push(WindowDescriptor {
                id,
                title,
                app_name,
                x: w.x().unwrap_or(0),
                y: w.y().unwrap_or(0),
                w: w.width().unwrap_or(0),
                h: w.height().unwrap_or(0),
                is_minimized: w.is_minimized().unwrap_or(false),
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(descriptors)
}

/// Capture a specific OS window by id and run the same OCR/translate
/// flow as a region capture. Mirrors `capture_and_process`'s overlay
/// event emission so the capture page UI lights up identically.
#[tauri::command]
pub async fn capture_window_by_id(
    id: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::pipeline::CaptureFlowResult, String> {
    overlay::emit(
        &app,
        overlay::OverlayEvent::Processing {
            message: "ウィンドウをキャプチャ中…".into(),
        },
    )
    .ok();

    let png = match tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let img = capture::capture_window_by_id(id)?;
        capture::png_bytes(&img)
    })
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            return Err(emit_capture_error(
                &app,
                format!("ウィンドウキャプチャ失敗: {e}"),
            ));
        }
        Err(e) => {
            return Err(emit_capture_error(
                &app,
                format!("キャプチャタスク失敗: {e}"),
            ));
        }
    };

    let bundle = match state.pipeline.run_capture_flow(png).await {
        Ok(b) => b,
        Err(e) => {
            return Err(emit_capture_error(&app, format!("解析失敗: {e}")));
        }
    };

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

    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            intent: Some(bundle.intent),
            result: bundle.result.clone(),
            source_rect: None,
        },
    )
    .ok();
    Ok(bundle)
}

/// Open the platform file picker, then process the chosen image file.
/// Returns `Ok(None)` if the user cancels the dialog. Mirrors
/// `pick_and_install_plugin`'s shape so the frontend doesn't need the
/// JS dialog plugin.
#[tauri::command]
pub async fn pick_and_capture_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<crate::pipeline::CaptureFlowResult>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .add_filter("Image", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
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
    let bundle = capture_file(path, app, state).await?;
    Ok(Some(bundle))
}

/// Read an image file off disk and run it through the same flow as a
/// live capture. Used by the main window's "ファイル" mode so users
/// can re-OCR an existing screenshot without using the region selector.
///
/// The file is decoded to PNG bytes (matching what the capture path
/// produces) and handed to `Pipeline::run_capture_flow`. Overlay
/// events are emitted exactly like `capture_and_process` so the
/// capture page UI lights up identically.
#[tauri::command]
pub async fn capture_file(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::pipeline::CaptureFlowResult, String> {
    overlay::emit(
        &app,
        overlay::OverlayEvent::Processing {
            message: "ファイルを読み込み中…".into(),
        },
    )
    .ok();

    let png = match tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let img = image::open(&path)?;
        let mut buf: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)?;
        Ok(buf)
    })
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            return Err(emit_capture_error(
                &app,
                format!("ファイル読み込み失敗: {e}"),
            ));
        }
        Err(e) => {
            return Err(emit_capture_error(
                &app,
                format!("ファイル処理タスク失敗: {e}"),
            ));
        }
    };

    let bundle = match state.pipeline.run_capture_flow(png).await {
        Ok(b) => b,
        Err(e) => {
            return Err(emit_capture_error(&app, format!("解析失敗: {e}")));
        }
    };

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

    // The main window capture page subscribes to this event stream, so
    // emitting `Result` is enough to drive the UI even though we don't
    // pop the floating overlay (the user is already in the capture
    // page when they pick a file).
    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            intent: Some(bundle.intent),
            result: bundle.result.clone(),
            source_rect: None,
        },
    )
    .ok();
    Ok(bundle)
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

#[tauri::command]
pub async fn install_ocr_models(app: AppHandle) -> Result<String, String> {
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    let app_for_cb = app.clone();
    let cb: Box<ocr_ocrs::ProgressCb> = Box::new(move |filename, downloaded, total| {
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
    });
    ocr_ocrs::install_models_with_progress(&dir, Some(&*cb))
        .await
        .map_err(|e| e.to_string())?;
    Ok(dir.display().to_string())
}

/// Download PP-OCRv5 multilingual + Korean model files from
/// `monkt/paddleocr-onnx` (Apache 2.0). The Korean recogniser is
/// fetched alongside the multilingual one by default — it costs 13
/// MB extra and powers the Hangul-routed best-of-two pass.
///
/// Progress events stream on `moonlens://paddleocr-model-progress`
/// with the same `ModelProgress` shape as ocrs / embedded LLM, so
/// the frontend can reuse the existing progress badge component.
#[cfg(feature = "paddleocr-engine")]
#[tauri::command]
pub async fn install_paddleocr_models(
    app: AppHandle,
    install_korean: Option<bool>,
) -> Result<String, String> {
    use crate::providers::ocr_paddle;
    let dir = ocr_paddle::default_models_dir().ok_or("no data dir")?;
    let app_for_cb = app.clone();
    let cb: Box<ocr_paddle::ProgressCb> = Box::new(move |filename, downloaded, total| {
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
        let _ = app_for_cb.emit("moonlens://paddleocr-model-progress", payload);
    });
    ocr_paddle::install_models_with_progress(&dir, install_korean.unwrap_or(true), Some(&*cb))
        .await
        .map_err(|e| e.to_string())?;
    Ok(dir.display().to_string())
}

/// Whether the PaddleOCR model files are present on disk. The
/// settings UI uses this to decide between "ダウンロード" and
/// "準備済み" without making the user click and find out.
#[derive(serde::Serialize, Clone)]
pub struct PaddleOcrModelStatus {
    pub dir: String,
    pub multi_present: bool,
    pub korean_present: bool,
    pub total_bytes: u64,
}

#[cfg(feature = "paddleocr-engine")]
#[tauri::command]
pub fn paddleocr_models_status() -> Result<PaddleOcrModelStatus, String> {
    use crate::providers::ocr_paddle;
    let dir = ocr_paddle::default_models_dir().ok_or("no data dir")?;
    let files_multi = ["det.onnx", "rec_multi.onnx", "dict_multi.txt"];
    let files_ko = ["rec_ko.onnx", "dict_ko.txt"];
    let total = |files: &[&str]| -> (bool, u64) {
        let mut all = true;
        let mut sum = 0u64;
        for f in files {
            let p = dir.join(f);
            match std::fs::metadata(&p) {
                Ok(m) if m.len() > 0 => sum += m.len(),
                _ => {
                    all = false;
                    break;
                }
            }
        }
        (all, sum)
    };
    let (multi_present, multi_bytes) = total(&files_multi);
    let (korean_present, ko_bytes) = total(&files_ko);
    Ok(PaddleOcrModelStatus {
        dir: dir.display().to_string(),
        multi_present,
        korean_present,
        total_bytes: multi_bytes + ko_bytes,
    })
}

/// Status snapshot for the embedded LLM model file. Used by the
/// settings UI to decide whether to surface the "ダウンロード" CTA
/// or the "準備済み" badge without round-tripping the path through
/// the frontend on every render.
#[derive(serde::Serialize, Clone)]
pub struct EmbeddedModelStatus {
    /// Absolute path on disk where MoonLens expects the GGUF model.
    pub path: String,
    /// True when the file exists. Does NOT verify the file is a
    /// valid GGUF — the inference call validates the format on
    /// load and surfaces a friendlier error if the file is corrupt.
    pub present: bool,
    /// Bytes already on disk. Lets the UI render "準備済み 770MB"
    /// or a partial state if a previous download was interrupted.
    pub size_bytes: Option<u64>,
    /// Resolved model identifier (`repo:filename_remote`) so the
    /// settings UI can show which variant the user has installed.
    pub model_id: String,
    /// Human-readable license advert (e.g. "Apache-2.0") + URL.
    /// Surfaced so corporate deployments can audit the bundle
    /// without leaving the app.
    pub license: Option<String>,
    pub license_url: Option<String>,
}

/// One model's presence + size for the `routed_model_status` IPC.
/// `Standard` and `Quality` tiers need 2 GGUFs (a JA bridge + a
/// multilingual model); the UI iterates this list to surface
/// per-model readiness rather than the single-file view the
/// legacy `embedded_model_status` provides.
#[derive(serde::Serialize, Clone)]
pub struct RoutedModelStatus {
    /// Registry id from `model_registry::TIERS` (`llm-jp-3.1-1.8b-instruct4-q4_k_m`
    /// etc.). Stable; the UI uses it to key download progress events
    /// to a specific row.
    pub tier_id: String,
    /// Human-readable label ("JA-Standard+ (llm-jp 3.1 1.8B)" etc.).
    pub display_name: String,
    /// Approximate on-disk size in MB. Pulled from the registry —
    /// matches what the legacy single-tier picker showed.
    pub size_mb: u32,
    /// `true` when EVERY file in the tier's `filenames` list is on
    /// disk. Multi-part GGUFs (Qwen 2.5 7B) only flip true after
    /// both parts have downloaded.
    pub present: bool,
    /// SPDX-style license advert ("Apache-2.0", "Gemma-Terms-of-Use").
    pub license: String,
    /// HuggingFace repo (`mmnga/llm-jp-3.1-1.8b-instruct4-gguf` etc.)
    /// — the UI uses this to pass to the existing
    /// `downloadEmbeddedModel` IPC when a "ダウンロード" button is
    /// clicked on a missing-model row.
    pub repo: String,
    /// Primary filename ("...-Q4_K_M.gguf"). Same purpose as `repo`
    /// — round-tripped through the download IPC.
    pub primary_filename: String,
}

#[tauri::command]
pub async fn embedded_model_status(
    state: State<'_, AppState>,
) -> Result<EmbeddedModelStatus, String> {
    let s = state.models.status().await.map_err(|e| e.to_string())?;
    Ok(EmbeddedModelStatus {
        path: s.path.display().to_string(),
        present: s.present,
        size_bytes: if s.present { Some(s.size_bytes) } else { None },
        model_id: s.model_id,
        license: Some(s.license),
        license_url: Some(s.license_url),
    })
}

/// All shipped model tiers, returned to the settings dropdown so
/// the user can pick a Compact / Standard / Quality build that
/// matches their hardware. The list mirrors `model_registry::TIERS`
/// — the registry is the source of truth.
#[tauri::command]
pub async fn list_model_tiers() -> Result<Vec<crate::model_registry::ModelTier>, String> {
    Ok(crate::model_registry::TIERS.to_vec())
}

/// Result of [`get_model_tier`] — the tier the user has currently
/// selected (resolved from the saved repo+filename in user.toml,
/// falling back to the default if the saved coord predates the
/// registry). Includes the recommended tier separately so the UI
/// can surface "現在: Standard / 推奨: Compact" alongside.
#[derive(serde::Serialize, Clone)]
pub struct ModelTierStatus {
    /// Currently-installed tier. `None` when the user has hand-edited
    /// `user.toml` to point at a coord we do not ship — the UI
    /// renders that as "カスタム" and lets them pick a registered
    /// tier or keep the override.
    pub current: Option<crate::model_registry::ModelTier>,
    /// Tier the HW detector recommends. Used by the onboarding modal
    /// and by the picker's "おすすめ" badge.
    pub recommended: crate::model_registry::ModelTier,
    /// Class label for the recommendation, exposed so the UI can
    /// render localized rationale ("RAM 8GB なので Standard 推奨").
    pub recommended_reason: String,
    /// Total installed RAM (GB), rounded to one decimal place. Shown
    /// in the recommendation tooltip so users can sanity-check the
    /// detector.
    pub total_ram_gb: f64,
    /// Logical CPU count.
    pub cpu_cores: usize,
    /// True when the user's currently-selected tier requires more
    /// RAM than the host has. UI renders a prominent "性能不足"
    /// warning with a one-click switch to the recommended tier
    /// so the user is steered away from configurations that will
    /// time out under real translation load.
    pub current_underpowered: bool,
}

#[tauri::command]
pub async fn model_tier_status(state: State<'_, AppState>) -> Result<ModelTierStatus, String> {
    let snap = state.models.tier_snapshot();
    Ok(ModelTierStatus {
        current: snap.current,
        recommended: snap.recommended,
        recommended_reason: snap.recommended_reason,
        total_ram_gb: snap.host.total_ram_gb,
        cpu_cores: snap.host.cpu_cores,
        current_underpowered: snap.current_underpowered,
    })
}

#[tauri::command]
pub async fn set_model_tier(state: State<'_, AppState>, tier_id: String) -> Result<(), String> {
    state
        .models
        .set_tier(&tier_id)
        .await
        .map_err(|e| e.to_string())
}

/* ---------------- user tier (translation routing) ---------------- */

/// Snapshot of the routing tier state surfaced to the settings UI.
/// The picker renders these three tiers as the only user-facing
/// translation tier choices; per-language model selection happens
/// inside the router based on this single value.
#[derive(serde::Serialize, Clone)]
pub struct UserTierStatus {
    /// Currently-active routing tier, or `None` when the user has
    /// not opted into routing yet (legacy single-tier path is
    /// still in effect for that install).
    pub current: Option<crate::translation_router::UserTier>,
    /// Compile-time hardware bucket (GPU feature flags ON ⇒ Gpu,
    /// else Cpu). The router uses this together with `current` to
    /// pick model paths at translate time. Surfaced here so the UI
    /// can render "CPU mode" / "GPU mode" alongside the tier.
    pub hardware: crate::translation_router::Hardware,
}

/// Read the user's saved routing tier (and detected hardware).
/// Returns `current = None` when the user has not yet opted into
/// routing — in that state, the legacy single-tier surface
/// (`set_model_tier`) is what controls which model loads.
#[tauri::command]
pub async fn get_user_tier(state: State<'_, AppState>) -> Result<UserTierStatus, String> {
    use crate::services::hardware_probe::HardwareProbe;
    let snap = state.settings.snapshot();
    let current = snap.providers.user_tier;
    let hardware = state.models.probe.capabilities().hw_kind();
    Ok(UserTierStatus { current, hardware })
}

/// Persist the user's routing tier choice. `tier_id` must be one
/// of `compact` / `standard` / `quality` — anything else is a
/// validation error.
///
/// Side effects:
/// 1. Writes `providers.user_tier` in `user.toml`.
/// 2. Triggers a pipeline rebuild so the routing-aware provider
///    is reconstructed for the new tier (and any newly-needed
///    models are queued for preload). The rebuild path is the
///    same one settings hot-reload uses, so this command never
///    leaves the running pipeline in a half-applied state.
#[tauri::command]
pub async fn set_user_tier(state: State<'_, AppState>, tier_id: String) -> Result<(), String> {
    let parsed = crate::translation_router::UserTier::from_id(&tier_id)
        .ok_or_else(|| format!("unknown user tier `{tier_id}`"))?;
    let mut s = (*state.settings.snapshot()).clone();
    s.providers.user_tier = Some(parsed);
    state.settings.save(s).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(())
}

/// Clear the routing tier so the install reverts to the legacy
/// single-tier `embedded.llm` provider. Used by the settings UI's
/// "ルーティング無効化" option (mostly for power users / bench
/// comparisons) and by the migration path that lets users
/// downgrade if a routing bug surfaces.
#[tauri::command]
pub async fn clear_user_tier(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = (*state.settings.snapshot()).clone();
    s.providers.user_tier = None;
    state.settings.save(s).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(())
}

/// One row of the preview matrix returned by [`preview_routing`].
/// Mirrors `translation_router::RoutingDecision` but expands the
/// tier IDs into display names so the UI does not need a second
/// round-trip to enrich the labels.
#[derive(serde::Serialize, Clone)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RoutingPreview {
    Direct {
        tier_id: String,
        display_name: String,
    },
    Pivot {
        stage1_tier_id: String,
        stage1_display_name: String,
        stage2_tier_id: String,
        stage2_display_name: String,
    },
    Unsupported,
}

fn preview_from_decision(d: crate::translation_router::RoutingDecision) -> RoutingPreview {
    use crate::translation_router::RoutingDecision as RD;
    fn name_of(id: &str) -> String {
        crate::model_registry::tier_by_id(id)
            .map(|t| t.display_name.to_string())
            .unwrap_or_else(|| id.to_string())
    }
    match d {
        RD::Direct { tier_id } => RoutingPreview::Direct {
            tier_id: tier_id.to_string(),
            display_name: name_of(tier_id),
        },
        RD::Pivot {
            stage1_tier,
            stage2_tier,
        } => RoutingPreview::Pivot {
            stage1_tier_id: stage1_tier.to_string(),
            stage1_display_name: name_of(stage1_tier),
            stage2_tier_id: stage2_tier.to_string(),
            stage2_display_name: name_of(stage2_tier),
        },
        RD::Unsupported => RoutingPreview::Unsupported,
    }
}

/// Per-model readiness for the user's current `user_tier`. The
/// settings UI calls this to render a row per model required by
/// the active tier (1 for Compact, 2 for Standard / Quality) so
/// users can see at a glance which GGUFs need downloading
/// before translation works end-to-end. The legacy
/// `embedded_model_status` only inspects the single
/// `[providers.configs."embedded.llm"]` config block, which
/// doesn't reflect the multi-model reality once routing is on.
#[tauri::command]
pub async fn routed_model_status(
    state: State<'_, AppState>,
) -> Result<Vec<RoutedModelStatus>, String> {
    use crate::providers::translate_routed::RoutedTranslator;
    let snap = state.settings.snapshot();
    let user_tier = snap
        .providers
        .user_tier
        .unwrap_or(crate::translation_router::UserTier::Standard);
    let tier_ids = RoutedTranslator::tier_ids_for(user_tier);
    let app_data = state
        .pipeline
        .app_data_dir()
        .ok_or_else(|| "app data dir not set".to_string())?;
    let models_dir = crate::providers::translate_embedded_llm::default_model_dir(&app_data);

    let mut rows = Vec::with_capacity(tier_ids.len());
    for tier_id in tier_ids {
        let Some(tier) = crate::model_registry::tier_by_id(tier_id) else {
            continue;
        };
        // Every part has to exist for a multi-part tier (Qwen 2.5 7B
        // ships as two LFS files). A half-downloaded multi-part tier
        // can't load, so we surface that as `present = false`.
        let all_present = tier.filenames.iter().all(|f| models_dir.join(f).exists());
        rows.push(RoutedModelStatus {
            tier_id: tier_id.to_string(),
            display_name: tier.display_name.to_string(),
            size_mb: tier.size_mb,
            present: all_present,
            license: tier.license.to_string(),
            repo: tier.repo.to_string(),
            primary_filename: tier.primary_filename().to_string(),
        });
    }
    Ok(rows)
}

/// Resolve which model(s) the router would dispatch for a given
/// (source, target) pair under the currently-saved user tier.
/// When no tier is saved yet, the preview falls back to
/// `UserTier::Standard` so the UI can render a "you would get..."
/// hint before the user commits to a choice. Surfaced via the
/// settings UI's per-language preview matrix.
#[tauri::command]
pub async fn preview_routing(
    state: State<'_, AppState>,
    source: String,
    target: String,
    tier_override: Option<String>,
) -> Result<RoutingPreview, String> {
    use crate::services::hardware_probe::HardwareProbe;
    let snap = state.settings.snapshot();
    let tier = match tier_override {
        Some(id) => crate::translation_router::UserTier::from_id(&id)
            .ok_or_else(|| format!("unknown user tier `{id}`"))?,
        None => snap
            .providers
            .user_tier
            .unwrap_or(crate::translation_router::UserTier::Standard),
    };
    let hw = state.models.probe.capabilities().hw_kind();
    Ok(preview_from_decision(crate::translation_router::route(
        tier, hw, &source, &target,
    )))
}

/// One bench sample, mirrored to JSON for the UI.
#[derive(serde::Serialize, Clone)]
pub struct BenchSampleResult {
    pub index: u32,
    pub total_ms: u64,
    pub prompt_eval_ms: u64,
    pub decode_ms: u64,
    pub output_tokens: u32,
    pub tokens_per_second: f64,
    pub decode_tokens_per_second: f64,
    pub translated_preview: String,
}

/// Aggregate of [`bench_embedded_translate`]. The settings UI binds
/// against this struct directly.
#[derive(serde::Serialize, Clone)]
pub struct BenchOutcomeResult {
    pub tier_display_name: String,
    pub model_path: String,
    pub samples: Vec<BenchSampleResult>,
    pub p50_tokens_per_second: f64,
    pub p95_tokens_per_second: f64,
    pub mean_tokens_per_second: f64,
    pub p50_decode_tokens_per_second: f64,
    pub p95_decode_tokens_per_second: f64,
    pub mean_decode_tokens_per_second: f64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub host_total_ram_gb: f64,
    pub host_cpu_cores: usize,
    pub total_run_ms: u64,
}

/// Run a representative-translation benchmark against the embedded
/// LLM. Heavy work (the actual inferences) happens in the
/// `ModelService::bench_embedded` call which the IPC simply forwards
/// — keeping the benchmarking harness in services lets unit tests
/// validate the percentile math without driving a real model.
///
/// Live progress is emitted via the `moonlens://embedded-bench-progress`
/// event channel so the UI can show "サンプル X/N 実行中… YY tokens
/// (Z 秒経過)" instead of an opaque spinner. Tokens are throttled
/// to ~4 Hz inside the service.
#[tauri::command]
pub async fn bench_embedded_translate(
    app: AppHandle,
    state: State<'_, AppState>,
    samples: Option<u32>,
) -> Result<BenchOutcomeResult, String> {
    use crate::services::model_service::BenchProgressEvent;
    let n = samples.unwrap_or(5);

    let app_for_cb = app.clone();
    let cb: crate::services::model_service::BenchProgressCallback =
        std::sync::Arc::new(move |event| {
            // Translate the typed Rust event into the JSON shape
            // the frontend listens for. Variant tag is `phase` so
            // the TS code can `switch` on a string.
            let payload = match event {
                BenchProgressEvent::Started {
                    total_samples,
                    tier_display_name,
                } => serde_json::json!({
                    "phase": "started",
                    "total_samples": total_samples,
                    "tier_display_name": tier_display_name,
                }),
                BenchProgressEvent::SampleStarted {
                    index,
                    total,
                    fixture_text,
                } => serde_json::json!({
                    "phase": "sample_started",
                    "index": index,
                    "total": total,
                    "fixture_text": fixture_text,
                }),
                BenchProgressEvent::SampleProgress {
                    index,
                    total,
                    tokens_so_far,
                    elapsed_ms,
                } => serde_json::json!({
                    "phase": "sample_progress",
                    "index": index,
                    "total": total,
                    "tokens_so_far": tokens_so_far,
                    "elapsed_ms": elapsed_ms,
                }),
                BenchProgressEvent::SampleCompleted { sample } => serde_json::json!({
                    "phase": "sample_completed",
                    "sample": {
                        "index": sample.index,
                        "total_ms": sample.total_ms,
                        "prompt_eval_ms": sample.prompt_eval_ms,
                        "decode_ms": sample.decode_ms,
                        "output_tokens": sample.output_tokens,
                        "tokens_per_second": sample.tokens_per_second,
                        "decode_tokens_per_second": sample.decode_tokens_per_second,
                        "translated_preview": sample.translated_preview,
                    },
                }),
                BenchProgressEvent::Completed { outcome: _ } => serde_json::json!({
                    "phase": "completed",
                }),
            };
            let _ = app_for_cb.emit("moonlens://embedded-bench-progress", payload);
        });

    let outcome = state
        .models
        .bench_embedded(n, Some(cb))
        .await
        .map_err(|e| e.to_string())?;
    Ok(BenchOutcomeResult {
        tier_display_name: outcome.tier_display_name,
        model_path: outcome.model_path,
        samples: outcome
            .samples
            .into_iter()
            .map(|s| BenchSampleResult {
                index: s.index,
                total_ms: s.total_ms,
                prompt_eval_ms: s.prompt_eval_ms,
                decode_ms: s.decode_ms,
                output_tokens: s.output_tokens,
                tokens_per_second: s.tokens_per_second,
                decode_tokens_per_second: s.decode_tokens_per_second,
                translated_preview: s.translated_preview,
            })
            .collect(),
        p50_tokens_per_second: outcome.p50_tokens_per_second,
        p95_tokens_per_second: outcome.p95_tokens_per_second,
        mean_tokens_per_second: outcome.mean_tokens_per_second,
        p50_decode_tokens_per_second: outcome.p50_decode_tokens_per_second,
        p95_decode_tokens_per_second: outcome.p95_decode_tokens_per_second,
        mean_decode_tokens_per_second: outcome.mean_decode_tokens_per_second,
        p50_latency_ms: outcome.p50_latency_ms,
        p95_latency_ms: outcome.p95_latency_ms,
        host_total_ram_gb: outcome.host_total_ram_gb,
        host_cpu_cores: outcome.host_cpu_cores,
        total_run_ms: outcome.total_run_ms,
    })
}

/// Result of [`package_embedded_model`] — the path the package
/// landed at + the manifest the importer will see. Returned to the
/// frontend so the UI can show "C:\\... に書き出しました (Apache-2.0,
/// 1.1 GB)" without re-reading the package off disk.
#[derive(serde::Serialize, Clone)]
pub struct EmbeddedModelPackageResult {
    pub path: String,
    pub model_id: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub license: Option<String>,
}

/// Wrap the currently-installed embedded model into a single
/// `.moonlens-model` file the user can drop on a fileserver and
/// distribute to PCs without internet access. The bundle includes
/// a manifest with sha256, license, and exporter version so the
/// importing PC verifies + audits without re-downloading.
#[tauri::command]
pub async fn package_embedded_model(
    state: State<'_, AppState>,
    target_path: String,
    note: Option<String>,
) -> Result<EmbeddedModelPackageResult, String> {
    let outcome = state
        .models
        .package_current(PathBuf::from(target_path), note)
        .await
        .map_err(|e| e.to_string())?;
    Ok(EmbeddedModelPackageResult {
        path: outcome.path.display().to_string(),
        model_id: outcome.manifest.model_id,
        size_bytes: outcome.manifest.size_bytes,
        sha256: outcome.manifest.sha256,
        license: outcome.manifest.license,
    })
}

/// Open a "save as" dialog and package the embedded model into the
/// chosen path. Mirrors the `pick_and_install_plugin` pattern: the
/// frontend calls one command, the Rust side handles the dialog so
/// we do not have to add `@tauri-apps/plugin-dialog` to the JS deps.
#[tauri::command]
pub async fn pick_and_package_embedded_model(
    app: AppHandle,
    state: State<'_, AppState>,
    note: Option<String>,
) -> Result<Option<EmbeddedModelPackageResult>, String> {
    use crate::embedded_model_package::PACKAGE_EXTENSION;
    use tauri_plugin_dialog::DialogExt;

    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .add_filter("MoonLens Model Package", &[PACKAGE_EXTENSION, "zip"])
            .set_file_name("moonlens-embedded-model.moonlens-model")
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = chosen else { return Ok(None) };
    let target = path.into_path().map_err(|e| e.to_string())?;
    let result = package_embedded_model(state, target.to_string_lossy().to_string(), note).await?;
    Ok(Some(result))
}

/// Open an "open file" dialog and import the chosen package.
/// Returns the on-disk path of the imported GGUF, or `None` if the
/// user cancels. Mirrors the `pick_and_install_plugin` flow.
#[tauri::command]
pub async fn pick_and_apply_embedded_model_package(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use crate::embedded_model_package::PACKAGE_EXTENSION;
    use tauri_plugin_dialog::DialogExt;

    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .add_filter("MoonLens Model Package", &[PACKAGE_EXTENSION, "zip"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = chosen else { return Ok(None) };
    let pkg = path.into_path().map_err(|e| e.to_string())?;
    let imported = apply_embedded_model_package(state, pkg.to_string_lossy().to_string()).await?;
    Ok(Some(imported))
}

/// Manifest preview for the import confirmation dialog. The UI calls
/// this BEFORE [`apply_embedded_model_package`] so the user sees
/// model id + license + size before the GGUF lands on disk.
#[tauri::command]
pub async fn inspect_embedded_model_package(
    package_path: String,
) -> Result<serde_json::Value, String> {
    use crate::embedded_model_package::inspect_package;
    let path = PathBuf::from(package_path);
    let manifest = tokio::task::spawn_blocking(move || inspect_package(&path))
        .await
        .map_err(|e| format!("inspect task panicked: {e}"))?
        .map_err(|e| e.to_string())?;
    serde_json::to_value(manifest).map_err(|e| e.to_string())
}

/// Apply a `.moonlens-model` package. Verifies the GGUF sha256
/// against the manifest before atomically swapping it into the
/// models directory; on mismatch the existing model is preserved.
/// Returns the final path of the imported GGUF.
#[tauri::command]
pub async fn apply_embedded_model_package(
    state: State<'_, AppState>,
    package_path: String,
) -> Result<String, String> {
    let outcome = state
        .models
        .import_package(PathBuf::from(package_path))
        .await
        .map_err(|e| e.to_string())?;
    Ok(outcome.path.display().to_string())
}

/// Download the embedded LLM GGUF model. Streams `moonlens://embedded-model-progress`
/// events tagged with the same `ModelProgress` shape used by the OCR
/// flow so the frontend can reuse its progress component.
///
/// Idempotent: when the file is already present and `force=false`,
/// the underlying `ModelService` short-circuits without touching the
/// network. On `force=true` the file is re-downloaded.
#[tauri::command]
pub async fn download_embedded_model(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<String, String> {
    let resolved = state.models.resolved();
    let display_filename = resolved.filename_remote.clone();

    // Wire the progress callback to the existing Tauri event channel
    // so the frontend's progress bar continues to work unchanged.
    // Throttling moved into the service's HTTP impl (~10 Hz emit cadence).
    let app_for_cb = app.clone();
    let cb: crate::services::ProgressCallback = std::sync::Arc::new(move |p| {
        let fraction = p.total.and_then(|t| {
            if t == 0 {
                None
            } else {
                Some((p.downloaded as f32 / t as f32).clamp(0.0, 1.0))
            }
        });
        let payload = ModelProgress {
            filename: display_filename.clone(),
            downloaded: p.downloaded,
            total: p.total,
            fraction,
        };
        let _ = app_for_cb.emit("moonlens://embedded-model-progress", payload);
    });

    let outcome = state
        .models
        .download_current(force, Some(cb))
        .await
        .map_err(|e| e.to_string())?;

    // Final 100% event so the UI can flip to "準備完了" cleanly even
    // when no chunks fired (e.g. the file was already present).
    let payload = ModelProgress {
        filename: resolved.filename_remote.clone(),
        downloaded: outcome.size_bytes,
        total: Some(outcome.size_bytes),
        fraction: Some(1.0),
    };
    let _ = app.emit("moonlens://embedded-model-progress", payload);

    Ok(outcome.path.display().to_string())
}

/// Download a specific routing-tier GGUF by id. Companion to
/// `routed_model_status`: the settings UI offers a "ダウンロード"
/// button per missing model row, which calls this with the
/// row's `tier_id`. Multi-part tiers (e.g. Qwen 2.5 7B) are
/// fetched sequentially and reported as one aggregate progress
/// stream on the same `moonlens://embedded-model-progress`
/// channel the legacy download uses, so the frontend's progress
/// bar code keeps working unchanged.
#[tauri::command]
pub async fn download_routed_model(
    app: AppHandle,
    state: State<'_, AppState>,
    tier_id: String,
    force: bool,
) -> Result<String, String> {
    // Resolve the primary filename ahead of the async work so the
    // progress callback closure does not have to look it up on
    // each chunk emission.
    let tier = crate::model_registry::tier_by_id(&tier_id)
        .ok_or_else(|| format!("tier_id `{tier_id}` not in registry"))?;
    let primary = tier.primary_filename().to_string();

    let app_for_cb = app.clone();
    let primary_for_cb = primary.clone();
    let cb: crate::services::ProgressCallback = std::sync::Arc::new(move |p| {
        let fraction = p.total.and_then(|t| {
            if t == 0 {
                None
            } else {
                Some((p.downloaded as f32 / t as f32).clamp(0.0, 1.0))
            }
        });
        let payload = ModelProgress {
            filename: primary_for_cb.clone(),
            downloaded: p.downloaded,
            total: p.total,
            fraction,
        };
        let _ = app_for_cb.emit("moonlens://embedded-model-progress", payload);
    });

    let outcome = state
        .models
        .download_tier_by_id(&tier_id, force, Some(cb))
        .await
        .map_err(|e| e.to_string())?;

    let payload = ModelProgress {
        filename: primary,
        downloaded: outcome.size_bytes,
        total: Some(outcome.size_bytes),
        fraction: Some(1.0),
    };
    let _ = app.emit("moonlens://embedded-model-progress", payload);

    Ok(outcome.path.display().to_string())
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

/// Relaunch the application. Used when the user toggled a setting
/// (encrypt-at-rest, log level, etc.) that takes effect only at boot.
/// Tauri 2's `app.restart()` spawns a fresh process and exits the
/// current one in a single transaction.
#[tauri::command]
pub async fn restart_app(app: AppHandle) -> Result<(), String> {
    app.restart();
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
    let all = state.storage.recent(cap).map_err(|e| e.to_string())?;
    let needle = query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);
    let filtered = match needle {
        None => all,
        Some(q) => all
            .into_iter()
            .filter(|e| {
                e.source_text
                    .as_deref()
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
                    || e.result_text
                        .as_deref()
                        .map(|s| s.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || e.provider
                        .as_deref()
                        .map(|s| s.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || e.kind.to_lowercase().contains(&q)
            })
            .collect(),
    };
    Ok(filtered)
}

#[tauri::command]
pub async fn delete_history_entry(id: i64, state: State<'_, AppState>) -> Result<usize, String> {
    // Return the affected row count so the UI can distinguish "no such
    // id" (0) from an actual removal (1) without a second round-trip.
    state.storage.delete(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<usize, String> {
    state.storage.clear_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_region_selection(app: AppHandle) -> Result<(), String> {
    selector::show(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_region_selection(app: AppHandle) -> Result<(), String> {
    selector::hide(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    state.subtitle.toggle(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn subtitle_state(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.subtitle.is_running())
}

#[tauri::command]
pub async fn stop_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.subtitle.stop(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.subtitle.start(app).map_err(|e| e.to_string())
}

/// Describes which first-run guidance the UI should show. Exposed as a
/// single struct so the frontend can decide in one round-trip.
#[derive(serde::Serialize)]
pub struct OnboardingStatus {
    pub platform: &'static str,
    pub needs_macos_permissions_review: bool,
    /// Live per-permission snapshot. The modal renders each status as
    /// a badge next to its System Settings link.
    pub macos_permissions: crate::macos_permissions::PermissionReport,
    /// True when the user has not yet been shown the embedded model
    /// setup prompt AND the GGUF file does not exist on disk. The
    /// frontend's first-run modal uses this to decide whether to
    /// pop "ローカル LLM をダウンロードして開始する" before the
    /// user reaches any translation feature.
    pub needs_embedded_model_setup: bool,
    /// Resolved on-disk path of the embedded model. Surfaced so the
    /// onboarding modal can show the user where the download will
    /// land before they accept.
    pub embedded_model_path: String,
    /// True when the embedded provider is in the user's translate
    /// chain — without this, prompting for the download would be
    /// pointless because the chain ignores it. Power users who
    /// removed `embedded.llm` from their chain (e.g. went pure
    /// cloud) skip the modal entirely.
    pub embedded_in_translate_chain: bool,
}

#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, AppState>) -> Result<OnboardingStatus, String> {
    use crate::providers::translate_embedded_llm::default_model_path;

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

    let embedded_path = snapshot
        .providers
        .configs
        .get("embedded.llm")
        .and_then(|cfg| cfg.get("model_path"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| {
            state
                .pipeline
                .app_data_dir()
                .as_ref()
                .map(|p| default_model_path(p))
        })
        .unwrap_or_default();
    let embedded_model_present = embedded_path.is_file();
    let embedded_in_translate_chain = snapshot
        .providers
        .translate
        .chain
        .iter()
        .any(|id| id == "embedded.llm" || id.starts_with("embedded."));
    let needs_embedded_model_setup = !snapshot.onboarding.embedded_model_setup_seen
        && !embedded_model_present
        && embedded_in_translate_chain;

    Ok(OnboardingStatus {
        platform: platform_label(platform),
        needs_macos_permissions_review,
        macos_permissions: perms,
        needs_embedded_model_setup,
        embedded_model_path: embedded_path.display().to_string(),
        embedded_in_translate_chain,
    })
}

/// Mark the embedded-model first-run prompt as seen. Called from the
/// onboarding modal whether the user clicks "ダウンロード" (after
/// success) or "後で". The modal does NOT reappear afterwards; users
/// who change their mind work through the normal Settings → Providers
/// card.
#[tauri::command]
pub async fn mark_embedded_model_setup_seen(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = (*state.settings.snapshot()).clone();
    s.onboarding.embedded_model_setup_seen = true;
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

#[tauri::command]
pub async fn new_lens_window(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let id = lens::create(&app, &state.lenses).map_err(|e| e.to_string())?;
    // Start the event-driven watcher. The initial capture fires
    // immediately because `start` pre-notifies the trigger.
    state.lens_watchers.start(
        id.clone(),
        app.clone(),
        state.pipeline.clone(),
        state.lenses.clone(),
        state.settings.clone(),
    );
    // Hook window events so moving or resizing the lens re-triggers
    // a capture automatically — this is the primary UX for "refresh
    // the translation" in the no-polling MVP. Debouncing lives inside
    // the watcher loop so a long drag only fires once when it ends.
    if let Some(win) = app.get_webview_window(&id) {
        let id_for_events = id.clone();
        let watchers = state.lens_watchers.clone();
        win.on_window_event(move |event| {
            if matches!(
                event,
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
            ) {
                watchers.trigger(&id_for_events);
            }
        });
    }
    Ok(id)
}

/// Manual refresh trigger. Lets a hotkey or UI button re-run the
/// capture/OCR/translate pipeline on demand for use cases where the
/// source content changes without the lens window moving (live
/// subtitles, scrolling pages, video frames).
#[tauri::command]
pub async fn refresh_lens(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.lens_watchers.trigger(&id);
    Ok(())
}

#[tauri::command]
pub async fn close_lens_window(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Stop the watcher first so in-flight pipeline runs have no
    // dangling window to emit `moonlens://lens-result` to.
    state.lens_watchers.stop(&id);
    lens::close(&app, &state.lenses, &id).map_err(|e| e.to_string())
}

/// Summary of what the global panic button actually stopped. Returned
/// so the UI can surface a toast like "Lens 3件・字幕・Overlay を停止"
/// without having to round-trip multiple status checks.
#[derive(Debug, Clone, Serialize, Default)]
pub struct StopAllReport {
    pub lens_count: usize,
    pub subtitle_was_running: bool,
    pub overlay_was_open: bool,
}

/// Panic-button stop: drains every running Lens streaming session,
/// closes each Lens window, halts the subtitle pipeline, and hides the
/// one-shot overlay. Best-effort — individual failures do not abort
/// the rest so the UI is guaranteed to reach a quiet state.
///
/// Surfaces `moonlens://stop-all` with the report so other windows
/// (overlay, subtitle) can reset any local UI state. Bound to the
/// `Ctrl+Alt+Escape` global hotkey by default, exposed to the frontend
/// for the sidebar panic button and command palette entry.
#[tauri::command]
pub async fn stop_all(app: AppHandle, state: State<'_, AppState>) -> Result<StopAllReport, String> {
    // Snapshot state first so the report reflects what was actually
    // running when the user hit the button, not what remains after the
    // cascade of cleanup calls.
    let lens_ids: Vec<String> = state.lenses.list().into_iter().map(|d| d.id).collect();
    let subtitle_was_running = state.subtitle.is_running();
    let overlay_was_open = app
        .get_webview_window("overlay")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    // 1. Drain every per-lens capture task. Safe even if already empty.
    state.lens_watchers.stop_all();

    // 2. Close each Lens window. Individual failures log but don't
    //    stop the rest — the user just hit a panic button, so partial
    //    cleanup is better than returning halfway through.
    let mut lens_count = 0usize;
    for id in &lens_ids {
        match lens::close(&app, &state.lenses, id) {
            Ok(()) => lens_count += 1,
            Err(e) => {
                tracing::warn!("stop_all: failed to close lens {id}: {e:?}");
            }
        }
    }

    // 3. Subtitle pipeline. `stop` is a no-op when nothing is running.
    if let Err(e) = state.subtitle.stop(&app) {
        tracing::warn!("stop_all: failed to stop subtitle pipeline: {e:?}");
    }

    // 4. Overlay window hide. Idempotent — already-hidden overlays just
    //    return Ok.
    if let Err(e) = overlay::hide(&app) {
        tracing::warn!("stop_all: failed to hide overlay: {e:?}");
    }

    let report = StopAllReport {
        lens_count,
        subtitle_was_running,
        overlay_was_open,
    };

    // 5. Tell the frontend so every window (not just main) can reset
    //    any streaming buffer / mask state it was holding.
    let _ = app.emit("moonlens://stop-all", &report);

    Ok(report)
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

#[tauri::command]
pub async fn set_lens_auto_refresh(
    id: String,
    ms: Option<u32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_auto_refresh(&id, ms);
    Ok(())
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
    let png =
        tokio::task::spawn_blocking(move || lens::capture_lens(&app_clone, &lenses, &id_clone))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    let bundle = pipeline
        .run_capture_flow(png)
        .await
        .map_err(|e| e.to_string())?;
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
