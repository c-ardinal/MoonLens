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
    tauri_build::build()
}
