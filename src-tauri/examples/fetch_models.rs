//! Download the models the live tests need into the app's models folder
//! (the same place the app downloads to): PP-OCRv5, ocrs and the default
//! translation model. Already installed models are skipped.
//!
//! ```sh
//! cd src-tauri
//! cargo run --release --example fetch_models
//! ```

use moonlens_lib::providers::{llm_models, ocr_ocrs, ocr_ppocr};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ocr_dir = ocr_ocrs::default_models_dir().expect("no data dir");
    println!("OCR models: {}", ocr_dir.display());
    ocr_ppocr::install_models_with_progress(
        &ocr_ppocr::default_models_dir().expect("no data dir"),
        None,
    )
    .await?;
    ocr_ocrs::install_models_with_progress(&ocr_dir, None).await?;

    let llm_dir = llm_models::default_models_dir().expect("no data dir");
    let id = llm_models::DEFAULT_TRANSLATE_MODEL_ID;
    println!("translation model {id}: {}", llm_dir.display());
    let path = llm_models::install(&llm_dir, id, None).await?;
    println!("installed {}", path.display());
    Ok(())
}
