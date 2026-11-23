use super::*;
use crate::settings::{rules::RULES, store, SettingsStore};

const PRE_RELEASE: &str = include_str!("../../tests/fixtures/config/pre-release-full.toml");

fn table(raw: &str) -> toml::Table {
    raw.parse().unwrap()
}

fn schema() -> Json {
    SettingsStore::json_schema()
}

fn paths(issues: &[Issue]) -> Vec<&str> {
    issues.iter().map(|i| i.path.as_str()).collect()
}

fn fixes(issues: &[Issue]) -> Vec<Fix> {
    issues.iter().filter_map(|i| i.fix.clone()).collect()
}

#[test]
fn bundled_defaults_have_no_issues() {
    let mut t = table(include_str!("default.toml"));
    assert!(migrate(&mut t, &RULES).is_empty());
    assert_eq!(check_schema(&t, &schema()), vec![]);
}

#[test]
fn pre_release_file_is_upgraded_by_the_rules() {
    let mut t = table(PRE_RELEASE);
    // As shipped, the loader has to drop these and the chains name
    // providers the current build does not register.
    let (_, before) = store::resolve_table(&t);
    assert!(!before.rejected.is_empty());

    let done = migrate(&mut t, &RULES);
    assert!(
        done.iter().any(|d| d.contains("paddle → ppocr")),
        "{done:?}"
    );
    assert!(done.iter().any(|d| d.contains("embedded.llm → llamacpp")));
    assert!(done.iter().any(|d| d.contains("hotkeys.stop_all")));

    assert_eq!(
        check_schema(&t, &schema()),
        vec![],
        "nothing left to propose"
    );
    let (s, after) = store::resolve_table(&t);
    assert!(after.rejected.is_empty(), "{:?}", after.rejected);
    assert_eq!(s.providers.ocr.chain, ["ppocr"]);
    assert_eq!(s.providers.translate.chain, ["llamacpp"]);
    assert_eq!(s.subtitle.target_language, "ja");
    assert!(!s.providers.configs.contains_key("paddle"));
    assert!(!s.providers.configs.contains_key("embedded.llm"));

    // Idempotent: a second load changes nothing.
    assert!(migrate(&mut t, &RULES).is_empty());
}

#[test]
fn rename_keeps_a_value_the_file_already_sets() {
    let mut t = table(
        "[translation]\ndefault_target_language = \"en\"\n[subtitle]\ntarget_language = \"fr\"\n",
    );
    migrate(&mut t, &RULES);
    assert_eq!(t["subtitle"]["target_language"].as_str(), Some("fr"));
    assert!(t.get("translation").is_none(), "emptied section is pruned");
}

#[test]
fn alias_can_depend_on_the_task() {
    let mut t = table(
        "[providers.translate]\nchain = [\"embedded.llm\"]\n\
         [providers.explain]\nchain = [\"embedded.llm\", \"deepl\"]\n",
    );
    migrate(&mut t, &RULES);
    let (s, _) = store::resolve_table(&t);
    assert_eq!(s.providers.translate.chain, ["llamacpp"]);
    assert_eq!(s.providers.explain.chain, ["llamacpp.assistant", "deepl"]);
}

