//! Settings store: loads layered TOML, publishes snapshots, hot-reloads.
//!
//! `config.toml` holds **only the values the user changed**; everything
//! else comes from the bundled `default.toml`. That way a default that a
//! later version improves reaches every install whose user never touched
//! it, without a migration rule per value. A read-only copy of the
//! defaults is written next to it as `config.default.toml` for reference.
//!
//! Loading never fails on a bad user file: a file that is not TOML is
//! moved aside and the defaults are used; a file with a few bad values
//! keeps every value that is valid and drops only the bad ones. Either
//! way the original is kept as `config.toml.broken-<unix time>` and a
//! notice tells the user what happened.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast;

use super::{
    rules::RULES,
    schema::Settings,
    validate::{self, Path as KeyPath, Seg, Source},
};
use crate::tr;

const DEFAULT_TOML: &str = include_str!("default.toml");

/// First lines of the user file. Kept short; the reference file carries
/// every key with its default.
fn user_header() -> &'static str {
    crate::i18n::text("# MoonLens 設定（変更値のみ）。\n# 既定値の一覧：同一フォルダーの config.default.toml\n# 未記載の項目は既定値を使用。保存時に即時反映。\n")
}

fn defaults_header() -> &'static str {
    crate::i18n::text("# MoonLens 既定値（参照用。起動時に再生成、編集は無効）。\n# 変更は config.toml に該当キーのみ記述。\n")
}

/// Notice id for problems found in config.toml.
pub const NOTICE_CONFIG: &str = "config";
/// Notice id for the automatic upgrade of an old config.toml.
pub const NOTICE_CONFIG_MIGRATED: &str = "config.migrated";

pub struct SettingsStore {
    inner: RwLock<Settings>,
    config_path: PathBuf,
    tx: broadcast::Sender<Arc<Settings>>,
}

/// What [`resolve`] made of a user file.
#[derive(Debug, Default)]
pub struct LoadOutcome {
    /// Paths of values that were dropped (typo, wrong type, …).
    pub rejected: Vec<KeyPath>,
    /// The file was not TOML at all.
    pub unparsable: bool,
    /// `config_version` written in the file; 0 when absent (files from
    /// before versioning).
    pub file_version: u32,
}

impl LoadOutcome {
    fn is_clean(&self) -> bool {
        self.rejected.is_empty() && !self.unparsable
    }
}

impl SettingsStore {
    /// `appearance.ui_language` straight from config.toml, before the full
    /// load: the loader words its own notices (upgrades, syntax errors),
    /// so the display language has to be known first.
    pub fn peek_ui_language(app: &AppHandle) -> String {
        app.path()
            .app_config_dir()
            .ok()
            .and_then(|dir| std::fs::read_to_string(dir.join("config.toml")).ok())
            .and_then(|raw| raw.parse::<toml::Table>().ok())
            .and_then(|t| {
                Some(
                    t.get("appearance")?
                        .get("ui_language")?
                        .as_str()?
                        .to_string(),
                )
            })
            .unwrap_or_else(super::schema::default_ui_language)
    }

    pub fn load_or_default(app: &AppHandle) -> Result<Self> {
        let config_dir = app.path().app_config_dir().context("no app config dir")?;
        std::fs::create_dir_all(&config_dir).ok();
        let config_path = config_dir.join("config.toml");
        Self::load_from(config_path)
    }

