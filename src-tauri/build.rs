fn main() {
    // The Vulkan backend of the bundled llama.cpp imports vulkan-1.dll,
    // which only exists where a GPU driver installed it. Delay-load it
    // so the app still starts without it; src/gpu.rs turns the failed
    // load into "no Vulkan devices" and translation runs on the CPU.
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os == "windows" && env == "msvc" && std::env::var_os("CARGO_FEATURE_LOCAL_LLM").is_some() {
        println!("cargo:rustc-link-arg=/DELAYLOAD:vulkan-1.dll");
        println!("cargo:rustc-link-lib=delayimp");
    }

    // On Windows, Tauri links functions that only exist in Common Controls
    // v6, which a binary gets through its manifest. tauri-build embeds that
    // manifest in the app binary only, so the test binaries failed to load
    // (STATUS_ENTRYPOINT_NOT_FOUND). Embed the same manifest in every binary
    // through the linker instead.
    let mut windows = tauri_build::WindowsAttributes::new();
    if env == "msvc" {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
