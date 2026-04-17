//! Screen and region capture via `xcap`.

use anyhow::{Context, Result};
use image::DynamicImage;
use xcap::Monitor;

pub struct CaptureRect {
    pub monitor_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub fn capture_rect(rect: CaptureRect) -> Result<DynamicImage> {
    let monitors = Monitor::all().context("list monitors")?;
    let monitor = monitors
        .get(rect.monitor_index)
        .context("monitor index out of range")?;
    let full = monitor.capture_image().context("capture")?;
    let dynamic = DynamicImage::ImageRgba8(full);
    let cropped = image::imageops::crop_imm(&dynamic, rect.x as u32, rect.y as u32, rect.w, rect.h)
        .to_image();
    Ok(DynamicImage::ImageRgba8(cropped))
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