    /// Tauri-free entry point: the caller provides the on-disk path.
    /// Integration tests use this to operate on a temp directory without
    /// spinning up a full Tauri runtime.
    pub fn load_from(config_path: PathBuf) -> Result<Self> {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).ok();
            write_if_changed(&parent.join("config.default.toml"), &defaults_reference());
        }

        let settings = if config_path.exists() {
            let (mut s, outcome, user) = load_user_file(&config_path);
            if !outcome.is_clean() {
                let kept = backup_broken(&config_path);
                report(&outcome, kept.as_deref());
            }
            let before = settings_table(&s).ok();
            let legacy = super::migrate::migrate_legacy_defaults(&mut s);
            let versioned = super::migrate::migrate_versioned(&mut s, outcome.file_version);
            if legacy || versioned {
                tracing::info!(
                    "upgraded config.toml from format {} to {}",
                    outcome.file_version,
                    super::migrate::CONFIG_VERSION
                );
                // Persist so one-shot rules do not run again. Patch the
                // user's own table rather than rewriting it from `s`:
                // values the loader dropped stay in the file, so the
                // issues listed for them can still be fixed.
                let body = match (user, before, settings_table(&s)) {
                    (Some(mut t), Some(b), Ok(a)) => {
                        patch_table(&mut t, &b, &a);
                        // The defaults carry the current version, so the
                        // diff alone may not record it.
                        t.insert(
                            "config_version".into(),
                            toml::Value::Integer(i64::from(super::migrate::CONFIG_VERSION)),
                        );
                        user_file(&t)
                    }
                    _ => overrides_toml(&s).map(|b| format!("{}\n{b}", user_header())),
                };
                if let Ok(body) = body {
                    let _ = write_atomic(&config_path, &body);
                }
            }
            s
        } else {
            // A fresh install: nothing is overridden yet.
            let s = defaults();
            if let Ok(body) = overrides_toml(&s) {
                let _ = write_atomic(&config_path, &format!("{}\n{body}", user_header()));
            }
            s
        };

        let (tx, _) = broadcast::channel(16);
        Ok(Self {
            inner: RwLock::new(settings),
            config_path,
            tx,
        })
    }

    pub fn snapshot(&self) -> Arc<Settings> {
        Arc::new(self.inner.read().clone())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Settings>> {
        self.tx.subscribe()
    }

    /// Re-read the on-disk TOML, layer it over the bundled defaults, and
    /// broadcast the new snapshot if anything changed. Used by the
    /// file-watcher for hot reload. A file that does not parse cleanly
    /// keeps the previous settings and raises a notice instead: the user
    /// is probably still typing.
    pub fn reload_from_disk(&self) -> Result<bool> {
        let (mut settings, outcome) = if self.config_path.exists() {
            let (s, o, _) = load_user_file(&self.config_path);
            (s, o)
        } else {
            resolve("")
        };
        if outcome.unparsable {
            report(&outcome, None);
            return Ok(false);
        }
        crate::notices::clear(NOTICE_CONFIG);
        super::migrate::migrate_legacy_defaults(&mut settings);
        super::migrate::migrate_versioned(&mut settings, outcome.file_version);
        let changed = {
            let current = self.inner.read();
            toml::to_string(&*current).ok() != toml::to_string(&settings).ok()
        };
        if changed {
            *self.inner.write() = settings.clone();
            let _ = self.tx.send(Arc::new(settings));
        }
        Ok(changed)
    }

    /// Apply the proposed fixes of the listed issues (every fixable issue
    /// when `ids` is empty) to `config.toml`, keep a copy of the old file,
    /// and reload. Returns how many fixes changed the file.
    pub fn apply_fixes(&self, ids: &[String]) -> Result<usize> {
        let raw = std::fs::read_to_string(&self.config_path).unwrap_or_default();
        let mut table: toml::Table = raw
            .parse()
            .context("config.toml has a syntax error; fix it by hand first")?;
        let n = validate::apply(&mut table, &validate::fixes_for(ids));
        if n > 0 {
            let kept = backup(&self.config_path, "bak");
            tracing::info!(
                "applied {n} config fixes; previous file kept as {:?}",
                kept.as_deref()
            );
            write_atomic(&self.config_path, &user_file(&table)?)
                .with_context(|| format!("writing {}", self.config_path.display()))?;
        }
        self.reload_from_disk()?;
        Ok(n)
    }

    /// Persist `new` (only the values that differ from the defaults) and
    /// broadcast it. The file is replaced atomically so a crash or a full
    /// disk never leaves a half-written config behind.
    pub fn save(&self, new: Settings) -> Result<()> {
        let body = overrides_toml(&new)?;
        write_atomic(&self.config_path, &format!("{}\n{body}", user_header()))
            .with_context(|| format!("writing {}", self.config_path.display()))?;
        if let Ok(t) = body.parse::<toml::Table>() {
            validate::publish(Source::File, validate::check_schema(&t, schema()));
        }
        *self.inner.write() = new.clone();
        let _ = self.tx.send(Arc::new(new));
        Ok(())
    }

    /// JSON Schema for the current `Settings` type, consumed by the frontend
    /// to auto-generate a form.
    pub fn json_schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(Settings)).expect("schema is serializable")
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
}

/// The bundled defaults.
pub fn defaults() -> Settings {
    toml::from_str(DEFAULT_TOML).expect("bundled default.toml is valid")
}

