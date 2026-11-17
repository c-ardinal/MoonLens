//! Visual search ("search with an image", Lens' main search mode).
//!
//! A screen selection has no public URL, so the URL-template providers
//! (`browser.google_images`, …) cannot search for it. Instead we write a
//! small local HTML page that carries the image and immediately POSTs it
//! to Google Lens' upload endpoint as a multipart form, then open that
//! page in the default browser. The browser follows the redirect to the
//! results page, so the user sees Lens' own results with their own
//! session. Nothing is uploaded unless the user asks for it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;

/// Google Lens image upload endpoint (multipart field `encoded_image`).
pub const GOOGLE_LENS_UPLOAD: &str = "https://lens.google.com/v3/upload";

/// Longest side sent to the search engine. Lens downsizes anything larger
/// anyway; keeping it bounded keeps the page small and the upload fast.
pub const MAX_SIDE: u32 = 1600;

const PAGE_NAME: &str = "moonlens-visual-search.html";

/// Downscale (if needed) and re-encode the selection as JPEG.
pub fn prepare_image(png: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(png).context("decode selection")?;
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&rgb)
        .context("encode jpeg")?;
    Ok(out)
}

/// The self-submitting page. `now_ms` feeds Lens' `st` parameter.
pub fn upload_page_html(jpeg: &[u8], now_ms: u128) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
    format!(
        r#"<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="referrer" content="no-referrer">
<title>MoonLens — 画像で検索</title>
<style>body{{font-family:system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0;color:#444}}</style>
</head>
<body>
<p id="msg">Google レンズに画像を送信しています…</p>
<form id="f" method="post" enctype="multipart/form-data" action="{action}?ep=ccm&amp;s=&amp;st={now_ms}">
<input id="file" type="file" name="encoded_image" hidden>
<noscript><p>JavaScript を有効にしてください。</p></noscript>
</form>
<script>
(function () {{
  try {{
    var bin = atob("{b64}");
    var bytes = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    var file = new File([bytes], "moonlens.jpg", {{ type: "image/jpeg" }});
    var dt = new DataTransfer();
    dt.items.add(file);
    document.getElementById("file").files = dt.files;
    document.getElementById("f").submit();
  }} catch (e) {{
    document.getElementById("msg").textContent =
      "送信できませんでした。お使いのブラウザでは自動送信に対応していない可能性があります。(" + e + ")";
  }}
}})();
</script>
</body>
</html>
"#,
        action = GOOGLE_LENS_UPLOAD,
    )
}

/// Write the page into `dir` (replacing the previous one) and return its
/// path. A fixed name means at most one image is ever left on disk.
pub fn write_upload_page(dir: &Path, html: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(PAGE_NAME);
    std::fs::write(&path, html).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn small_images_keep_their_size() {
        let jpg = prepare_image(&png(300, 120)).unwrap();
        let img = image::load_from_memory(&jpg).unwrap();
        assert_eq!((img.width(), img.height()), (300, 120));
        assert_eq!(&jpg[..2], &[0xFF, 0xD8], "JPEG magic");
    }

    #[test]
    fn large_images_are_bounded() {
        let jpg = prepare_image(&png(4000, 1000)).unwrap();
        let img = image::load_from_memory(&jpg).unwrap();
        assert_eq!(img.width(), MAX_SIDE);
        assert_eq!(img.height(), 400);
    }

    #[test]
    fn page_posts_the_image_to_lens() {
        let html = upload_page_html(&[1, 2, 3], 42);
        assert!(
            html.contains(r#"action="https://lens.google.com/v3/upload?ep=ccm&amp;s=&amp;st=42""#)
        );
        assert!(html.contains(r#"name="encoded_image""#));
        assert!(html.contains(r#"enctype="multipart/form-data""#));
        assert!(html.contains(r#"atob("AQID")"#));
    }

    #[test]
    fn page_is_written_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = write_upload_page(dir.path(), "a").unwrap();
        let p2 = write_upload_page(dir.path(), "b").unwrap();
        assert_eq!(p1, p2);
        assert_eq!(std::fs::read_to_string(p2).unwrap(), "b");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
