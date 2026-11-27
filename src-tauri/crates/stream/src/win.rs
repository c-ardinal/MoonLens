//! The composite window on Windows.
//!
//! One thread per composite owns everything: the window, the Direct3D
//! device, the capture of the target window and the drawing. Frames of
//! the target arrive on the GPU (Windows.Graphics.Capture), are copied
//! into the window's back buffer and get the scene drawn over them with
//! Direct2D. Nothing is read back to the CPU.
//!
//! The window sits past the corner of the desktop with one pixel left
//! on it: capture tools list it like any other window, and it covers
//! nothing the user works with. Tools read it through
//! Windows.Graphics.Capture (OBS: capture method "Windows 10 (1903 and
//! up)"; Discord and browsers use it anyway); GDI capture of a window
//! off the screen gets nothing.

use std::mem::ManuallyDrop;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::core::{w, Interface, BOOL, HSTRING, PCWSTR};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, IsWindow,
    IsWindowVisible, PeekMessageW, PostQuitMessage, RegisterClassW, SetWindowPos, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOZORDER,
    SW_SHOWNOACTIVATE, WM_DESTROY, WM_ERASEBKGND, WM_QUIT, WNDCLASSW, WS_EX_APPWINDOW, WS_POPUP,
};
use windows_numerics::{Matrix3x2, Vector2};

use crate::{Align, Error, Item, Scene};

type WinResult<T> = windows::core::Result<T>;

impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error::Os(e.to_string())
    }
}

/// How long the thread sleeps at most when nothing arrives: window
/// messages and a moved translation window are noticed this often.
const IDLE: Duration = Duration::from_millis(16);
/// How long a frame of the new size is waited for after a resize before
/// it is asked for again, and how often at most.
const REFRESH_AFTER: Duration = Duration::from_millis(250);
const REFRESHES: u32 = 8;

/// Padding of the box colour behind text that runs past its box.
const BACK_PAD: f32 = 2.0;

enum Wake {
    Frame,
    Scene,
    Stop,
}

pub struct Running {
    pub window: isize,
    scene: Arc<Mutex<Scene>>,
    wake: Sender<Wake>,
}

impl Running {
    pub fn set_scene(&self, scene: Scene) {
        *self.scene.lock().unwrap_or_else(|e| e.into_inner()) = scene;
        let _ = self.wake.send(Wake::Scene);
    }

    pub fn stop(&self) {
        let _ = self.wake.send(Wake::Stop);
    }
}

pub fn start(
    target: isize,
    title: &str,
    on_end: Box<dyn FnOnce() + Send>,
) -> Result<Running, Error> {
    let scene = Arc::new(Mutex::new(Scene::new()));
    let (wake, woken) = mpsc::channel();
    let (ready, started) = mpsc::channel();
    let (thread_scene, thread_wake, title) = (scene.clone(), wake.clone(), title.to_string());
    std::thread::Builder::new()
        .name("stream".into())
        .spawn(move || {
            // SAFETY: first COM call of this thread.
            let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            match Composite::open(target, &title, thread_wake) {
                Ok(mut composite) => {
                    let _ = ready.send(Ok(composite.window.0 as isize));
                    if let Err(e) = composite.run(&woken, &thread_scene) {
                        eprintln!("stream composite ended: {e}");
                    }
                    composite.close();
                }
                Err(e) => {
                    let _ = ready.send(Err(e));
                }
            }
            on_end();
        })
        .map_err(|e| Error::Os(e.to_string()))?;
    let window = started
        .recv()
        .map_err(|_| Error::Os("composite thread ended early".into()))??;
    Ok(Running {
        window,
        scene,
        wake,
    })
}

/// Back buffers of the flip-model swap chain.
const BUFFERS: u32 = 2;

fn hwnd(handle: isize) -> HWND {
    HWND(handle as *mut core::ffi::c_void)
}

/// The Direct3D device: the GPU, or the software rasterizer where
/// there is none (remote sessions, virtual machines).
fn device() -> WinResult<(ID3D11Device, ID3D11DeviceContext)> {
    let mut last = None;
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let (mut device, mut context) = (None, None);
        // SAFETY: out pointers to locals.
        let made = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        match (made, device, context) {
            (Ok(()), Some(device), Some(context)) => return Ok((device, context)),
            (Err(e), ..) => last = Some(e),
            _ => {}
        }
    }
    Err(last.unwrap_or_else(windows::core::Error::empty))
}