fn defaults_table() -> toml::Table {
    DEFAULT_TOML
        .parse::<toml::Table>()
        .expect("bundled default.toml is valid")
}

fn defaults_reference() -> String {
    format!("{}\n{DEFAULT_TOML}", defaults_header())
}

/// Build settings from a user file layered over the defaults, keeping
/// every valid value and dropping invalid ones (see [`LoadOutcome`]).
pub fn resolve(raw: &str) -> (Settings, LoadOutcome) {
    match raw.parse::<toml::Table>() {
        Ok(t) => resolve_table(&t),
        Err(e) => {
            tracing::warn!("user config is not valid TOML: {e}");
            let outcome = LoadOutcome {
                unparsable: true,
                ..Default::default()
            };
            (defaults(), outcome)
        }
    }
}

/// [`resolve`] for an already parsed user file.
pub fn resolve_table(user: &toml::Table) -> (Settings, LoadOutcome) {
    let mut outcome = LoadOutcome {
        file_version: user
            .get("config_version")
            .and_then(|v| v.as_integer())
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0),
        ..Default::default()
    };
    let mut acc = defaults_table();
    let mut whole = acc.clone();
    deep_merge(&mut whole, user);
    if let Some(s) = build(&whole) {
        return (s, outcome);
    }
    // Something is wrong: add the user's values one section (then one
    // key) at a time and keep each one that still builds.
    for (key, value) in user {
        let mut candidate = acc.clone();
        deep_merge(&mut candidate, &single(key, value));
        if build(&candidate).is_some() {
            acc = candidate;
            continue;
        }
        match value {
            toml::Value::Table(section) => {
                for (k2, v2) in section {
                    let mut candidate = acc.clone();
                    deep_merge(&mut candidate, &single(key, &table_of(k2, v2)));
                    if build(&candidate).is_some() {
                        acc = candidate;
                    } else {
                        outcome
                            .rejected
                            .push(vec![Seg::Key(key.clone()), Seg::Key(k2.clone())]);
                    }
                }
            }
            _ => outcome.rejected.push(vec![Seg::Key(key.clone())]),
        }
    }
    (build(&acc).unwrap_or_else(defaults), outcome)
}

fn build(t: &toml::Table) -> Option<Settings> {
    toml::Value::Table(t.clone()).try_into().ok()
}

fn single(key: &str, value: &toml::Value) -> toml::Table {
    let mut t = toml::Table::new();
    t.insert(key.to_string(), value.clone());
    t
}

fn table_of(key: &str, value: &toml::Value) -> toml::Value {
    toml::Value::Table(single(key, value))
}

/// Recursively overlay `over` onto `base`: tables merge key by key, any
/// other value (including arrays) replaces the base value.
pub fn deep_merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => deep_merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

