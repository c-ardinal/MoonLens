//! End-to-end pipeline test with in-process stub providers.
//!
//! Uses `Pipeline::with_providers` so the pipeline can be driven without
//! a Tauri runtime or a real plugin registry. The stubs record how many
//! times they were called so we can assert fallback order, stickiness,
//! and early success.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use async_trait::async_trait;
use moonlens_lib::{
    pipeline::Pipeline,
    providers::{Capability, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput},
    settings::{
        schema::{FallbackCondition, Settings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
};

/// Stub provider configured to either succeed, sleep-then-succeed, or
/// fail with a specific error variant. Records call counts so tests can
/// assert that fallback actually happened.
struct StubProvider {
    id: String,
    capability: Capability,
    is_local: bool,
    behaviour: Behaviour,
    calls: Arc<AtomicUsize>,
}

#[derive(Clone)]
enum Behaviour {
    Ok(String),
    Sleep(Duration),
    Err(fn() -> ProviderError),
}

#[async_trait]
impl Provider for StubProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        std::slice::from_ref(&self.capability)
    }
    fn is_local(&self) -> bool {
        self.is_local
    }
    async fn run(&self, _input: TaskInput) -> Result<TaskOutput, ProviderError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        match self.behaviour.clone() {
            Behaviour::Ok(text) => Ok(TaskOutput::Text(text)),
            Behaviour::Sleep(d) => {
                tokio::time::sleep(d).await;
                Ok(TaskOutput::Text("slow".into()))
            }
            Behaviour::Err(f) => Err(f()),
        }
    }
}

fn make(
    id: &str,
    capability: Capability,
    is_local: bool,
    behaviour: Behaviour,
    calls: Arc<AtomicUsize>,
) -> (String, ProviderHandle) {
    let p: ProviderHandle = Arc::new(StubProvider {
        id: id.to_string(),
        capability,
        is_local,
        behaviour,
        calls,
    });
    (id.to_string(), p)
}

fn settings_with_chain(cap: Capability, chain: TaskChain) -> Settings {
    let mut s = Settings::default();
    match cap {
        Capability::Translate => s.providers.translate = chain,
        Capability::Ocr => s.providers.ocr = chain,
        _ => unimplemented!("fixture does not cover {cap:?}"),
    }
    s
}

fn build(
    providers: HashMap<String, ProviderHandle>,
    cap: Capability,
    chain: TaskChain,
) -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    let store = SettingsStore::load_from(cfg_path).unwrap();
    store.save(settings_with_chain(cap, chain)).unwrap();
    let settings = Arc::new(store);
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let pipe = Pipeline::with_providers(settings, storage, providers);
    // Keep tmp dir alive for the duration of the caller by leaking it
    // (it's a tempdir; OS cleans up). Tests are short-lived.
    std::mem::forget(tmp);
    pipe
}

