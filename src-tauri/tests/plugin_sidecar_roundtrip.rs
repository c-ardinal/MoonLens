//! End-to-end sidecar plugin test.
//!
//! Launches a Python-based "echo" plugin and verifies that the host
//! `SidecarPlugin` can speak JSON-RPC 2.0 to it end-to-end. The script
//! is language-agnostic on purpose — if this test passes, a plugin
//! written in any language that can read line-delimited JSON on stdin
//! will interoperate with MoonLens.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

use moonlens_lib::plugins::{manifest::PluginManifest, sidecar_host::SidecarPlugin};
use moonlens_lib::providers::{Provider, TaskInput, TaskOutput};

const PYTHON_ECHO_SIDECAR: &str = r#"#!/usr/bin/env python3
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    input_obj = req['params']['input']
    out = {"jsonrpc":"2.0","id":req['id']}
    if 'Translate' in input_obj:
        t = input_obj['Translate']
        out['result'] = {"Text": f"[py] {t['text']}"}
    elif 'Summarize' in input_obj:
        out['result'] = {"Text": f"sum: {input_obj['Summarize']}"}
    else:
        out['error'] = {"code": -1, "message": "unsupported"}
    sys.stdout.write(json.dumps(out) + "\n")
    sys.stdout.flush()
"#;

fn build_plugin_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_path = bin_dir.join("sidecar");
    fs::write(&bin_path, PYTHON_ECHO_SIDECAR).unwrap();
    let mut perms = fs::metadata(&bin_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&bin_path, perms).unwrap();

    let manifest = r#"
        [plugin]
        id = "com.moonlens.it.echo"
        name = "Echo"
        version = "0.1.0"
        author = ""
        license = "MIT"
        kind = "sidecar"
        entry = "bin/sidecar"
        api_version = "1"

        [capabilities]
        provides = ["translator", "summarize"]

        [permissions]
    "#;
    fs::write(dir.path().join("plugin.toml"), manifest).unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

#[tokio::test]
async fn sidecar_round_trip_translate() {
    let (_guard, dir) = build_plugin_dir();
    let manifest_text = fs::read_to_string(dir.join("plugin.toml")).unwrap();
    let manifest: PluginManifest = toml::from_str(&manifest_text).unwrap();

    let plugin = SidecarPlugin::spawn(&manifest, &dir).expect("spawn sidecar");
    let handles = plugin.provider_handles(&manifest.plugin.id);
    // Plugin advertises translator + summarize but "summarize" is not a
    // recognized capability string; only "translator" creates a provider.
    assert!(handles.iter().any(|(id, _)| id.ends_with(".translator")));

    let (_id, handle) = handles
        .into_iter()
        .find(|(id, _)| id.ends_with(".translator"))
        .unwrap();
    let out = handle
        .run(TaskInput::Translate {
            text: "good morning".into(),
            source_language: "en".into(),
            target_language: "ja".into(),
        })
        .await
        .expect("provider ok");
    match out {
        TaskOutput::Text(t) => assert_eq!(t, "[py] good morning"),
        other => panic!("unexpected output: {other:?}"),
    }
}

#[tokio::test]
async fn sidecar_handles_unicode_payload() {
    let (_guard, dir) = build_plugin_dir();
    let manifest_text = fs::read_to_string(dir.join("plugin.toml")).unwrap();
    let manifest: PluginManifest = toml::from_str(&manifest_text).unwrap();
    let plugin = SidecarPlugin::spawn(&manifest, &dir).expect("spawn");
    let (_id, handle) = plugin
        .provider_handles(&manifest.plugin.id)
        .into_iter()
        .find(|(id, _)| id.ends_with(".translator"))
        .unwrap();
    let out = handle
        .run(TaskInput::Translate {
            text: "猫が 🐱 鳴く".into(),
            source_language: "ja".into(),
            target_language: "en".into(),
        })
        .await
        .expect("provider ok");
    match out {
        TaskOutput::Text(t) => assert_eq!(t, "[py] 猫が 🐱 鳴く"),
        other => panic!("unexpected output: {other:?}"),
    }
}