/// The part of `current` that differs from `base`.
pub fn diff_table(base: &toml::Table, current: &toml::Table) -> toml::Table {
    let mut out = toml::Table::new();
    for (k, v) in current {
        match (base.get(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(c)) => {
                let d = diff_table(b, c);
                if !d.is_empty() {
                    out.insert(k.clone(), toml::Value::Table(d));
                }
            }
            (Some(b), c) if b == c => {}
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

/// `settings` as TOML holding only the values that differ from the
/// defaults. Both sides go through the same serializer so float fields
/// compare exactly.
pub fn overrides_toml(settings: &Settings) -> Result<String> {
    let mut diff = diff_table(&settings_table(&defaults())?, &settings_table(settings)?);
    // Always record the format version, or the file would read as a
    // pre-versioning one next time and one-shot upgrades would rerun.
    diff.insert(
        "config_version".into(),
        toml::Value::Integer(i64::from(settings.config_version.max(1))),
    );
    Ok(toml::to_string_pretty(&diff)?)
}

fn settings_table(s: &Settings) -> Result<toml::Table> {
    match toml::Value::try_from(s)? {
        toml::Value::Table(t) => Ok(t),
        _ => anyhow::bail!("settings did not serialize to a table"),
    }
}

/// Carry the change from `before` to `after` into `user`: changed and
/// added values are set, keys that disappeared are removed. Everything
/// else in `user` is left as written.
pub fn patch_table(user: &mut toml::Table, before: &toml::Table, after: &toml::Table) {
    deep_merge(user, &diff_table(before, after));
    for (k, b) in before {
        match (after.get(k), b) {
            (None, _) => {
                user.remove(k);
            }
            (Some(toml::Value::Table(a)), toml::Value::Table(b)) => {
                if let Some(toml::Value::Table(u)) = user.get_mut(k) {
                    patch_table(u, b, a);
                }
            }
            _ => {}
        }
    }
}

/// Write via a temp file + rename so readers never see a partial file.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn write_if_changed(path: &Path, contents: &str) {
    if std::fs::read_to_string(path).ok().as_deref() != Some(contents) {
        if let Err(e) = write_atomic(path, contents) {
            tracing::warn!("writing {}: {e}", path.display());
        }
    }
}

/// Keep a copy of a user file we could not fully use, so the next save
/// (which writes only the valid values) does not lose the user's text.
fn backup_broken(path: &Path) -> Option<PathBuf> {
    backup(path, "broken")
}

/// Copy `path` to `<name>.<tag>-<unix time>`.
fn backup(path: &Path, tag: &str) -> Option<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = format!("{}.{tag}-{secs}", path.file_name()?.to_string_lossy());
    let dest = path.with_file_name(name);
    match std::fs::copy(path, &dest) {
        Ok(_) => Some(dest),
        Err(e) => {
            tracing::warn!("cannot back up {}: {e}", path.display());
            None
        }
    }
}

/// Tell the user about a file that could not be read at all. Values
/// that were dropped one by one are listed as issues instead (with a
/// fix), see [`load_user_file`].
fn report(outcome: &LoadOutcome, backup: Option<&Path>) {
    if !outcome.unparsable {
        tracing::warn!(
            "config.toml: ignored invalid values; original kept as {:?}",
            backup
        );
        return;
    }
    let saved = backup
        .and_then(|p| p.file_name())
        .map(|n| tr!("元ファイル：{file}。", file = n.to_string_lossy()))
        .unwrap_or_default();
    crate::notices::push(
        NOTICE_CONFIG,
        tr!(
            "設定ファイルの構文エラー。既定値を使用。{saved}",
            saved = saved
        ),
        None,
    );
}

/// Settings JSON Schema, built once.
fn schema() -> &'static serde_json::Value {
    static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(SettingsStore::json_schema)
}

fn user_file(t: &toml::Table) -> Result<String> {
    Ok(format!("{}\n{}", user_header(), toml::to_string_pretty(t)?))
}

