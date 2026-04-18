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
    // `capture_image` returns a platform-specific `image::RgbaImage` in
    // xcap 0.9; wrap it in `DynamicImage` so the rest of the pipeline can
    // treat every capture source uniformly.
    let full = monitor.capture_image().context("capture")?;
    let dynamic = DynamicImage::ImageRgba8(full);
    let cropped = image::imageops::crop_imm(&dynamic, r.x as u32, r.y as u32, r.w, r.h).to_image();
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
    capture_rect(CaptureRect {
        monitor_index: local.monitor_index,
        x: local.x as i32,
        y: local.y as i32,
        w: local.w,
        h: local.h,
    })
}

pub fn capture_primary_full() -> Result<DynamicImage> {
    let monitors = Monitor::all().context("list monitors")?;
    let monitor = monitors.first().context("no monitor")?;
    let full = monitor.capture_image().context("capture")?;
    Ok(DynamicImage::ImageRgba8(full))
}

pub fn png_bytes(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}
