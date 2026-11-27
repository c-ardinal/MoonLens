//! The composite window on a real desktop: a window of ours stands in
//! for the other app, a second one for a translation window lying over
//! it, and the composite is read back the way capture tools read it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, GetWindowRect, WS_EX_TOPMOST, WS_VISIBLE,
};

use super::*;
use crate::{Compositor, Layer, Line};

unsafe extern "system" fn plain_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwards the arguments as received.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// A plain window filled with one colour, with its own message loop.
fn colour_window(class: &'static str, rgb: [u8; 3], topmost: bool, at: [i32; 4]) -> isize {
    let (ready, made) = mpsc::channel();
    std::thread::spawn(move || {
        // SAFETY: plain window creation; the loop ends with the process.
        unsafe {
            let class = &HSTRING::from(class);
            let module = GetModuleHandleW(None).unwrap();
            let colour = u32::from(rgb[0]) | u32::from(rgb[1]) << 8 | u32::from(rgb[2]) << 16;
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(plain_proc),
                hInstance: module.into(),
                lpszClassName: PCWSTR(class.as_ptr()),
                hbrBackground: CreateSolidBrush(COLORREF(colour)),
                ..Default::default()
            });
            let window = CreateWindowExW(
                if topmost {
                    WS_EX_TOPMOST
                } else {
                    Default::default()
                },
                class,
                class,
                WS_POPUP | WS_VISIBLE,
                at[0],
                at[1],
                at[2],
                at[3],
                None,
                None,
                Some(module.into()),
                None,
            )
            .unwrap();
            ready.send(window.0 as isize).unwrap();
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                DispatchMessageW(&message);
            }
        }
    });
    made.recv().unwrap()
}

struct Picture {
    width: usize,
    height: usize,
    /// BGRA rows.
    data: Vec<u8>,
}

impl Picture {
    /// Mean colour (RGB) of the 5×5 patch around (`x`, `y`).
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let mut sum = [0u32; 3];
        for py in y - 2..=y + 2 {
            for px in x - 2..=x + 2 {
                let i = (py * self.width + px) * 4;
                sum[0] += u32::from(self.data[i + 2]);
                sum[1] += u32::from(self.data[i + 1]);
                sum[2] += u32::from(self.data[i]);
            }
        }
        sum.map(|v| (v / 25) as u8)
    }

    /// Whether some pixel of the area is close to white.
    fn has_white(&self, [x, y, w, h]: [usize; 4]) -> bool {
        (y..y + h).any(|py| {
            (x..x + w).any(|px| {
                let i = (py * self.width + px) * 4;
                self.data[i..i + 3].iter().all(|&v| v > 215)
            })
        })
    }
}

