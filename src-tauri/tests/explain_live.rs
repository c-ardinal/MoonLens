//! Live check of 内容説明 with the default settings and the default
//! general model: the first request after launch (model not loaded yet)
//! must return an explanation instead of a timeout.
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test explain_live -- --ignored --nocapture
//! ```

use std::{sync::Arc, time::Instant};

use moonlens_lib::{
    pipeline::Pipeline,
    plugins::PluginRegistry,
    providers::{Capability, TaskInput, TaskOutput},
    settings::SettingsStore,
    storage::Storage,
};

const SOURCE: &str = "The North Wind and the Sun were disputing which was the stronger, \
when a traveler came along wrapped in a warm cloak. They agreed that the one who first \
succeeded in making the traveler take his cloak off should be considered stronger than \
the other. Then the North Wind blew as hard as he could, but the more he blew the more \
closely did the traveler fold his cloak around him; and at last the North Wind gave up \
the attempt. Then the Sun shined out warmly, and immediately the traveler took off his \
cloak. And so the North Wind was obliged to confess that the Sun was the stronger of \
the two.";

fn pipeline() -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the gemma-4-e4b model"]
async fn first_explain_after_launch_returns_text() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let p = pipeline();
    // Same prompt as the ウィンドウ翻訳 window (src/routes/lens/+page.svelte).
    let prompt = format!("次のテキストの内容を日本語で簡潔に説明してください:\n\n{SOURCE}");
    let started = Instant::now();
    let r = p
        .run(
            Capability::Explain,
            TaskInput::Explain {
                prompt,
                image_png: None,
            },
        )
        .await;
    println!("explain took {:.1}s", started.elapsed().as_secs_f32());
    let r = r.expect("explain failed");
    let TaskOutput::Text(text) = r.output else {
        panic!("unexpected output");
    };
    println!("{text}");
    assert!(text.chars().count() > 20, "explanation too short: {text:?}");
}
