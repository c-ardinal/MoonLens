//! Screen and region capture via `xcap`.
//!
//! Pure geometry lives in [`rect`] so the "which monitor contains this
//! rect" logic can be tested without touching the display server.

use anyhow::{Context, Result};
use image::DynamicImage;
use xcap::Monitor;

#[derive(Debug, Clone, Copy)]
pub struct CaptureRect {
    pub monitor_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub mod rect {
    //! Pure monitor-resolution helpers. `Physical` coordinates match what
    //! xcap returns; a monitor is described by its top-left `origin` and
    //! its `size`. The input rect may use global virtual-screen
    //! coordinates (including negative values, common on Linux/Windows
    //! multi-monitor setups).

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MonitorBounds {
        pub origin_x: i32,
        pub origin_y: i32,
        pub w: u32,
        pub h: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GlobalRect {
        pub x: i32,
        pub y: i32,
        pub w: u32,
        pub h: u32,
    }

    /// Monitor-local rect, with (x, y) relative to the monitor's origin,
    /// clamped to the monitor's bounds.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LocalRect {
        pub monitor_index: usize,
        pub x: u32,
        pub y: u32,
        pub w: u32,
        pub h: u32,
    }

    fn overlap_area(a: GlobalRect, b: MonitorBounds) -> i64 {
        let ax0 = a.x as i64;
        let ay0 = a.y as i64;
        let ax1 = ax0 + a.w as i64;
        let ay1 = ay0 + a.h as i64;
        let bx0 = b.origin_x as i64;
        let by0 = b.origin_y as i64;
        let bx1 = bx0 + b.w as i64;
        let by1 = by0 + b.h as i64;
        let dx = (ax1.min(bx1) - ax0.max(bx0)).max(0);
        let dy = (ay1.min(by1) - ay0.max(by0)).max(0);
        dx * dy
    }

    /// Pick the monitor that contains the largest area of `rect`. Ties are
    /// broken in favour of the earlier index so ordering is deterministic.
    pub fn best_monitor(monitors: &[MonitorBounds], rect: GlobalRect) -> Option<usize> {
        let mut best: Option<(usize, i64)> = None;
        for (i, m) in monitors.iter().enumerate() {
            let area = overlap_area(rect, *m);
            if area == 0 {
                continue;
            }
            match best {
                Some((_, prev)) if area > prev => best = Some((i, area)),
                None => best = Some((i, area)),
                _ => {}
            }
        }
        best.map(|(i, _)| i)
    }

    /// Index of the monitor containing the global point `(x, y)`, e.g. the
    /// mouse cursor. Falls back to the nearest monitor (by edge distance)
    /// so a cursor reported a pixel off-screen still resolves.
    pub fn monitor_at_point(monitors: &[MonitorBounds], x: i32, y: i32) -> Option<usize> {
        let dist = |m: &MonitorBounds| -> i64 {
            let (x0, y0) = (m.origin_x as i64, m.origin_y as i64);
            let (x1, y1) = (x0 + m.w as i64, y0 + m.h as i64);
            let (px, py) = (x as i64, y as i64);
            let dx = if px < x0 {
                x0 - px
            } else if px >= x1 {
                px - x1 + 1
            } else {
                0
            };
            let dy = if py < y0 {
                y0 - py
            } else if py >= y1 {
                py - y1 + 1
            } else {
                0
            };
            dx * dx + dy * dy
        };
        monitors
            .iter()
            .enumerate()
            .min_by_key(|(_, m)| dist(m))
            .map(|(i, _)| i)
    }

    /// Project a global rect onto the chosen monitor, clipping any parts
    /// that spill off the edges. Returns `None` if the rect does not
    /// intersect any monitor.
    pub fn to_local(monitors: &[MonitorBounds], rect: GlobalRect) -> Option<LocalRect> {
        let i = best_monitor(monitors, rect)?;
        let m = monitors[i];
        let x0 = rect.x.max(m.origin_x);
        let y0 = rect.y.max(m.origin_y);
        let x1 = (rect.x + rect.w as i32).min(m.origin_x + m.w as i32);
        let y1 = (rect.y + rect.h as i32).min(m.origin_y + m.h as i32);
        let w = (x1 - x0).max(0) as u32;
        let h = (y1 - y0).max(0) as u32;
        if w == 0 || h == 0 {
            return None;
        }
        Some(LocalRect {
            monitor_index: i,
            x: (x0 - m.origin_x) as u32,
            y: (y0 - m.origin_y) as u32,
            w,
            h,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn m(origin_x: i32, origin_y: i32, w: u32, h: u32) -> MonitorBounds {
            MonitorBounds {
                origin_x,
                origin_y,
                w,
                h,
            }
        }

        #[test]
        fn point_resolves_to_containing_monitor() {
            let monitors = vec![
                m(0, 0, 1920, 1080),
                m(1920, 0, 2560, 1440),
                m(-1280, 0, 1280, 1024),
            ];
            assert_eq!(monitor_at_point(&monitors, 10, 10), Some(0));
            assert_eq!(monitor_at_point(&monitors, 1920, 500), Some(1));
            assert_eq!(monitor_at_point(&monitors, -1, 500), Some(2));
            // Off every monitor: nearest wins.
            assert_eq!(monitor_at_point(&monitors, 5000, 100), Some(1));
            assert_eq!(monitor_at_point(&[], 0, 0), None);
        }

        #[test]
        fn single_monitor_rect_is_local() {
            let monitors = vec![m(0, 0, 1920, 1080)];
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: 100,
                    y: 200,
                    w: 300,
                    h: 150,
                },
            )
            .unwrap();
            assert_eq!(
                lr,
                LocalRect {
                    monitor_index: 0,
                    x: 100,
                    y: 200,
                    w: 300,
                    h: 150
                }
            );
        }

        #[test]
        fn rect_on_secondary_monitor_to_the_right() {
            let monitors = vec![m(0, 0, 1920, 1080), m(1920, 0, 2560, 1440)];
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: 2000,
                    y: 100,
                    w: 400,
                    h: 300,
                },
            )
            .unwrap();
            assert_eq!(lr.monitor_index, 1);
            assert_eq!(lr.x, 80);
            assert_eq!(lr.y, 100);
            assert_eq!(lr.w, 400);
            assert_eq!(lr.h, 300);
        }