/// One frame of `window` through Windows.Graphics.Capture: what OBS's
/// and Discord's window capture get.
fn capture(window: isize) -> Picture {
    let (device, context) = device().unwrap();
    let dxgi: IDXGIDevice = device.cast().unwrap();
    // SAFETY: plain COM calls on live objects; descriptors outlive them.
    unsafe {
        let winrt: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
            .unwrap()
            .cast()
            .unwrap();
        let interop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().unwrap();
        let item: GraphicsCaptureItem = interop.CreateForWindow(hwnd(window)).unwrap();
        let size = item.Size().unwrap();
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            size,
        )
        .unwrap();
        let session = pool.CreateCaptureSession(&item).unwrap();
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().unwrap();
        let started = Instant::now();
        let frame = loop {
            if let Ok(frame) = pool.TryGetNextFrame() {
                break frame;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "no frame of the composite"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let access: IDirect3DDxgiInterfaceAccess = frame.Surface().unwrap().cast().unwrap();
        let texture: ID3D11Texture2D = access.GetInterface().unwrap();
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .unwrap();
        let staging = staging.unwrap();
        context.CopyResource(&staging, &texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        let (width, height) = (desc.Width as usize, desc.Height as usize);
        let mut data = Vec::with_capacity(width * height * 4);
        for row in 0..height {
            let start = (mapped.pData as *const u8).add(row * mapped.RowPitch as usize);
            data.extend_from_slice(std::slice::from_raw_parts(start, width * 4));
        }
        context.Unmap(&staging, 0);
        let _ = session.Close();
        let _ = pool.Close();
        Picture {
            width,
            height,
            data,
        }
    }
}

fn close_to(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= 24)
}

/// Read the composite until `ok` holds for it (the first frames may
/// predate a change), or fail with what the last reading showed.
fn settle(window: isize, what: &str, ok: impl Fn(&Picture) -> bool) -> Picture {
    let started = Instant::now();
    loop {
        let picture = capture(window);
        if ok(&picture) {
            return picture;
        }
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "{what}: {}x{}, corner {:?}, far corner {:?}",
            picture.width,
            picture.height,
            picture.at(10, 10),
            picture.at(picture.width - 20, picture.height - 20)
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

const BLUE: [u8; 3] = [0, 0, 200];
const GREEN: [u8; 3] = [0, 180, 0];
const RED: [u8; 3] = [200, 0, 0];

#[test]
fn the_composite_shows_the_target_with_the_translations_over_it() {
    // The app to stream, and a translation window lying over it.
    let target = colour_window("MoonLensStreamTestTarget", BLUE, false, [60, 60, 400, 300]);
    let lens = colour_window("MoonLensStreamTestLens", GREEN, true, [110, 140, 240, 120]);
    std::thread::sleep(Duration::from_millis(500));

    static ENDED: AtomicBool = AtomicBool::new(false);
    let composite = Compositor::start(target, "MoonLens 配信 - test", || {
        ENDED.store(true, Ordering::SeqCst);
    })
    .expect("composite window");
    let window = composite.window();

    // Like capture tools list it: a visible top-level window.
    // SAFETY: plain calls with a live handle.
    unsafe {
        assert!(IsWindowVisible(hwnd(window)).as_bool());
        let mut bounds = RECT::default();
        GetWindowRect(hwnd(window), &mut bounds).unwrap();
        println!("composite window at {bounds:?}");
    }

    // Without translations: the target alone. The translation window
    // lying over it on the screen is not in the picture.
    let plain = settle(window, "target not shown", |p| {
        p.width == 400 && p.height == 300 && close_to(p.at(10, 10), BLUE)
    });
    assert!(
        close_to(plain.at(170, 140), BLUE),
        "{:?}",
        plain.at(170, 140)
    );

    // A translated paragraph at (20, 20) of the translation window,
    // which is at (50, 80) of the target: (70, 100) of the composite.
    composite.set_scene(vec![Layer {
        window: lens,
        items: vec![Item {
            rect: [20.0, 20.0, 200.0, 60.0],
            fills: Vec::new(),
            background: RED,
            foreground: [255, 255, 255],
            bold: false,
            font_px: 32.0,
            align: Align::Left,
            lines: vec![Line {
                text: "翻訳 OK".into(),
                rect: [20.0, 20.0, 200.0, 60.0],
                text_width: 200.0,
                backed: false,
            }],
        }],
    }]);
    let translated = settle(window, "translation box not shown", |p| {
        close_to(p.at(262, 104), RED)
    });
    assert!(close_to(translated.at(10, 10), BLUE));
    assert!(
        close_to(translated.at(60, 90), BLUE),
        "outside the box: {:?}",
        translated.at(60, 90)
    );
    assert!(close_to(translated.at(280, 170), BLUE));
    assert!(
        translated.has_white([70, 100, 200, 60]),
        "no text in the box"
    );
    assert!(
        !translated.has_white([0, 0, 400, 95]),
        "text outside the box"
    );

    // The translation window moves: its translations move along.
    // SAFETY: plain call with a live handle.
    unsafe {
        SetWindowPos(
            hwnd(lens),
            None,
            210,
            240,
            240,
            120,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .unwrap();
    }
    let moved = settle(window, "translation did not follow its window", |p| {
        close_to(p.at(362, 204), RED) && close_to(p.at(262, 104), BLUE)
    });
    assert!(moved.has_white([170, 200, 200, 60]));

    // The target is resized: the composite takes the new size.
    // SAFETY: as above.
    unsafe {
        SetWindowPos(
            hwnd(target),
            None,
            60,
            60,
            520,
            360,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .unwrap();
    }
    settle(window, "composite did not take the new size", |p| {
        p.width == 520 && p.height == 360 && close_to(p.at(500, 340), BLUE)
    });

    // Stopped: the window is gone and the owner hears about it.
    composite.stop();
    let started = Instant::now();
    while !ENDED.load(Ordering::SeqCst) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "composite did not end"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // SAFETY: a stale handle makes the call return false.
    assert!(!unsafe { IsWindow(Some(hwnd(window))) }.as_bool());
}

#[test]
fn a_window_that_is_gone_cannot_be_mirrored() {
    assert!(matches!(
        Compositor::start(0x7fff_fff0, "x", || {}),
        Err(Error::TargetGone)
    ));
}
