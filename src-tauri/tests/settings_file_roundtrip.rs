//! SettingsStore against a real file. Verifies the seed-on-first-run
//! behaviour, user overrides merging with defaults, and save()
//! persisting to the expected path.

use moonlens_lib::settings::{schema::Theme, SettingsStore};

#[test]
fn first_run_seeds_default_toml_on_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    assert!(!config_path.exists());
    let store = SettingsStore::load_from(config_path.clone()).unwrap();
    assert!(config_path.exists(), "store did not seed config.toml");
    assert_eq!(store.config_path(), config_path);
}

#[test]
fn user_overrides_merge_on_top_of_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    // Pre-seed a partial user file that only overrides the theme.
    std::fs::write(
        &config_path,
        r##"
        active_profile = "dev"
        [appearance]
        theme = "dark"
        accent = "#8B5CF6"
        density = "normal"
        ui_language = "ja"
        font_family_ui = "Inter Variable"
        font_family_mono = "JetBrains Mono"
        reduced_motion = false
        "##,
    )
    .unwrap();
    let store = SettingsStore::load_from(config_path).unwrap();
    let s = store.snapshot();
    assert!(matches!(s.appearance.theme, Theme::Dark));
    assert_eq!(s.active_profile.as_deref(), Some("dev"));
}

#[test]
fn save_writes_to_disk_and_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    let store = SettingsStore::load_from(config_path.clone()).unwrap();

    let mut settings = (*store.snapshot()).clone();
    settings.subtitle.target_language = "fr".into();
    store.save(settings).unwrap();

    let raw = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        raw.contains("target_language = \"fr\""),
        "save did not persist: {raw}"
    );

    // Reopening picks up the saved override.
    let reopened = SettingsStore::load_from(config_path).unwrap();
    assert_eq!(reopened.snapshot().subtitle.target_language, "fr");
}

#[test]
fn invalid_user_file_falls_back_to_defaults_without_panicking() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, "not :: valid :: toml").unwrap();
    let store = SettingsStore::load_from(config_path).unwrap();
    // Defaults restored.
    assert_eq!(store.snapshot().subtitle.target_language, "ja");
}

#[test]
fn subscribing_receives_snapshots_after_save() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    let store = SettingsStore::load_from(config_path).unwrap();
    let mut rx = store.subscribe();
    let mut next = (*store.snapshot()).clone();
    next.subtitle.target_language = "de".into();
    store.save(next).unwrap();
    let received = rx
        .try_recv()
        .expect("subscriber should get the saved snapshot");
    assert_eq!(received.subtitle.target_language, "de");
}

/// 音声翻訳の操作透過ショートカット：追加前の設定ファイルは既定値で補完、
/// 利用者の割当ては保存・再読込で維持。
#[test]
fn subtitle_operate_shortcut_fills_in_and_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r##"
        [hotkeys]
        toggle_subtitle = "CommandOrControl+Shift+S"
        [triggers]
        toggle_subtitle = "Mouse4"
        "##,
    )
    .unwrap();
    let store = SettingsStore::load_from(config_path.clone()).unwrap();
    let mut s = (*store.snapshot()).clone();
    assert_eq!(
        s.hotkeys.toggle_subtitle_operate,
        "CommandOrControl+Shift+Y"
    );
    assert_eq!(s.triggers.toggle_subtitle_operate, "");

    s.hotkeys.toggle_subtitle_operate = "CommandOrControl+Alt+T".into();
    s.triggers.toggle_subtitle_operate = "Mouse5".into();
    store.save(s).unwrap();
    let reloaded = SettingsStore::load_from(config_path).unwrap().snapshot();
    assert_eq!(
        reloaded.hotkeys.toggle_subtitle_operate,
        "CommandOrControl+Alt+T"
    );
    assert_eq!(reloaded.triggers.toggle_subtitle_operate, "Mouse5");
}