#[test]
fn alias_does_not_duplicate_a_chain_entry() {
    let mut t = table("[providers.ocr]\nchain = [\"paddle\", \"ppocr\", \"ocrs\"]\n");
    migrate(&mut t, &RULES);
    let chain: Vec<_> = t["providers"]["ocr"]["chain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(chain, ["ppocr", "ocrs"]);
}

const BROKEN: &str = r##"
typo_at_top = 1
[appearance]
theme = "Dark"
density = "huge"
acent = "#112233"
[overlay]
opacity = 1.5
font_size_px = "big"
max_width_px = 300.0
[subtitle]
vad_aggressiveness = 9
[providers.ocr]
fallback_on = ["error", "sometimes"]
timeout_ms = 50
"##;

#[test]
fn schema_walk_finds_every_kind_of_problem() {
    let issues = check_schema(&table(BROKEN), &schema());
    let by = |p: &str| {
        issues
            .iter()
            .find(|i| i.path == p)
            .unwrap_or_else(|| panic!("{p}: {issues:#?}"))
    };

    assert_eq!(by("typo_at_top").problem, "未知の項目");
    assert_eq!(by("typo_at_top").fix_label.as_deref(), Some("削除"));
    // A near miss is renamed rather than deleted.
    assert_eq!(
        by("appearance.acent").fix_label.as_deref(),
        Some("「accent」に改名")
    );
    // Case-only mismatch is corrected; anything else falls back to default.
    assert_eq!(
        by("appearance.theme").fix_label.as_deref(),
        Some("「dark」に補正")
    );
    assert!(by("appearance.density").problem.contains("compact"));
    assert_eq!(
        by("appearance.density").fix_label.as_deref(),
        Some("既定値に復元")
    );
    assert_eq!(by("overlay.opacity").fix_label.as_deref(), Some("1 に補正"));
    assert!(by("overlay.font_size_px").problem.starts_with("型不一致"));
    assert_eq!(
        by("overlay.max_width_px").fix_label.as_deref(),
        Some("整数 300 に補正")
    );
    assert_eq!(
        by("subtitle.vad_aggressiveness").fix_label.as_deref(),
        Some("3 に補正")
    );
    assert_eq!(
        by("providers.ocr.fallback_on[1]").fix_label.as_deref(),
        Some("配列から除外")
    );
    assert_eq!(
        by("providers.ocr.timeout_ms").fix_label.as_deref(),
        Some("100 に補正")
    );
    assert_eq!(issues.len(), 10, "{:?}", paths(&issues));
}

#[test]
fn applying_the_fixes_leaves_a_clean_file_and_keeps_good_values() {
    let mut t = table(BROKEN);
    let issues = check_schema(&t, &schema());
    assert_eq!(apply(&mut t, &fixes(&issues)), issues.len());
    assert_eq!(check_schema(&t, &schema()), vec![]);
    let (s, o) = store::resolve_table(&t);
    assert!(o.rejected.is_empty(), "{:?}", o.rejected);
    assert_eq!(s.appearance.accent, "#112233");
    assert!(matches!(
        s.appearance.theme,
        crate::settings::schema::Theme::Dark
    ));
    assert_eq!(s.overlay.max_width_px, 300);
    assert_eq!(s.subtitle.vad_aggressiveness, 3);
    assert_eq!(s.providers.ocr.fallback_on.len(), 1);
    // Applying again is a no-op.
    assert_eq!(apply(&mut t, &fixes(&issues)), 0);
}

#[test]
fn every_value_the_loader_drops_is_reported() {
    let t = table(BROKEN);
    let issues = check_schema(&t, &schema());
    let (_, o) = store::resolve_table(&t);
    for r in &o.rejected {
        assert!(
            issues.iter().any(|i| starts_with(&i.path, r)),
            "{} dropped but not reported",
            display(r)
        );
    }
}

#[test]
fn chains_are_checked_against_registered_providers() {
    let (mut s, _) = store::resolve("");
    s.providers.ocr.chain = vec!["ppocr".into(), "paddle".into(), "llamacpp".into()];
    s.providers.translate.chain = vec!["nonexistent".into()];
    let registered: HashMap<String, Vec<Capability>> = [
        ("ppocr", vec![Capability::Ocr]),
        ("ocrs", vec![Capability::Ocr]),
        ("llamacpp", vec![Capability::Translate, Capability::Explain]),
        (
            "llamacpp.assistant",
            vec![Capability::Explain, Capability::Summarize],
        ),
        ("whisper", vec![Capability::Asr]),
        ("rqrr", vec![Capability::QrHandler]),
        ("builtin.heuristic", vec![Capability::IntentClassifier]),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let issues = check_providers(&s, &registered);
    assert_eq!(
        paths(&issues),
        [
            "providers.translate.chain[0]",
            "providers.ocr.chain[1]",
            "providers.ocr.chain[2]"
        ]
    );
    assert_eq!(
        issues[0].problem,
        "未登録のプロバイダー「nonexistent」（候補：llamacpp）"
    );
    // The only entry is bad: restore the default chain instead of
    // leaving it empty.
    assert_eq!(issues[0].fix_label.as_deref(), Some("既定値に復元"));
    assert_eq!(
        issues[1].problem,
        "未登録のプロバイダー「paddle」（候補：ocrs、ppocr）"
    );
    assert_eq!(issues[1].fix_label.as_deref(), Some("チェーンから除外"));
    assert!(issues[2].problem.starts_with("「llamacpp」はOCR非対応"));

    let mut t = table(
        "[providers.ocr]\nchain = [\"ppocr\", \"paddle\", \"llamacpp\"]\n\
         [providers.translate]\nchain = [\"nonexistent\"]\n",
    );
    apply(&mut t, &fixes(&issues));
    let (s, _) = store::resolve_table(&t);
    assert_eq!(s.providers.ocr.chain, ["ppocr"]);
    assert_eq!(s.providers.translate.chain, ["llamacpp"]);
}

#[test]
fn display_quotes_dotted_keys() {
    let p = vec![
        key("providers"),
        key("configs"),
        key("llamacpp.assistant"),
        Seg::Index(2),
    ];
    assert_eq!(display(&p), "providers.configs.\"llamacpp.assistant\"[2]");
}

#[test]
fn f32_noise_is_not_shown() {
    assert_eq!(num(f64::from(1.7f32)), "1.7");
    assert_eq!(num(0.25), "0.25");
}

#[test]
fn a_version_upgrade_keeps_values_the_loader_dropped() {
    // The one-shot upgrade must not rewrite the file from the resolved
    // settings, or the issues listed for dropped values could no longer
    // be fixed (and "Dark" would silently become the default).
    let dir = std::env::temp_dir().join(format!("moonlens-validate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "[appearance]\ntheme = \"Dark\"\n[privacy]\nallow_cloud_globally = true\n",
    )
    .unwrap();
    let store = SettingsStore::load_from(path.clone()).unwrap();
    assert!(!store.snapshot().privacy.allow_cloud_globally);
    let t = table(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(t["appearance"]["theme"].as_str(), Some("Dark"));
    assert_eq!(t["config_version"].as_integer(), Some(1));
    assert_eq!(t["privacy"]["allow_cloud_globally"].as_bool(), Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}
