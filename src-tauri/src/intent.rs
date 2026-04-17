//! Heuristic intent classification: given a captured region, decide whether
//! it is a QR code, text, photo, or mixed. A plugin can override by
//! registering an `intent_classifier` provider.

use image::{DynamicImage, GenericImageView};

use crate::providers::Intent;

/// Quick heuristics only. A real implementation can chain into a QR detector
/// + a tiny saliency model via plugins.
pub fn classify(img: &DynamicImage) -> Intent {
    let (w, h) = img.dimensions();
    if w < 64 || h < 64 {
        return Intent::Unknown;
    }
    // Contrast density proxy for "has text": compute stddev of luminance.
    let gray = img.to_luma8();
    let mut sum: f64 = 0.0;
    let mut sumsq: f64 = 0.0;
    let n = (gray.width() * gray.height()) as f64;
    for p in gray.pixels() {
        let v = p[0] as f64;
        sum += v;
        sumsq += v * v;
    }
    let mean = sum / n;
    let var = (sumsq / n) - mean * mean;
    let std = var.max(0.0).sqrt();

    if std > 70.0 && mean > 60.0 && mean < 220.0 {
        Intent::Text
    } else if std < 35.0 {
        Intent::Photo
    } else {
        Intent::Mixed
    }
}