#[tokio::test]
async fn first_provider_succeeds_and_second_is_not_called() {
    let a_calls = Arc::new(AtomicUsize::new(0));
    let b_calls = Arc::new(AtomicUsize::new(0));
    let mut providers = HashMap::new();
    let (id_a, p_a) = make(
        "a",
        Capability::Translate,
        true,
        Behaviour::Ok("hello".into()),
        a_calls.clone(),
    );
    providers.insert(id_a, p_a);
    let (id_b, p_b) = make(
        "b",
        Capability::Translate,
        true,
        Behaviour::Ok("world".into()),
        b_calls.clone(),
    );
    providers.insert(id_b, p_b);

    let chain = TaskChain {
        chain: vec!["a".into(), "b".into()],
        timeout_ms: 1000,
        fallback_on: vec![FallbackCondition::Error],
        ..TaskChain::default()
    };
    let pipe = build(providers, Capability::Translate, chain);
    let res = pipe
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: "x".into(),
                source_language: "en".into(),
                target_language: "ja".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(res.used_provider, "a");
    assert_eq!(a_calls.load(Ordering::Relaxed), 1);
    assert_eq!(b_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn timeout_falls_through_to_next_provider() {
    let a_calls = Arc::new(AtomicUsize::new(0));
    let b_calls = Arc::new(AtomicUsize::new(0));
    let mut providers = HashMap::new();
    let (id_a, p_a) = make(
        "slow",
        Capability::Translate,
        true,
        Behaviour::Sleep(Duration::from_millis(500)),
        a_calls.clone(),
    );
    providers.insert(id_a, p_a);
    let (id_b, p_b) = make(
        "fast",
        Capability::Translate,
        true,
        Behaviour::Ok("done".into()),
        b_calls.clone(),
    );
    providers.insert(id_b, p_b);

    let chain = TaskChain {
        chain: vec!["slow".into(), "fast".into()],
        timeout_ms: 80,
        fallback_on: vec![FallbackCondition::Timeout],
        ..TaskChain::default()
    };
    let pipe = build(providers, Capability::Translate, chain);
    let res = pipe
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: "x".into(),
                source_language: "en".into(),
                target_language: "ja".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(res.used_provider, "fast");
    assert_eq!(b_calls.load(Ordering::Relaxed), 1);
    // The first provider was attempted, even if it never finished.
    assert_eq!(a_calls.load(Ordering::Relaxed), 1);
    assert!(res.fallback_reasons.iter().any(|r| r.contains("timeout")));
}

#[tokio::test]
async fn cloud_provider_is_skipped_when_allow_cloud_is_false() {
    let local_calls = Arc::new(AtomicUsize::new(0));
    let cloud_calls = Arc::new(AtomicUsize::new(0));
    let mut providers = HashMap::new();
    let (id_l, p_l) = make(
        "local",
        Capability::Translate,
        true,
        Behaviour::Err(|| ProviderError::Offline),
        local_calls.clone(),
    );
    providers.insert(id_l, p_l);
    let (id_c, p_c) = make(
        "cloud",
        Capability::Translate,
        false,
        Behaviour::Ok("cloudy".into()),
        cloud_calls.clone(),
    );
    providers.insert(id_c, p_c);

    let chain = TaskChain {
        chain: vec!["local".into(), "cloud".into()],
        allow_cloud: false,
        fallback_on: vec![FallbackCondition::Offline],
        timeout_ms: 1000,
        ..TaskChain::default()
    };
    let pipe = build(providers, Capability::Translate, chain);
    let err = pipe
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: "x".into(),
                source_language: "en".into(),
                target_language: "ja".into(),
            },
        )
        .await
        .unwrap_err();
    // Cloud was denied, so fallback had no target left and we surface the
    // aggregated failure.
    assert!(matches!(err, ProviderError::Other(_)));
    assert_eq!(cloud_calls.load(Ordering::Relaxed), 0);
    assert_eq!(local_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn provider_missing_from_map_is_skipped_with_a_reason() {
    let a_calls = Arc::new(AtomicUsize::new(0));
    let mut providers = HashMap::new();
    let (id_a, p_a) = make(
        "a",
        Capability::Translate,
        true,
        Behaviour::Ok("hi".into()),
        a_calls.clone(),
    );
    providers.insert(id_a, p_a);

    // Chain lists a non-existent provider first; pipeline must fall
    // through to the one that actually exists.
    let chain = TaskChain {
        chain: vec!["ghost".into(), "a".into()],
        fallback_on: vec![FallbackCondition::Error],
        timeout_ms: 1000,
        ..TaskChain::default()
    };
    let pipe = build(providers, Capability::Translate, chain);
    let res = pipe
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: "x".into(),
                source_language: "en".into(),
                target_language: "ja".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(res.used_provider, "a");
    assert_eq!(a_calls.load(Ordering::Relaxed), 1);
    assert!(res
        .fallback_reasons
        .iter()
        .any(|r| r.contains("ghost") && r.contains("not registered")));
}
