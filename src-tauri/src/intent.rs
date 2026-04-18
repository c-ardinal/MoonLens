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
    classify_stats(Stats { mean, std })
}

/// Extracted pure scoring so it can be unit tested without synthesizing
/// full images. The thresholds are tuned for on-screen text: text tends to
/// have mid-brightness with high local contrast; photos are more uniform.
#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub mean: f64,
    pub std: f64,
}

pub fn classify_stats(s: Stats) -> Intent {
    if s.std > 70.0 && s.mean > 60.0 && s.mean < 220.0 {
        Intent::Text
    } else if s.std < 35.0 {
        Intent::Photo
    } else {
        Intent::Mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};

    #[test]
    fn tiny_image_is_unknown() {
        let img =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(32, 32, Rgba([128, 128, 128, 255])));
        assert!(matches!(classify(&img), Intent::Unknown));
    }

    #[test]
    fn uniform_gray_is_photo() {
        // Large uniform image has ~0 stddev.
        let img =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(128, 128, Rgba([120, 120, 120, 255])));
        assert!(matches!(classify(&img), Intent::Photo));
    }

    #[test]
    fn text_like_stats_classify_as_text() {
        let s = Stats { mean: 180.0, std: 90.0 };
        assert!(matches!(classify_stats(s), Intent::Text));
    }

    #[test]
    fn very_uniform_stats_classify_as_photo() {
        let s = Stats { mean: 120.0, std: 10.0 };
        assert!(matches!(classify_stats(s), Intent::Photo));
    }

    #[test]
    fn medium_contrast_classifies_as_mixed() {
        let s = Stats { mean: 40.0, std: 50.0 };
        assert!(matches!(classify_stats(s), Intent::Mixed));
    }

    #[test]
    fn bright_low_contrast_classifies_as_photo() {
        // Mean outside text band AND std below 35 → photo.
        let s = Stats { mean: 240.0, std: 10.0 };
        assert!(matches!(classify_stats(s), Intent::Photo));
    }

    #[test]
    fn checkerboard_high_contrast_stats_are_text_like() {
        // 128x128 checkerboard of 8x8 squares alternating black/white.
        // Using grayscale values (equal R/G/B) so the luma conversion
        // does not clip one channel away and skew the mean.
        let mut img = ImageBuffer::new(128, 128);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            let v = if on { 255 } else { 0 };
            *p = Rgba([v, v, v, 255]);
        }
        let intent = classify(&DynamicImage::ImageRgba8(img));
        // Mean ≈ 127, stddev ≈ 127 → text band under our thresholds.
        assert!(matches!(intent, Intent::Text));
    }
}

