fn main() {
    // GPU offload fix (Vulkan, Windows/MSVC).
    //
    // The Vulkan backend lives in `ggml-vulkan.lib`, a static archive.
    // It registers itself with ggml's backend registry via C++ static
    // initializers and exposes its compute kernels only through that
    // registry — nothing in the Rust code references those symbols
    // directly. With MSVC static linking the linker therefore keeps
    // only the handful of objects that happen to be referenced and
    // dead-strips the rest of the archive. The backend still registers
    // (model layers get assigned to `Vulkan0`), but a kernel needed to
    // upload tensors to the GPU is missing, so loading the first tensor
    // (`token_embd.weight`) jumps into stripped code and the process
    // dies with STATUS_ACCESS_VIOLATION (0xc0000005) — observed
    // directly in `lens_eval`. Symptom in the app: "GPU is assigned but
    // translation never completes."
    //
    // Force the whole archive in so every Vulkan object survives the
    // link. `llama-cpp-sys-2` already emits the `link-search` for the
    // directory holding `ggml-vulkan.lib`, so the bare name resolves.
    // CPU / base backends are referenced directly and do not need this.
    if std::env::var_os("CARGO_FEATURE_EMBEDDED_LLM_VULKAN").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    {
        println!("cargo:rustc-link-arg-bins=/WHOLEARCHIVE:ggml-vulkan.lib");
    }
    tauri_build::build()
}
