//! GPU support for the built-in llama.cpp engine.
//!
//! Windows and Linux builds include llama.cpp's Vulkan backend, which
//! runs on NVIDIA, AMD and Intel GPUs through the driver that is already
//! installed. macOS uses Metal.
//!
//! On Windows the Vulkan loader (`vulkan-1.dll`) ships with every GPU
//! driver, but a PC without one (a VM, a server, the basic display
//! adapter) does not have it. The executable therefore delay-loads it
//! (see `build.rs`), and the hook below turns a missing loader into a
//! Vulkan instance that reports no devices: llama.cpp then registers
//! no GPU backend and translates on the CPU instead of the process
//! failing to start.

/// Call before the llama.cpp backend first initialises. On Windows it
/// keeps the delay-load hook linked in (nothing else names it).
pub fn prepare() {
    #[cfg(all(windows, target_env = "msvc", feature = "local-llm"))]
    std::hint::black_box(&delay_load::__pfnDliFailureHook2);
}

/// GPU devices llama.cpp can offload to, as "backend: description"
/// (empty when translation runs on the CPU only). Initialises the
/// backend registry on first use.
#[cfg(feature = "local-llm")]
pub fn offload_devices() -> Vec<String> {
    use llama_cpp_2::LlamaBackendDeviceType as T;
    llama_cpp_2::list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|d| matches!(d.device_type, T::Gpu | T::IntegratedGpu))
        .map(|d| format!("{}: {}", d.backend, d.description))
        .collect()
}

#[cfg(not(feature = "local-llm"))]
pub fn offload_devices() -> Vec<String> {
    Vec::new()
}

#[cfg(all(windows, target_env = "msvc", feature = "local-llm"))]
mod delay_load {
    //! Failure hook for the MSVC delay-load helper (`delayimp.lib`).
    //! Layouts follow `delayimp.h`.

    use std::ffi::{c_char, c_void, CStr};

    #[repr(C)]
    pub struct DelayLoadProc {
        import_by_name: i32,
        /// Union of the procedure name and its ordinal.
        name_or_ordinal: *const c_char,
    }

    #[repr(C)]
    pub struct DelayLoadInfo {
        cb: u32,
        pidd: *const c_void,
        pp_iat: *mut *mut c_void,
        dll: *const c_char,
        dlp: DelayLoadProc,
        hmod_cur: *mut c_void,
        pfn_cur: *mut c_void,
        last_error: u32,
    }

    const DLI_FAIL_LOAD_LIB: u32 = 3;
    const DLI_FAIL_GET_PROC: u32 = 4;

    pub type Hook = unsafe extern "system" fn(u32, *mut DelayLoadInfo) -> *mut c_void;

    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    /// Stand-in for `vkGetInstanceProcAddr`: resolves nothing, so the
    /// Vulkan backend sees no Vulkan and declines to register.
    unsafe extern "system" fn no_vulkan_proc(
        _instance: *mut c_void,
        _name: *const c_char,
    ) -> *mut c_void {
        std::ptr::null_mut()
    }

    fn is_vulkan_loader(dll: *const c_char) -> bool {
        !dll.is_null()
            && unsafe { CStr::from_ptr(dll) }
                .to_bytes()
                .eq_ignore_ascii_case(b"vulkan-1.dll")
    }

    unsafe extern "system" fn on_failure(notify: u32, info: *mut DelayLoadInfo) -> *mut c_void {
        let Some(info) = info.as_ref() else {
            return std::ptr::null_mut();
        };
        if !is_vulkan_loader(info.dll) {
            return std::ptr::null_mut();
        }
        match notify {
            // Hand back our own module: its lookups fail too and come
            // back here as DLI_FAIL_GET_PROC.
            DLI_FAIL_LOAD_LIB => GetModuleHandleW(std::ptr::null()),
            DLI_FAIL_GET_PROC
                if info.dlp.import_by_name != 0
                    && !info.dlp.name_or_ordinal.is_null()
                    && CStr::from_ptr(info.dlp.name_or_ordinal).to_bytes()
                        == b"vkGetInstanceProcAddr" =>
            {
                tracing::info!("vulkan-1.dll not found; local models run on the CPU");
                no_vulkan_proc as *mut c_void
            }
            _ => std::ptr::null_mut(),
        }
    }

    /// Picked up by `delayimp.lib` in place of its default (none).
    #[no_mangle]
    #[used]
    #[allow(non_upper_case_globals)]
    pub static __pfnDliFailureHook2: Option<Hook> = Some(on_failure);
}

#[cfg(all(test, feature = "local-llm"))]
mod tests {
    /// The registry initialises whether or not a GPU (or, on Windows,
    /// the Vulkan loader) is present, and the CPU backend is always
    /// there to fall back to.
    #[test]
    fn backend_registry_initialises_with_or_without_a_gpu() {
        let gpus = super::offload_devices();
        eprintln!("offload devices: {gpus:?}");
        let all = llama_cpp_2::list_llama_ggml_backend_devices();
        assert!(
            all.iter()
                .any(|d| d.device_type == llama_cpp_2::LlamaBackendDeviceType::Cpu),
            "no CPU backend in {all:?}"
        );
    }
}