        #[test]
        fn secondary_monitor_with_negative_origin() {
            // Many Linux/Windows setups place a secondary monitor at
            // negative X. A rect on that monitor must come through intact.
            let monitors = vec![m(-1920, 0, 1920, 1080), m(0, 0, 1920, 1080)];
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: -1000,
                    y: 50,
                    w: 200,
                    h: 200,
                },
            )
            .unwrap();
            assert_eq!(lr.monitor_index, 0);
            assert_eq!(lr.x, 920);
        }

        #[test]
        fn rect_straddling_two_monitors_picks_larger_overlap() {
            let monitors = vec![m(0, 0, 1000, 1000), m(1000, 0, 1000, 1000)];
            // 900..1500 → 100 px on monitor 0, 500 px on monitor 1.
            let idx = best_monitor(
                &monitors,
                GlobalRect {
                    x: 900,
                    y: 0,
                    w: 600,
                    h: 100,
                },
            )
            .unwrap();
            assert_eq!(idx, 1);
        }

        #[test]
        fn rect_completely_outside_monitors_returns_none() {
            let monitors = vec![m(0, 0, 1920, 1080)];
            assert!(to_local(
                &monitors,
                GlobalRect {
                    x: 5000,
                    y: 5000,
                    w: 100,
                    h: 100
                }
            )
            .is_none());
        }

        #[test]
        fn rect_clipped_to_monitor_edge() {
            let monitors = vec![m(0, 0, 1000, 1000)];
            // Rect 900..1200 × 900..1200 → clipped to 900..1000.
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: 900,
                    y: 900,
                    w: 300,
                    h: 300,
                },
            )
            .unwrap();
            assert_eq!(lr.w, 100);
            assert_eq!(lr.h, 100);
        }

        #[test]
        fn empty_monitors_list_returns_none() {
            let monitors: Vec<MonitorBounds> = vec![];
            assert!(to_local(
                &monitors,
                GlobalRect {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 100
                }
            )
            .is_none());
            assert!(best_monitor(
                &monitors,
                GlobalRect {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 100
                }
            )
            .is_none());
        }

        #[test]
        fn rect_with_zero_width_does_not_match_any_monitor() {
            let monitors = vec![m(0, 0, 1000, 1000)];
            assert!(to_local(
                &monitors,
                GlobalRect {
                    x: 100,
                    y: 100,
                    w: 0,
                    h: 100
                }
            )
            .is_none());
        }

        #[test]
        fn rect_touching_monitor_edge_has_zero_overlap() {
            // A rect whose right edge is exactly at the monitor's left edge
            // contributes no pixels and should not select that monitor.
            let monitors = vec![m(1000, 0, 1000, 1000)];
            assert!(to_local(
                &monitors,
                GlobalRect {
                    x: 900,
                    y: 0,
                    w: 100,
                    h: 100
                }
            )
            .is_none());
        }

        #[test]
        fn rect_exactly_equal_to_monitor_bounds_is_full_local_rect() {
            let monitors = vec![m(100, 200, 1920, 1080)];
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: 100,
                    y: 200,
                    w: 1920,
                    h: 1080,
                },
            )
            .unwrap();
            assert_eq!(
                lr,
                LocalRect {
                    monitor_index: 0,
                    x: 0,
                    y: 0,
                    w: 1920,
                    h: 1080
                }
            );
        }

        #[test]
        fn tied_overlap_prefers_earlier_monitor_for_determinism() {
            // Two monitors with identical overlap of the rect — the earlier
            // index must win so the selection is stable across runs.
            let monitors = vec![m(0, 0, 1000, 1000), m(0, 0, 1000, 1000)];
            let idx = best_monitor(
                &monitors,
                GlobalRect {
                    x: 100,
                    y: 100,
                    w: 200,
                    h: 200,
                },
            )
            .unwrap();
            assert_eq!(idx, 0);
        }

        #[test]
        fn rect_starting_above_monitor_clips_top_edge() {
            let monitors = vec![m(0, 0, 1000, 1000)];
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: 10,
                    y: -50,
                    w: 100,
                    h: 200,
                },
            )
            .unwrap();
            // Top is clipped to monitor origin; height drops to 150.
            assert_eq!(lr.y, 0);
            assert_eq!(lr.h, 150);
            assert_eq!(lr.x, 10);
        }

        #[test]
        fn rect_inside_monitor_is_not_clipped() {
            let monitors = vec![m(0, 0, 1920, 1080)];
            let lr = to_local(
                &monitors,
                GlobalRect {
                    x: 500,
                    y: 400,
                    w: 300,
                    h: 200,
                },
            )
            .unwrap();
            assert_eq!(lr.w, 300);
            assert_eq!(lr.h, 200);
        }
    }
}

