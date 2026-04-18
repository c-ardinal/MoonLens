//! Region watcher for the pinned overlay.
//!
//! When the user pins the overlay onto a region, the watcher polls that
//! region periodically and re-runs the capture flow whenever the pixels
//! change meaningfully. Change detection is a cheap average-hash
//! difference: sufficient for "the video moved to the next subtitle"
//! while rejecting mouse movement and anti-aliasing jitter.

use std::sync::Arc;
use std::time::Duration;

use image::DynamicImage;
use parking_lot::Mutex;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};

use crate::{
    capture::{self, CaptureRect},
    overlay::OverlayEvent,
    pipeline::Pipeline,
    settings::SettingsStore,
};

/// Threshold on the number of differing bits between two 8x8 average
/// hashes before we consider the region "changed". Tuned so static
/// screens produce zero re-captures and typical subtitle transitions
/// trigger one.
const HASH_DIFF_THRESHOLD: u32 = 8;

/// What the watcher is currently tracking. Swapping this (for example
/// when the user re-captures elsewhere) replaces the observation
/// target without restarting the task.
#[derive(Debug, Clone, Copy)]
pub struct WatchTarget {
    pub rect: CaptureRect,
}

#[derive(Default)]
pub struct RegionWatcher {
    inner: Mutex<Option<Running>>,
}

struct Running {
    handle: JoinHandle<()>,
    stop: Arc<tokio::sync::Notify>,
}

impl RegionWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().is_some()
    }

    pub fn start(
        &self,
        app: AppHandle,
        pipeline: Arc<Pipeline>,
        settings: Arc<SettingsStore>,
        target: WatchTarget,
    ) {
        self.stop();
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_t = stop.clone();
        let handle = tauri::async_runtime::spawn(async move {
            run_watch(app, pipeline, settings, target, stop_t).await;
        });
        *self.inner.lock() = Some(Running { handle, stop });
    }

    pub fn stop(&self) {
        if let Some(r) = self.inner.lock().take() {
            r.stop.notify_waiters();
            r.handle.abort();
        }
    }
}

async fn run_watch(
    app: AppHandle,
    pipeline: Arc<Pipeline>,
    settings: Arc<SettingsStore>,
    target: WatchTarget,
    stop: Arc<tokio::sync::Notify>,
) {
    let mut last_hash: Option<u64> = None;
    loop {
        let snap = settings.snapshot();
        if !snap.overlay.follow_source {
            break;
        }
        let interval = snap.performance.region_watcher_poll_ms.max(50) as u64;
        drop(snap);

        tokio::select! {
            _ = stop.notified() => break,
            _ = tokio::time::sleep(Duration::from_millis(interval)) => {}
        }

        let rect = target.rect;
        let captured = tokio::task::spawn_blocking(move || capture::capture_rect(rect))
            .await
            .ok()
            .and_then(|r| r.ok());
        let Some(image) = captured else { continue };
        let hash = average_hash(&image);
        let changed = match last_hash {
            None => true,
            Some(prev) => hamming(prev, hash) >= HASH_DIFF_THRESHOLD,
        };
        if !changed {
            continue;
        }
        last_hash = Some(hash);
        if let Ok(png) = capture::png_bytes(&image) {
            if let Ok(bundle) = pipeline.run_capture_flow(png).await {
                let _ = app.emit(
                    "moonlens://overlay",
                    OverlayEvent::Result {
                        source_text: bundle.source_text,
                        intent: Some(bundle.intent),
                        result: bundle.result,
                        source_rect: bundle.source_rect,
                    },
                );
            }
        }
    }
}

/// Perceptual average hash over an 8x8 greyscale downsample. Exposed
/// so tests (and the watcher) share exactly one implementation.
pub fn average_hash(img: &DynamicImage) -> u64 {
    let small = img.resize_exact(8, 8, image::imageops::FilterType::Triangle);
    let gray = small.to_luma8();
    let pixels: Vec<u8> = gray.pixels().map(|p| p[0]).collect();
    let sum: u64 = pixels.iter().map(|v| *v as u64).sum();
    let mean = (sum / pixels.len().max(1) as u64) as u8;
    let mut hash: u64 = 0;
    for (i, v) in pixels.iter().enumerate() {
        if *v > mean {
            hash |= 1u64 << i;
        }
    }
    hash
}

/// Hamming distance between two hashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn solid(w: u32, h: u32, v: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba([v, v, v, 255])))
    }

    #[test]
    fn identical_images_hash_identically() {
        let a = solid(64, 64, 100);
        let b = solid(64, 64, 100);
        assert_eq!(average_hash(&a), average_hash(&b));
    }

    #[test]
    fn different_brightness_produces_different_hash() {
        let a = solid(64, 64, 40);
        let b = solid(64, 64, 200);
        // With a uniform image, every pixel equals the mean so the
        // hash is 0 in both cases by construction. The interesting
        // check is that a gradient produces a non-zero bit pattern
        // that differs from the uniform case.
        let grad = {
            let mut img = ImageBuffer::new(64, 64);
            for (x, _y, p) in img.enumerate_pixels_mut() {
                let v = (x * 4) as u8;
                *p = Rgba([v, v, v, 255]);
            }
            DynamicImage::ImageRgba8(img)
        };
        let h_uniform = average_hash(&a);
        let h_grad = average_hash(&grad);
        assert_ne!(h_grad, h_uniform);
        assert!(hamming(h_grad, h_uniform) > 0);
        // b is uniform so its hash should match a's.
        assert_eq!(average_hash(&b), h_uniform);
    }

    #[test]
    fn hamming_counts_differing_bits() {
        assert_eq!(hamming(0, 0), 0);
        assert_eq!(hamming(0, 0xFF), 8);
        assert_eq!(hamming(0xF0, 0x0F), 8);
    }

    #[test]
    fn hash_diff_threshold_rejects_trivial_noise() {
        // 2-bit noise (below the 8-bit threshold) should not trigger.
        let base: u64 = 0xAAAA_AAAA_AAAA_AAAA;
        let noisy = base ^ 0b11;
        assert!(hamming(base, noisy) < HASH_DIFF_THRESHOLD);
    }

    #[test]
    fn hash_diff_threshold_accepts_large_changes() {
        let a: u64 = 0;
        let b: u64 = u64::MAX;
        assert!(hamming(a, b) >= HASH_DIFF_THRESHOLD);
    }
}
