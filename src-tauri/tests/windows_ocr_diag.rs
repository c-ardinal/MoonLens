//! TEMPORARY diagnostic (not for merge): where does a Windows.Media.Ocr
//! call spend its time, and what makes one call stall for seconds?
#![cfg(windows)]

use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use moonlens_lib::{
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    providers::{ocr_windows, Capability, TaskInput},
    settings::SettingsStore,
    storage::Storage,
};

const IMAGES: [&str; 6] = [
    "english0001",
    "english0002",
    "english0003",
    "english0004",
    "synth_title_body",
    "synth_ui_buttons",
];

static T0: OnceLock<Instant> = OnceLock::new();

fn say(msg: impl AsRef<str>) {
    let t = T0.get_or_init(Instant::now).elapsed();
    eprintln!("[diag {:>8.3}] {}", t.as_secs_f64(), msg.as_ref());
}

fn png(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/lens")
            .join(format!("{name}.png")),
    )
    .unwrap()
}

fn pipeline(engine: &str) -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!("[providers.ocr]\nchain = [\"{engine}\"]\ntimeout_ms = 30000\n"),
    )
    .unwrap();
    let store = Arc::new(SettingsStore::load_from(cfg).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

async fn ocr(p: &Pipeline, tag: &str, name: &str) -> u128 {
    say(format!("> {tag} {name}"));
    let t = Instant::now();
    let r = p
        .run(
            Capability::Ocr,
            TaskInput::Ocr {
                image_png: png(name),
                languages: vec![],
            },
        )
        .await;
    let ms = t.elapsed().as_millis();
    say(format!(
        "< {tag} {name}: {ms} ms{}{}",
        if r.is_err() { " ERR " } else { "" },
        r.err().map(|e| e.to_string()).unwrap_or_default()
    ));
    ms
}

fn summary(label: &str, mut ms: Vec<u128>) {
    ms.sort_unstable();
    if ms.is_empty() {
        return;
    }
    say(format!(
        "SUMMARY {label}: n {}, min {}, median {}, p95 {}, max {}",
        ms.len(),
        ms[0],
        ms[ms.len() / 2],
        ms[ms.len() * 95 / 100],
        ms[ms.len() - 1]
    ));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "diagnostic: needs Windows and the OCR models"]
async fn windows_ocr_timing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "warn,moonlens_lib::providers::ocr_windows=debug,moonlens_lib::providers::ocr_ppocr=info",
        ))
        .with_writer(std::io::stderr)
        .init();
    say(format!(
        "cores {:?}",
        std::thread::available_parallelism().map(|n| n.get())
    ));

    // P1: the engine alone on a plain thread, no tokio, no other engine.
    say("== P1 direct run_ocr on one std thread, each image 3x");
    std::thread::spawn(|| {
        for name in IMAGES {
            let bytes = png(name);
            for _ in 0..3 {
                let t = Instant::now();
                let r = ocr_windows::run_ocr(&bytes, None);
                say(format!(
                    "P1 {name}: {} ms ({:?} lines)",
                    t.elapsed().as_millis(),
                    r.map(|l| l.len()).map_err(|e| e.to_string())
                ));
            }
        }
    })
    .join()
    .unwrap();

    let win = pipeline("windows");
    let pp = pipeline("ppocr");
    let ocrs = pipeline("ocrs");

    say("== P2 pipeline, windows only, each image 2x");
    for name in IMAGES {
        ocr(&win, "P2 windows", name).await;
        ocr(&win, "P2 windows", name).await;
    }

    say("== P3a ppocr (new image) then windows at once");
    let mut a = Vec::new();
    for i in 0..18 {
        let name = IMAGES[i % 6];
        ocr(&pp, "P3a ppocr", name).await;
        a.push(ocr(&win, "P3a windows", name).await);
    }
    summary("P3a windows right after ppocr", a);
    say("== P3b ppocr (new image), 500 ms pause, windows");
    let mut b = Vec::new();
    for i in 0..18 {
        let name = IMAGES[i % 6];
        ocr(&pp, "P3b ppocr", name).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        b.push(ocr(&win, "P3b windows", name).await);
    }
    summary("P3b windows 500 ms after ppocr", b);

    // P4: tokio retires idle blocking threads after 10 s; the thread
    // that entered the COM apartment goes with them.
    say("== P4 windows, 11 s idle, windows");
    let mut c = Vec::new();
    for i in 0..4 {
        c.push(ocr(&win, "P4 windows", IMAGES[i % 6]).await);
        tokio::time::sleep(Duration::from_secs(11)).await;
    }
    c.push(ocr(&win, "P4 windows", IMAGES[1]).await);
    summary("P4 windows after 11 s idle", c);

    say("== P5 stress: ppocr, windows, ocrs per image, 300 rounds");
    let mut w = Vec::new();
    let mut slow = Vec::new();
    for i in 0..300 {
        let name = IMAGES[i % 6];
        ocr(&pp, "P5 ppocr", name).await;
        let ms = ocr(&win, "P5 windows", name).await;
        if ms > 1500 {
            slow.push((i, name, ms));
        }
        w.push(ms);
        if i % 3 == 0 {
            ocr(&ocrs, "P5 ocrs", name).await;
        }
    }
    summary("P5 windows", w);
    say(format!("P5 windows calls over 1500 ms: {slow:?}"));
}