/// Capture from an explicit monitor-local rect.
pub fn capture_rect(r: CaptureRect) -> Result<DynamicImage> {
    let monitors = Monitor::all().context("list monitors")?;
    let monitor = monitors
        .get(r.monitor_index)
        .context("monitor index out of range")?;
    capture_on(monitor, r.x.max(0) as u32, r.y.max(0) as u32, r.w, r.h)
}

/// Read `w`×`h` pixels at (`x`, `y`) of `monitor`.
///
/// Only the region is read from the screen: a window-translation window
/// reads its region several times a second, and copying and converting
/// a whole 4K monitor for a small region each time costs far more than
/// the reading itself. A backend that cannot read a region falls back
/// to the whole monitor.
fn capture_on(monitor: &Monitor, x: u32, y: u32, w: u32, h: u32) -> Result<DynamicImage> {
    if w > 0 && h > 0 {
        if let Ok(region) = monitor.capture_region(x, y, w, h) {
            return Ok(DynamicImage::ImageRgba8(region));
        }
    }
    let full = monitor.capture_image().context("capture")?;
    let dynamic = DynamicImage::ImageRgba8(full);
    let cropped = image::imageops::crop_imm(&dynamic, x, y, w, h).to_image();
    Ok(DynamicImage::ImageRgba8(cropped))
}

/// Capture from a global-coordinate rect. Automatically picks the monitor
/// that contains most of the rect and clips to its edges.
pub fn capture_global_rect(x: i32, y: i32, w: u32, h: u32) -> Result<DynamicImage> {
    let monitors = Monitor::all().context("list monitors")?;
    let bounds: Vec<rect::MonitorBounds> = monitors
        .iter()
        .map(|m| -> Result<rect::MonitorBounds> {
            Ok(rect::MonitorBounds {
                origin_x: m.x()?,
                origin_y: m.y()?,
                w: m.width()?,
                h: m.height()?,
            })
        })
        .collect::<Result<_>>()?;
    let local = rect::to_local(&bounds, rect::GlobalRect { x, y, w, h })
        .context("rect does not intersect any monitor")?;
    let monitor = monitors
        .get(local.monitor_index)
        .context("monitor index out of range")?;
    capture_on(monitor, local.x, local.y, local.w, local.h)
}

pub fn capture_primary_full() -> Result<DynamicImage> {
    let monitors = Monitor::all().context("list monitors")?;
    let monitor = monitors.first().context("no monitor")?;
    let full = monitor.capture_image().context("capture")?;
    Ok(DynamicImage::ImageRgba8(full))
}

/// PNG encode tuned for speed over size: used for full-monitor frames
/// that only travel over local IPC (a 4K frame encodes in ~100 ms
/// instead of ~1 s with default compression).
pub fn png_bytes_fast(img: &image::RgbaImage) -> Result<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Sub).write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(out)
}

pub fn png_bytes(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// Largest image file accepted by "translate an image file" (bytes).
pub const MAX_IMAGE_FILE_BYTES: usize = 40 * 1024 * 1024;

/// Decode an image file (PNG / JPEG / WebP) into the PNG bytes the
/// capture pipeline expects, applying the EXIF orientation so phone
/// photos are not read sideways.
pub fn image_file_png(bytes: &[u8]) -> Result<Vec<u8>> {
    use image::ImageDecoder;
    if bytes.len() > MAX_IMAGE_FILE_BYTES {
        anyhow::bail!("too large");
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    png_bytes(&img)
}

#[cfg(test)]
mod image_file_tests {
    use super::*;

    fn encode(img: &DynamicImage, format: image::ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), format)
            .unwrap();
        out
    }

    #[test]
    fn jpeg_and_png_files_become_png() {
        let img = DynamicImage::new_rgb8(40, 20);
        for format in [image::ImageFormat::Jpeg, image::ImageFormat::Png] {
            let png = image_file_png(&encode(&img, format)).unwrap();
            let back = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
            assert_eq!((back.width(), back.height()), (40, 20));
        }
    }

    #[test]
    fn rejects_non_images_and_oversized_files() {
        assert!(image_file_png(b"not an image").is_err());
        assert!(image_file_png(&vec![0u8; MAX_IMAGE_FILE_BYTES + 1]).is_err());
    }
}