/// Where the frame of `window` is on the screen as drawn (without the
/// invisible resize border): the origin of its captured frames.
fn frame_origin(window: HWND) -> Option<(i32, i32)> {
    let mut r = RECT::default();
    // SAFETY: `r` has the size passed along.
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut r).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    }
    .ok()?;
    Some((r.left, r.top))
}

/// Client area of `window` on the screen: `[x, y, w, h]`. `None` while
/// it is hidden.
fn client_rect(window: HWND) -> Option<[i32; 4]> {
    let mut origin = POINT::default();
    let mut size = RECT::default();
    // SAFETY: pointers to locals; a stale handle makes the calls fail.
    unsafe {
        if !IsWindowVisible(window).as_bool() || !ClientToScreen(window, &mut origin).as_bool() {
            return None;
        }
        GetClientRect(window, &mut size).ok()?;
    }
    Some([origin.x, origin.y, size.right, size.bottom])
}

/// A spot past the corner of the desktop: the bottom-right pixel of
/// the monitor that reaches furthest right (the lowest of those), so
/// that all but one pixel of a window placed there is on no monitor.
fn parking_spot() -> (i32, i32) {
    unsafe extern "system" fn each(monitor: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: `out` is the `&mut (i32, i32)` passed below.
        unsafe {
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let best = &mut *(out.0 as *mut (i32, i32));
                let r = info.rcMonitor;
                if (r.right, r.bottom) > *best {
                    *best = (r.right, r.bottom);
                }
            }
        }
        BOOL(1)
    }
    let mut best = (i32::MIN, i32::MIN);
    // SAFETY: the callback only runs during the call.
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(each), LPARAM(&raw mut best as isize)) };
    if best.0 == i32::MIN {
        return (0, 0);
    }
    (best.0 - 1, best.1 - 1)
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        // Closed by the user (taskbar, Alt+F4): the composite ends.
        WM_DESTROY => {
            // SAFETY: plain call.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // Every pixel comes from the swap chain.
        WM_ERASEBKGND => LRESULT(1),
        // SAFETY: forwards the arguments as received.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn create_window(title: &str, width: i32, height: i32) -> WinResult<HWND> {
    const CLASS: PCWSTR = w!("MoonLensStream");
    // SAFETY: the class and its procedure live as long as the process;
    // registering it a second time fails harmlessly.
    unsafe {
        let module = GetModuleHandleW(None)?;
        RegisterClassW(&WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            ..Default::default()
        });
        let (x, y) = parking_spot();
        let window = CreateWindowExW(
            WS_EX_APPWINDOW,
            CLASS,
            &HSTRING::from(title),
            WS_POPUP,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(module.into()),
            None,
        )?;
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        Ok(window)
    }
}

struct Composite {
    target: HWND,
    window: HWND,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt_device: IDirect3DDevice,
    swap_chain: IDXGISwapChain1,
    d2d: ID2D1DeviceContext,
    dwrite: IDWriteFactory,
    brush: ID2D1SolidColorBrush,
    /// The swap chain's back buffer as Direct2D target.
    canvas: Option<ID2D1Bitmap1>,
    back_buffer: Option<ID3D11Texture2D>,
    /// The last frame of the target: drawn again when only the scene
    /// changes (a window at rest sends no frames).
    held: Option<ID3D11Texture2D>,
    /// Set while the held frame covers only a part of the new size
    /// (see `refresh`): since when, and how often a frame was asked for.
    partial: Option<(Instant, u32)>,
    size: SizeInt32,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    _item: GraphicsCaptureItem,
}

