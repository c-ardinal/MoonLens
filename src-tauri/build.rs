fn main() {
    #[cfg(feature = "local-asr")]
    whisper::build();

    // On Windows, Tauri links functions that only exist in Common Controls
    // v6, which a binary gets through its manifest. tauri-build embeds that
    // manifest in the app binary only, so the test binaries failed to load
    // (STATUS_ENTRYPOINT_NOT_FOUND). Embed the same manifest in every binary
    // through the linker instead.
    let mut windows = tauri_build::WindowsAttributes::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
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

/// Compile the vendored whisper.cpp (speech recognition for live
/// subtitles) against the ggml that `llama-cpp-sys-2` builds for the
/// translation engine. Linking a second ggml would clash symbol by
/// symbol with the first, and the GPU backends (Metal / Vulkan / CUDA)
/// chosen for llama.cpp now serve whisper.cpp too.
#[cfg(feature = "local-asr")]
mod whisper {
    use std::env;
    use std::path::PathBuf;

    pub fn build() {
        println!("cargo:rerun-if-changed=vendor/whisper.cpp");
        println!("cargo:rerun-if-changed=native/whisper_shim.cpp");
        println!("cargo:rerun-if-changed=native/aneforge_stub.cpp");

        // `<prefix>/lib/cmake` (or lib64) of llama-cpp-sys-2's install;
        // the ggml headers sit in `<prefix>/include`.
        let cmake_dir = env::var("DEP_LLAMA_GGML_CMAKE_DIR")
            .expect("llama-cpp-sys-2 did not report its ggml (DEP_LLAMA_GGML_CMAKE_DIR)");
        let prefix = PathBuf::from(cmake_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("ggml cmake dir has a prefix")
            .to_path_buf();
        let ggml_include = prefix.join("include");
        assert!(
            ggml_include.join("ggml.h").exists(),
            "ggml.h not found in {}",
            ggml_include.display()
        );

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

        let mut b = cc::Build::new();
        b.cpp(true)
            .std("c++17")
            // Decoding loops live in whisper.cpp itself; keep them fast in
            // debug builds as well (ggml is already optimized).
            .opt_level(3)
            .warnings(false)
            .include("vendor/whisper.cpp/include")
            .include("vendor/whisper.cpp/src")
            .include(&ggml_include)
            .define("WHISPER_VERSION", "\"1.9.5\"")
            .file("vendor/whisper.cpp/src/whisper.cpp")
            .file("native/whisper_shim.cpp")
            .file("native/aneforge_stub.cpp");
        if msvc {
            b.flag("/utf-8").define("_CRT_SECURE_NO_WARNINGS", None);
        }
        // Only used to size the KV cache padding for flash attention.
        if target_os == "macos" {
            b.define("GGML_USE_METAL", None);
        }
        if env::var("CARGO_FEATURE_LLM_CUDA").is_ok() {
            b.define("GGML_USE_CUDA", None);
        }
        b.compile("moonlens_whisper");
    }
}
