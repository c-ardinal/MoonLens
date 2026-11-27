//! Windows half of `trigger_watch.rs`: raw keyboard / mouse input through
//! a message-only window, and XInput gamepad polling. Depends on
//! `trigger.rs` only.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    PostQuitMessage, RegisterClassW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_DESTROY, WM_INPUT, WNDCLASSW,
};

use crate::trigger::{raw, InputEvent, Modifier, Mods, PadButtons, Sink, Sources, PAD_SLOTS};

/// Gamepad polling interval: one frame at 60 Hz.
const PAD_POLL: Duration = Duration::from_millis(16);
/// Empty slots are asked again every this many polls (about 2 s):
/// XInput is slow on a slot with no gamepad.
const PAD_RESCAN: u32 = 120;

pub struct Watch {
    /// Raw input thread and the handle of its message-only window.
    raw: Option<(isize, JoinHandle<()>)>,
    pad: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
}

impl Watch {
    pub fn start(sources: Sources, sink: Sink) -> Self {
        let raw = (sources.keyboard || sources.mouse)
            .then(|| {
                let (ready_tx, ready_rx) = mpsc::channel();
                let sink = sink.clone();
                let thread = std::thread::Builder::new()
                    .name("trigger-raw-input".into())
                    .spawn(move || run_raw(sources, sink, ready_tx))
                    .ok()?;
                // No window: the thread has ended by itself.
                let hwnd = ready_rx.recv().ok()?;
                Some((hwnd, thread))
            })
            .flatten();
        let pad = sources
            .pad
            .then(|| {
                let stop = Arc::new(AtomicBool::new(false));
                let stop_t = stop.clone();
                let thread = std::thread::Builder::new()
                    .name("trigger-gamepad".into())
                    .spawn(move || run_pad(sink, stop_t))
                    .ok()?;
                Some((stop, thread))
            })
            .flatten();
        Watch { raw, pad }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some((hwnd, thread)) = self.raw.take() {
            let posted = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut c_void)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
            // Without the message the thread never ends: leave it.
            if posted.is_ok() {
                let _ = thread.join();
            }
        }
        if let Some((stop, thread)) = self.pad.take() {
            stop.store(true, Ordering::Relaxed);
            let _ = thread.join();
        }
    }
}

thread_local! {
    /// Where the raw input thread's window procedure sends events.
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

/// Message loop of a message-only window that receives raw keyboard
/// and mouse input while other applications are in front.
fn run_raw(sources: Sources, sink: Sink, ready: mpsc::Sender<isize>) {
    let class_name = w!("MoonLensTriggerWatch");
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        // Fails when registered by an earlier run of this thread: fine.
        RegisterClassW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )
    };
    let hwnd = match hwnd {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("trigger watch: no message window: {e}");
            return;
        }
    };
    // Generic desktop page: mouse = usage 2, keyboard = usage 6.
    let device = |usage| RAWINPUTDEVICE {
        usUsagePage: 1,
        usUsage: usage,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    };
    let mut devices = Vec::new();
    if sources.mouse {
        devices.push(device(2));
    }
    if sources.keyboard {
        devices.push(device(6));
    }
    if let Err(e) = unsafe { RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32) }
    {
        tracing::warn!("trigger watch: raw input not registered: {e}");
    }
    SINK.with(|s| *s.borrow_mut() = Some(sink));
    let _ = ready.send(hwnd.0 as isize);
    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        unsafe { DispatchMessageW(&msg) };
    }
    SINK.with(|s| *s.borrow_mut() = None);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            on_input(HRAWINPUT(lparam.0 as *mut c_void));
            // The system frees the input data in its default handling.
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn on_input(handle: HRAWINPUT) {
    let mut input = RAWINPUT::default();
    let mut size = size_of::<RAWINPUT>() as u32;
    let read = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(&mut input as *mut RAWINPUT as *mut c_void),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if read == 0 || read == u32::MAX {
        return;
    }
    let kind = input.header.dwType;
    let events: Vec<InputEvent> = if kind == RIM_TYPEMOUSE.0 {
        let flags = unsafe { input.data.mouse.Anonymous.Anonymous.usButtonFlags };
        // Almost every mouse message is a movement.
        if flags == 0 {
            return;
        }
        let events = raw::mouse(flags, Mods::NONE);
        if events.is_empty() {
            return;
        }
        let mods = held_mods();
        events
            .into_iter()
            .map(|e| match e {
                InputEvent::Mouse { button, down, .. } => InputEvent::Mouse { button, down, mods },
                other => other,
            })
            .collect()
    } else if kind == RIM_TYPEKEYBOARD.0 {
        let keyboard = unsafe { input.data.keyboard };
        raw::key(keyboard.VKey, keyboard.Flags)
            .into_iter()
            .collect()
    } else {
        return;
    };
    SINK.with(|s| {
        if let Some(sink) = &*s.borrow() {
            for e in events {
                sink(e);
            }
        }
    });
}

fn held_mods() -> Mods {
    // VK_CONTROL, VK_MENU, VK_SHIFT.
    [
        (0x11, Modifier::Control),
        (0x12, Modifier::Alt),
        (0x10, Modifier::Shift),
    ]
    .into_iter()
    .filter(|(vk, _)| unsafe { GetAsyncKeyState(*vk) } < 0)
    .fold(Mods::NONE, |mods, (_, m)| mods.with(m))
}

/// `XINPUT_STATE`.
#[repr(C)]
#[derive(Default)]
struct XInputState {
    packet: u32,
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumbs: [i16; 4],
}

type XInputGetState = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

/// Poll the XInput gamepads and report every change of their buttons.
/// The library is loaded at run time: a PC without it only loses the
/// gamepad triggers.
fn run_pad(sink: Sink, stop: Arc<AtomicBool>) {
    let lib = match unsafe { libloading::Library::new("xinput1_4.dll") } {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("trigger watch: XInput not available: {e}");
            return;
        }
    };
    let get_state = match unsafe { lib.get::<XInputGetState>(b"XInputGetState\0") } {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("trigger watch: XInput not available: {e}");
            return;
        }
    };
    // `None`: no gamepad in the slot.
    let mut last = [None::<PadButtons>; PAD_SLOTS];
    let mut tick = 0u32;
    while !stop.load(Ordering::Relaxed) {
        for (slot, last) in last.iter_mut().enumerate() {
            if last.is_none() && !tick.is_multiple_of(PAD_RESCAN) {
                continue;
            }
            let mut state = XInputState::default();
            let connected = unsafe { get_state(slot as u32, &mut state) } == 0;
            let now = connected.then(|| {
                PadButtons::from_xinput(state.buttons, state.left_trigger, state.right_trigger)
            });
            if now != *last {
                *last = now;
                sink(InputEvent::Pad {
                    slot,
                    buttons: now.unwrap_or(PadButtons::NONE),
                });
            }
        }
        tick = tick.wrapping_add(1);
        std::thread::sleep(PAD_POLL);
    }
}