impl Composite {
    fn open(target: isize, title: &str, wake: Sender<Wake>) -> Result<Self, Error> {
        let target = hwnd(target);
        // SAFETY: a stale handle makes the call return false.
        if !unsafe { IsWindow(Some(target)) }.as_bool() {
            return Err(Error::TargetGone);
        }
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err(Error::Unsupported);
        }
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: `target` was a window a moment ago; the call fails
        // for one that is gone or cannot be captured.
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(target) }?;
        let size = usable(item.Size()?);

        let (device, context) = device()?;
        let dxgi: IDXGIDevice = device.cast()?;
        // SAFETY: `dxgi` is a live device.
        let winrt_device: IDirect3DDevice =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }?.cast()?;

        let window = create_window(title, size.Width, size.Height)?;
        let opened = (|| -> Result<Self, Error> {
            // SAFETY: plain COM calls on live objects.
            let factory: IDXGIFactory2 = unsafe { dxgi.GetAdapter()?.GetParent() }?;
            // A flip-model swap chain: the compositor takes its
            // buffers whole, so the part of the window past the edge
            // of the desktop is there for Windows.Graphics.Capture. A
            // blit-model one is copied to the window only where it is
            // on a screen (measured: black off-screen). GDI capture
            // (`BitBlt`) gets neither while the window is parked.
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size.Width as u32,
                Height: size.Height as u32,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: BUFFERS,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                ..Default::default()
            };
            // SAFETY: `desc` outlives the call.
            let swap_chain =
                unsafe { factory.CreateSwapChainForHwnd(&device, window, &desc, None, None) }?;

            // SAFETY: plain factory calls.
            let d2d = unsafe {
                let factory: ID2D1Factory1 =
                    D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
                factory
                    .CreateDevice(&dxgi)?
                    .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
            };
            // Scene coordinates are physical pixels.
            // SAFETY: plain calls.
            let (dwrite, brush) = unsafe {
                d2d.SetDpi(96.0, 96.0);
                let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
                let brush = d2d.CreateSolidColorBrush(&color([0, 0, 0]), None)?;
                (dwrite, brush)
            };

            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &winrt_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                size,
            )?;
            let session = pool.CreateCaptureSession(&item)?;
            // The yellow capture frame around the target; older
            // Windows versions cannot switch it off.
            let _ = session.SetIsBorderRequired(false);
            let frames = wake.clone();
            pool.FrameArrived(&TypedEventHandler::new(move |_, _| {
                let _ = frames.send(Wake::Frame);
                Ok(())
            }))?;
            let closed = wake.clone();
            item.Closed(&TypedEventHandler::new(move |_, _| {
                let _ = closed.send(Wake::Stop);
                Ok(())
            }))?;
            session.StartCapture()?;

            let mut composite = Self {
                target,
                window,
                device,
                context,
                winrt_device,
                swap_chain,
                d2d,
                dwrite,
                brush,
                canvas: None,
                back_buffer: None,
                held: None,
                partial: None,
                size,
                pool,
                session,
                _item: item,
            };
            composite.bind()?;
            Ok(composite)
        })();
        if opened.is_err() {
            // SAFETY: the window was created above on this thread.
            let _ = unsafe { DestroyWindow(window) };
        }
        opened
    }

    /// Take the back buffer as copy destination and drawing target.
    fn bind(&mut self) -> WinResult<()> {
        // SAFETY: plain COM calls; `properties` outlives the call.
        unsafe {
            let buffer: ID3D11Texture2D = self.swap_chain.GetBuffer(0)?;
            let surface: IDXGISurface = buffer.cast()?;
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                colorContext: ManuallyDrop::new(None),
            };
            let canvas = self
                .d2d
                .CreateBitmapFromDxgiSurface(&surface, Some(&properties))?;
            self.d2d.SetTarget(&canvas);
            self.canvas = Some(canvas);
            self.back_buffer = Some(buffer);
        }
        Ok(())
    }

    /// The target changed its size: window, buffers and capture follow.
    fn resize(&mut self, size: SizeInt32) -> WinResult<()> {
        self.size = size;
        self.held = None;
        self.canvas = None;
        self.back_buffer = None;
        // SAFETY: every reference to the back buffer was dropped above.
        unsafe {
            self.d2d.SetTarget(None);
            self.context.ClearState();
            self.swap_chain.ResizeBuffers(
                BUFFERS,
                size.Width as u32,
                size.Height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
            let (x, y) = parking_spot();
            let _ = SetWindowPos(
                self.window,
                None,
                x,
                y,
                size.Width,
                size.Height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        self.bind()?;
        self.pool.Recreate(
            &self.winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
    }

    /// Take the newest frame the capture has, if any. `Ok(true)` when
    /// there was one.
    fn take_frame(&mut self) -> WinResult<bool> {
        let mut newest = None;
        while let Ok(frame) = self.pool.TryGetNextFrame() {
            newest = Some(frame);
        }
        let Some(frame) = newest else {
            return Ok(false);
        };
        let content = usable(frame.ContentSize()?);
        if content != self.size {
            self.resize(content)?;
        }
        let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
        // SAFETY: plain COM calls; the descriptors outlive the calls.
        unsafe {
            let texture: ID3D11Texture2D = access.GetInterface()?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            if self.held.is_none() {
                let mut held = None;
                self.device.CreateTexture2D(
                    &D3D11_TEXTURE2D_DESC {
                        Width: self.size.Width as u32,
                        Height: self.size.Height as u32,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                        ..Default::default()
                    },
                    None,
                    Some(&mut held),
                )?;
                self.held = held;
            }
            let Some(held) = &self.held else {
                return Ok(false);
            };
            // Mid-resize the frame's texture still has the old size.
            let area = D3D11_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: desc.Width.min(self.size.Width as u32),
                bottom: desc.Height.min(self.size.Height as u32),
                back: 1,
            };
            self.context
                .CopySubresourceRegion(held, 0, 0, 0, 0, &texture, 0, Some(&area));
            let whole =
                desc.Width >= self.size.Width as u32 && desc.Height >= self.size.Height as u32;
            self.partial = match (whole, self.partial) {
                (true, _) => None,
                (false, None) => Some((Instant::now(), 0)),
                (false, waiting) => waiting,
            };
        }
        Ok(true)
    }

    /// After a resize the held frame has the old size until the capture
    /// sends one from its new buffers. A target at rest may never send
    /// it, which would leave the new part of the composite empty: ask
    /// for a frame again (recreating the buffers makes the capture send
    /// one), a few times at most.
    fn refresh(&mut self) -> WinResult<()> {
        let Some((since, asked)) = self.partial else {
            return Ok(());
        };
        if asked >= REFRESHES || since.elapsed() < REFRESH_AFTER {
            return Ok(());
        }
        self.partial = Some((Instant::now(), asked + 1));
        self.pool.Recreate(
            &self.winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            self.size,
        )
    }

    /// Draw the held frame and the scene over it, and show the result.
    fn render(&mut self, scene: &Scene) -> WinResult<()> {
        let (Some(held), Some(back_buffer)) = (&self.held, &self.back_buffer) else {
            return Ok(());
        };
        let origin = frame_origin(self.target);
        // SAFETY: plain COM calls on objects of this thread.
        unsafe {
            self.context.CopyResource(back_buffer, held);
            self.d2d.BeginDraw();
            if let Some((left, top)) = origin {
                for layer in scene {
                    let Some([x, y, w, h]) = client_rect(hwnd(layer.window)) else {
                        continue;
                    };
                    let (dx, dy) = ((x - left) as f32, (y - top) as f32);
                    self.d2d.SetTransform(&Matrix3x2::translation(dx, dy));
                    self.d2d.PushAxisAlignedClip(
                        &rect(0.0, 0.0, w as f32, h as f32),
                        D2D1_ANTIALIAS_MODE_ALIASED,
                    );
                    for item in &layer.items {
                        // One item that cannot be drawn (a font that
                        // fails to load) does not cost the others.
                        let _ = self.draw(item, dx, dy);
                    }
                    self.d2d.PopAxisAlignedClip();
                }
            }
            self.d2d.SetTransform(&Matrix3x2::identity());
            self.d2d.EndDraw(None, None)?;
            self.swap_chain.Present(0, DXGI_PRESENT(0)).ok()
        }
    }

    /// Draw one item; (`dx`, `dy`) is the layer's place in the frame.
    ///
    /// # Safety
    /// Between `BeginDraw` and `EndDraw` of `self.d2d`.
    unsafe fn draw(&self, item: &Item, dx: f32, dy: f32) -> WinResult<()> {
        let [x, y, w, h] = item.rect;
        // SAFETY: see above; all arguments outlive the calls.
        unsafe {
            self.brush.SetColor(&color(item.background));
            if item.fills.is_empty() {
                self.d2d.FillRectangle(&rect(x, y, w, h), &self.brush);
            }
            for &[x, y, w, h] in &item.fills {
                self.d2d.FillRectangle(&rect(x, y, w, h), &self.brush);
            }
            if item.lines.is_empty() || item.font_px <= 0.0 {
                return Ok(());
            }
            let format: IDWriteTextFormat = self.dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                if item.bold {
                    DWRITE_FONT_WEIGHT_BOLD
                } else {
                    DWRITE_FONT_WEIGHT_MEDIUM
                },
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                item.font_px,
                w!(""),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            // Text is cut at the top and bottom of its box, and may
            // run past it to the right.
            self.d2d
                .PushAxisAlignedClip(&rect(-1.0e6, y, 2.0e6, h), D2D1_ANTIALIAS_MODE_ALIASED);
            for line in &item.lines {
                let text: Vec<u16> = line.text.encode_utf16().collect();
                let [lx, ly, lw, lh] = line.rect;
                let layout = self
                    .dwrite
                    .CreateTextLayout(&text, &format, 1.0e6, lh.max(1.0))?;
                let mut metrics = DWRITE_TEXT_METRICS::default();
                layout.GetMetrics(&mut metrics)?;
                if metrics.width <= 0.0 {
                    continue;
                }
                // The line breaks were made for the translation
                // window's font: a wider font here is narrowed to the
                // same width rather than run past the line's end.
                let squeeze = if line.text_width > 0.0 {
                    (line.text_width / metrics.width).min(1.0)
                } else {
                    1.0
                };
                let width = metrics.width * squeeze;
                let left = match item.align {
                    Align::Left => lx,
                    Align::Center => lx + (lw - width) / 2.0,
                    Align::Right => lx + lw - width,
                };
                let top = ly + (lh - metrics.height) / 2.0;
                if line.backed {
                    self.brush.SetColor(&color(item.background));
                    self.d2d
                        .FillRectangle(&rect(left, ly, width + BACK_PAD, lh), &self.brush);
                }
                self.brush.SetColor(&color(item.foreground));
                self.d2d.SetTransform(
                    &(Matrix3x2 {
                        M11: squeeze,
                        M12: 0.0,
                        M21: 0.0,
                        M22: 1.0,
                        M31: dx + left,
                        M32: dy + top,
                    }),
                );
                self.d2d.DrawTextLayout(
                    Vector2 { X: 0.0, Y: 0.0 },
                    &layout,
                    &self.brush,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                );
                self.d2d.SetTransform(&Matrix3x2::translation(dx, dy));
            }
            self.d2d.PopAxisAlignedClip();
        }
        Ok(())
    }

    /// Until told to stop, the target closes or the window is closed.
    fn run(&mut self, woken: &Receiver<Wake>, scene: &Mutex<Scene>) -> WinResult<()> {
        // Where the layers were at the last frame: a translation
        // window that moved is drawn at its new place.
        let mut placed: Vec<Option<[i32; 4]>> = Vec::new();
        loop {
            let mut message = MSG::default();
            // SAFETY: `message` outlives the calls.
            unsafe {
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    if message.message == WM_QUIT {
                        return Ok(());
                    }
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            let mut dirty = false;
            match woken.recv_timeout(IDLE) {
                Ok(Wake::Stop) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
                Ok(Wake::Frame | Wake::Scene) => dirty = true,
                Err(RecvTimeoutError::Timeout) => {}
            }
            // Whatever else queued up meanwhile is covered by one frame.
            for wake in woken.try_iter() {
                if matches!(wake, Wake::Stop) {
                    return Ok(());
                }
                dirty = true;
            }
            // SAFETY: a stale handle makes the call return false.
            if !unsafe { IsWindow(Some(self.target)) }.as_bool() {
                return Ok(());
            }
            if self.take_frame()? {
                dirty = true;
            }
            self.refresh()?;
            let scene = scene.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let now: Vec<Option<[i32; 4]>> = scene
                .iter()
                .map(|layer| client_rect(hwnd(layer.window)))
                .collect();
            if dirty || now != placed {
                placed = now;
                self.render(&scene)?;
            }
        }
    }

    fn close(self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
        // SAFETY: the window belongs to this thread.
        let _ = unsafe { DestroyWindow(self.window) };
    }
}

/// Buffers cannot have an empty side (a target collapsed to nothing).
fn usable(size: SizeInt32) -> SizeInt32 {
    SizeInt32 {
        Width: size.Width.max(1),
        Height: size.Height.max(1),
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}

fn color([r, g, b]: [u8; 3]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: 1.0,
    }
}

#[cfg(test)]
#[path = "win_tests.rs"]
mod tests;