/// Read the user file, apply the declarative upgrade rules (persisting
/// them, with a copy of the old file and a notice), publish the problems
/// that are left as issues, and resolve it over the defaults.
fn load_user_file(path: &Path) -> (Settings, LoadOutcome, Option<toml::Table>) {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
        tracing::warn!("cannot read user config: {e}");
        String::new()
    });
    let Ok(mut table) = raw.parse::<toml::Table>() else {
        validate::publish(Source::File, Vec::new());
        let (s, o) = resolve(&raw);
        return (s, o, None);
    };
    let done = validate::migrate(&mut table, &RULES);
    if !done.is_empty() {
        let kept = backup(path, "bak");
        for d in &done {
            tracing::info!("config upgrade: {d}");
        }
        match user_file(&table).map(|body| write_atomic(path, &body)) {
            Ok(Ok(())) => {
                // Provider id changes first: they decide whether
                // translation works at all.
                let mut ordered: Vec<&str> = done.iter().map(String::as_str).collect();
                ordered.sort_by_key(|d| !d.starts_with(validate::alias_prefix()));
                let shown: Vec<&str> = ordered.into_iter().take(2).collect();
                let more = done.len().saturating_sub(shown.len());
                let more = if more > 0 {
                    tr!("他 {more} 件", more = more)
                } else {
                    String::new()
                };
                let saved = kept
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .map(|n| tr!("元ファイル：{file}。", file = n.to_string_lossy()))
                    .unwrap_or_default();
                crate::notices::push(
                    NOTICE_CONFIG_MIGRATED,
                    tr!(
                        "旧形式の設定を自動更新（{done} 件）：{items}{more}。{saved}",
                        done = done.len(),
                        items = shown.join(crate::i18n::text("、")),
                        more = more,
                        saved = saved
                    ),
                    None,
                );
            }
            Ok(Err(e)) => tracing::warn!("cannot write upgraded config: {e}"),
            Err(e) => tracing::warn!("cannot serialize upgraded config: {e}"),
        }
    }
    let mut issues = validate::check_schema(&table, schema());
    let (settings, outcome) = resolve_table(&table);
    // Anything the loader had to drop but the schema walk did not explain
    // (should not happen; kept so no dropped value goes unreported).
    for r in &outcome.rejected {
        if !issues.iter().any(|i| validate::starts_with(&i.path, r)) {
            issues.push(validate::Issue::unreadable(r));
        }
    }
    validate::publish(Source::File, issues);
    (settings, outcome, Some(table))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_user_file_is_the_defaults() {
        let (s, o) = resolve("");
        assert!(o.is_clean());
        assert_eq!(
            toml::to_string(&s).unwrap(),
            toml::to_string(&defaults()).unwrap()
        );
    }

    #[test]
    fn missing_sections_and_keys_come_from_default_toml() {
        // Rust's `Default` for TaskChain has an empty chain; the bundled
        // file has the real one. A partial file must get the latter.
        let (s, o) = resolve("[providers.ocr]\ntimeout_ms = 9000\n");
        assert!(o.is_clean());
        assert_eq!(s.providers.ocr.timeout_ms, 9000);
        assert_eq!(s.providers.ocr.chain, vec!["ppocr", "ocrs"]);
        assert_eq!(s.providers.translate.chain, vec!["llamacpp"]);
        // Provider configs merge per key too.
        let (s, _) = resolve("[providers.configs.llamacpp]\nmodel = \"plamo-3-translate-0.8b\"\n");
        let c = &s.providers.configs["llamacpp"];
        assert_eq!(c["model"], "plamo-3-translate-0.8b");
        assert_eq!(c["n_ctx"], 4096);
    }

    #[test]
    fn bad_values_are_dropped_one_by_one() {
        let raw = r##"
            typo_at_top = 1
            [appearance]
            theme = "rainbow"
            accent = "#112233"
            [subtitle]
            target_language = "fr"
        "##;
        let (s, o) = resolve(raw);
        let rejected: Vec<String> = o.rejected.iter().map(|p| validate::display(p)).collect();
        assert_eq!(rejected, vec!["appearance.theme", "typo_at_top"]);
        assert_eq!(s.appearance.accent, "#112233");
        assert_eq!(s.subtitle.target_language, "fr");
    }

    #[test]
    fn unparsable_file_gives_defaults() {
        let (s, o) = resolve("not :: toml");
        assert!(o.unparsable);
        assert_eq!(s.subtitle.target_language, "ja");
    }

    #[test]
    fn overrides_hold_only_changed_values() {
        let mut s = defaults();
        assert_eq!(overrides_toml(&s).unwrap().trim(), "config_version = 1");
        s.subtitle.target_language = "fr".into();
        s.overlay.opacity = 0.8;
        let out = overrides_toml(&s).unwrap();
        let t: toml::Table = out.parse().unwrap();
        assert_eq!(t.len(), 3, "{out}");
        assert_eq!(t["subtitle"]["target_language"].as_str(), Some("fr"));
        // And it reads back to the same settings.
        let (back, o) = resolve(&out);
        assert!(o.is_clean());
        assert_eq!(
            toml::to_string(&back).unwrap(),
            toml::to_string(&s).unwrap()
        );
    }

    #[test]
    fn version_is_read_from_the_file_not_the_defaults() {
        assert_eq!(resolve("").1.file_version, 0);
        assert_eq!(resolve("config_version = 1").1.file_version, 1);
    }

    #[test]
    fn full_legacy_file_still_loads() {
        // Earlier builds wrote the whole default.toml to config.toml.
        let (s, o) = resolve(DEFAULT_TOML);
        assert!(o.is_clean());
        assert_eq!(s.providers.translate.chain, vec!["llamacpp"]);
    }

    #[test]
    fn empty_asr_chain_in_the_user_file_is_repaired_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "config_version = 1\n\n[providers.asr]\nchain = []\n\n[providers.configs.whisper]\nmodel = \"whisper-small\"\n",
        )
        .unwrap();
        let store = SettingsStore::load_from(path.clone()).unwrap();
        assert_eq!(store.snapshot().providers.asr.chain, vec!["whisper"]);
        // The file no longer overrides the chain; the model choice stays.
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("chain = []"), "{saved}");
        assert!(saved.contains("whisper-small"), "{saved}");
        let again = SettingsStore::load_from(path).unwrap();
        assert_eq!(again.snapshot().providers.asr.chain, vec!["whisper"]);
    }
}
