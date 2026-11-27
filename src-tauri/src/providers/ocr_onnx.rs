//! ONNX Runtime for the PP-OCR models: the faster way to run them.
//!
//! The networks [`super::ocr_ppocr`] runs on rten, on ONNX Runtime
//! instead: on the CPU everywhere, and through DirectML on the GPU on
//! Windows. Which of them reads a line fastest differs from PC to PC,
//! so [`super::ocr_ppocr::engine`] measures the ones that load and
//! keeps the fastest (see `PpOcr::load`).
//!
//! ONNX Runtime is not linked in: its library is fetched on request
//! with the OCR models ([`install`]) and checked against a pinned
//! SHA-256. Without it, or where the library does not load, OCR runs
//! on rten.

use std::path::{Path, PathBuf};

/// ONNX Runtime release the app loads. The `api-NN` feature of the
/// `ort` crate must name the same minor version.
pub const RUNTIME_VERSION: &str = "1.24.4";

/// A file fetched as it is and stored under `file`.
#[derive(Debug)]
pub struct Remote {
    pub url: &'static str,
    pub sha256: &'static str,
    pub file: &'static str,
}

impl Remote {
    /// Whether the file is in `models_dir`.
    pub fn present(&self, models_dir: &Path) -> bool {
        present(&models_dir.join(self.file))
    }

    /// Download the file into `models_dir` unless it is there.
    pub async fn install(
        &'static self,
        models_dir: &Path,
        progress: Option<&super::ocr_ocrs::ProgressCb>,
    ) -> anyhow::Result<()> {
        if self.present(models_dir) {
            return Ok(());
        }
        let bytes = fetch(&reqwest::Client::new(), self.url, self.file, progress).await?;
        let dir = models_dir.to_path_buf();
        tokio::task::spawn_blocking(move || verify_and_store(&bytes, self, &dir)).await??;
        Ok(())
    }
}

/// The ONNX Runtime build for one platform: a zip archive (the
/// official Python wheel, the one distribution that carries a single
/// platform's libraries and, on Windows, DirectML with them) and the
/// members taken from it.
pub struct RuntimePackage {
    pub url: &'static str,
    pub sha256: &'static str,
    /// `(path in the archive, file name on disk)`.
    pub members: &'static [(&'static str, &'static str)],
    /// The member ONNX Runtime is loaded from.
    pub library: &'static str,
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub const RUNTIME: Option<RuntimePackage> = Some(RuntimePackage {
    url: "https://files.pythonhosted.org/packages/88/ea/33814eb0ec96775eda4c1d30b0d86e91d7d2cd0d84c66d3915aef0e06fa3/onnxruntime_directml-1.24.4-cp312-cp312-win_amd64.whl",
    sha256: "f2ecb68b7b7b259d2ef3112ae760149f9b5a1e7c0fbb73d539da6250a648a614",
    members: &[
        ("onnxruntime/capi/onnxruntime.dll", "onnxruntime.dll"),
        ("onnxruntime/capi/DirectML.dll", DIRECTML_LIBRARY),
    ],
    library: "onnxruntime.dll",
});

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const RUNTIME: Option<RuntimePackage> = Some(RuntimePackage {
    url: "https://files.pythonhosted.org/packages/6d/ab/5b68110e0460d73fad814d5bd11c7b1ddcce5c37b10177eb264d6a36e331/onnxruntime-1.24.4-cp312-cp312-manylinux_2_27_x86_64.manylinux_2_28_x86_64.whl",
    sha256: "0d640eb9f3782689b55cfa715094474cd5662f2f137be6a6f847a594b6e9705c",
    members: &[(
        "onnxruntime/capi/libonnxruntime.so.1.24.4",
        "libonnxruntime.so",
    )],
    library: "libonnxruntime.so",
});

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const RUNTIME: Option<RuntimePackage> = Some(RuntimePackage {
    url: "https://files.pythonhosted.org/packages/aa/60/c4d1c8043eb42f8a9aa9e931c8c293d289c48ff463267130eca97d13357f/onnxruntime-1.24.4-cp312-cp312-manylinux_2_27_aarch64.manylinux_2_28_aarch64.whl",
    sha256: "1a5c5a544b22f90859c88617ecb30e161ee3349fcc73878854f43d77f00558b5",
    members: &[(
        "onnxruntime/capi/libonnxruntime.so.1.24.4",
        "libonnxruntime.so",
    )],
    library: "libonnxruntime.so",
});

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const RUNTIME: Option<RuntimePackage> = Some(RuntimePackage {
    url: "https://files.pythonhosted.org/packages/d7/38/31db1b232b4ba960065a90c1506ad7a56995cd8482033184e97fadca17cc/onnxruntime-1.24.4-cp312-cp312-macosx_14_0_arm64.whl",
    sha256: "cad1c2b3f455c55678ab2a8caa51fb420c25e6e3cf10f4c23653cdabedc8de78",
    members: &[(
        "onnxruntime/capi/libonnxruntime.1.24.4.dylib",
        "libonnxruntime.dylib",
    )],
    library: "libonnxruntime.dylib",
});

/// No build of this release for the platform: OCR stays on rten.
#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
pub const RUNTIME: Option<RuntimePackage> = None;

/// DirectML ships beside ONNX Runtime: the copy in System32 is older
/// than the one this release needs.
pub const DIRECTML_LIBRARY: &str = "DirectML.dll";

/// Whether this build can use ONNX Runtime on this platform at all.
pub fn supported() -> bool {
    cfg!(feature = "ocr-onnx") && RUNTIME.is_some()
}

/// Where the runtime libraries live; one directory per release, so an
/// update never loads a library of another version.
pub fn runtime_dir(models_dir: &Path) -> PathBuf {
    models_dir.join(format!("onnxruntime-{RUNTIME_VERSION}"))
}

/// The library to load: `MOONLENS_ONNXRUNTIME` (a development
/// override) or the installed one.
pub fn library_path(models_dir: &Path) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MOONLENS_ONNXRUNTIME") {
        return Some(PathBuf::from(p));
    }
    Some(runtime_dir(models_dir).join(RUNTIME.as_ref()?.library))
}

fn present(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Whether the runtime library is on disk.
pub fn installed(models_dir: &Path) -> bool {
    let Some(runtime) = RUNTIME.as_ref() else {
        return false;
    };
    let dir = runtime_dir(models_dir);
    runtime.members.iter().all(|(_, f)| present(&dir.join(f)))
}

fn check(bytes: &[u8], sha256: &str, what: &str) -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};
    let digest: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if digest != sha256 {
        anyhow::bail!("{what} failed integrity check");
    }
    Ok(())
}

/// Write through a temporary name so a crash never leaves a truncated
/// file that looks installed.
fn store(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, path)?;
    Ok(())
}

/// Check `bytes` against the pinned hash and store them as the model.
pub fn verify_and_store(bytes: &[u8], model: &Remote, models_dir: &Path) -> anyhow::Result<()> {
    check(bytes, model.sha256, model.file)?;
    std::fs::create_dir_all(models_dir)?;
    store(&models_dir.join(model.file), bytes)
}

/// Check the archive against the pinned hash, then take the runtime
/// libraries out of it. Nothing else in the archive is written.
pub fn verify_and_unpack(
    zip: &[u8],
    package: &RuntimePackage,
    models_dir: &Path,
) -> anyhow::Result<()> {
    use std::io::Read;
    check(zip, package.sha256, "ONNX Runtime")?;
    let dir = runtime_dir(models_dir);
    std::fs::create_dir_all(&dir)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip))?;
    for (member, file) in package.members {
        let mut entry = archive
            .by_name(member)
            .map_err(|e| anyhow::anyhow!("ONNX Runtime archive lacks {member}: {e}"))?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        store(&dir.join(file), &bytes)?;
    }
    Ok(())
}

/// Download the runtime into `models_dir`. No-op when it is installed
/// or the platform has no runtime build.
pub async fn install(
    models_dir: &Path,
    progress: Option<&super::ocr_ocrs::ProgressCb>,
) -> anyhow::Result<()> {
    let Some(runtime) = RUNTIME.as_ref() else {
        return Ok(());
    };
    let dir = runtime_dir(models_dir);
    if !runtime.members.iter().all(|(_, f)| present(&dir.join(f))) {
        let client = reqwest::Client::new();
        let bytes = fetch(&client, runtime.url, "onnxruntime", progress).await?;
        let dir = models_dir.to_path_buf();
        tokio::task::spawn_blocking(move || verify_and_unpack(&bytes, runtime, &dir)).await??;
    }
    Ok(())
}

async fn fetch(
    client: &reqwest::Client,
    url: &str,
    label: &str,
    progress: Option<&super::ocr_ocrs::ProgressCb>,
) -> anyhow::Result<Vec<u8>> {
    use futures::StreamExt;
    let resp = client.get(url).send().await?.error_for_status()?;
    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(total.unwrap_or(16 << 20) as usize);
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk?);
        if let Some(cb) = progress {
            cb(label, buf.len() as u64, total);
        }
    }
    Ok(buf)
}

#[cfg(feature = "ocr-onnx")]
pub mod runtime {
    //! Sessions on the loaded library.

    use std::path::Path;
    use std::sync::OnceLock;

    use anyhow::{anyhow, Result};
    use ort::environment::Environment;
    use ort::session::Session;
    use ort::value::Tensor;
    use parking_lot::Mutex;

    use super::library_path;

    /// What the networks run on.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Device {
        Cpu,
        /// The GPU through DirectML (Windows).
        DirectMl,
    }

    /// The library loads once per process, and so does its failure:
    /// a library that is there but did not load is not tried again.
    static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();

    fn environment(models_dir: &Path) -> Result<&'static Environment> {
        let path =
            library_path(models_dir).ok_or_else(|| anyhow!("no ONNX Runtime for this platform"))?;
        // Checked before the one-time load, so that "not installed yet"
        // is not what the process remembers: the library can arrive
        // while the app runs.
        if !path.is_file() {
            return Err(anyhow!("{} is not installed", path.display()));
        }
        ENVIRONMENT
            .get_or_init(|| {
                preload_directml(&path);
                ort::init_from(&path)
                    .map_err(|e| format!("load {}: {e}", path.display()))?
                    .with_name("moonlens")
                    .build()
                    .map_err(|e| format!("start ONNX Runtime: {e}"))
            })
            .as_ref()
            .map_err(|e| anyhow!("{e}"))
    }

    /// ONNX Runtime asks Windows for `DirectML.dll` by name, and Windows
    /// would answer with the older System32 copy. Loading ours first
    /// makes it the one that name resolves to.
    #[cfg(windows)]
    fn preload_directml(library: &Path) {
        static KEEP: OnceLock<Option<libloading::Library>> = OnceLock::new();
        KEEP.get_or_init(|| {
            let path = library.with_file_name(super::DIRECTML_LIBRARY);
            // SAFETY: a Microsoft system library whose bytes passed the
            // pinned hash at install; loading it runs no code of ours.
            match unsafe { libloading::Library::new(&path) } {
                Ok(l) => Some(l),
                Err(e) => {
                    tracing::debug!("DirectML not preloaded from {}: {e}", path.display());
                    None
                }
            }
        });
    }

    #[cfg(not(windows))]
    fn preload_directml(_library: &Path) {}

    fn cores() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(1, 8)
    }

    fn open(models_dir: &Path, file: &str, device: Device, threads: usize) -> Result<Session> {
        let env = environment(models_dir)?;
        let mut builder = Session::builder(env).map_err(|e| anyhow!("{e}"))?;
        builder = match device {
            Device::Cpu => builder
                .with_intra_threads(threads)
                .map_err(|e| anyhow!("{e}"))?,
            Device::DirectMl => directml(builder)?,
        };
        builder
            .commit_from_file(models_dir.join(file))
            .map_err(|e| anyhow!("load {file}: {e}"))
    }

    /// The detection network on one device.
    pub struct Detector(Mutex<Session>);

    impl Detector {
        pub fn load(models_dir: &Path, file: &str, device: Device) -> Result<Self> {
            Ok(Self(Mutex::new(open(models_dir, file, device, cores())?)))
        }

        /// Run the detector on one `[1, 3, h, w]` picture.
        pub fn run(&self, input: Vec<f32>, h: usize, w: usize) -> Result<(Vec<f32>, Vec<usize>)> {
            run(self.0.lock(), input, [1, 3, h, w])
        }
    }

    /// A recognition network on one device: one session per concurrent
    /// caller (see [`Recognizer::run`]).
    pub struct Recognizer(Vec<Mutex<Session>>);

    impl Recognizer {
        /// `callers`: how many threads will call [`Recognizer::run`]
        /// at the same time; the cores are split between them.
        pub fn load(models_dir: &Path, file: &str, device: Device, callers: usize) -> Result<Self> {
            let callers = callers.clamp(1, cores());
            Ok(Self(
                (0..callers)
                    .map(|_| {
                        open(models_dir, file, device, (cores() / callers).max(1)).map(Mutex::new)
                    })
                    .collect::<Result<_>>()?,
            ))
        }

        /// Run recognition on `n` lines of `[3, h, w]`.
        pub fn run(
            &self,
            input: Vec<f32>,
            n: usize,
            h: usize,
            w: usize,
        ) -> Result<(Vec<f32>, Vec<usize>)> {
            // A free session when there is one, else wait for the first.
            let session = self
                .0
                .iter()
                .find_map(|s| s.try_lock())
                .unwrap_or_else(|| self.0[0].lock());
            run(session, input, [n, 3, h, w])
        }
    }

    fn run(
        mut session: parking_lot::MutexGuard<'_, Session>,
        input: Vec<f32>,
        shape: [usize; 4],
    ) -> Result<(Vec<f32>, Vec<usize>)> {
        let input = Tensor::from_array((shape, input)).map_err(|e| anyhow!("{e}"))?;
        let out = session
            .run(ort::inputs!["x" => input])
            .map_err(|e| anyhow!("{e}"))?;
        let (shape, data) = out[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("{e}"))?;
        Ok((data.to_vec(), shape.iter().map(|&d| d as usize).collect()))
    }

    #[cfg(windows)]
    fn directml(
        builder: ort::session::builder::SessionBuilder,
    ) -> Result<ort::session::builder::SessionBuilder> {
        // DirectML requires both: no memory pattern, one run at a time.
        builder
            .with_memory_pattern(false)
            .map_err(|e| anyhow!("{e}"))?
            .with_parallel_execution(false)
            .map_err(|e| anyhow!("{e}"))?
            .with_execution_providers([ort::ep::DirectML::default().build().error_on_failure()])
            .map_err(|e| anyhow!("DirectML: {e}"))
    }

    #[cfg(not(windows))]
    fn directml(
        _builder: ort::session::builder::SessionBuilder,
    ) -> Result<ort::session::builder::SessionBuilder> {
        Err(anyhow!("DirectML is a Windows feature"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn only_the_named_libraries_leave_the_archive() {
        let zip = archive(&[
            ("pkg/capi/lib.bin", b"library"),
            ("pkg/capi/other.py", b"not wanted"),
        ]);
        let package = RuntimePackage {
            url: "",
            sha256: Box::leak(sha256(&zip).into_boxed_str()),
            members: &[("pkg/capi/lib.bin", "lib.bin")],
            library: "lib.bin",
        };
        let dir = tempfile::tempdir().unwrap();
        verify_and_unpack(&zip, &package, dir.path()).unwrap();
        let out = runtime_dir(dir.path());
        assert_eq!(std::fs::read(out.join("lib.bin")).unwrap(), b"library");
        assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    }

    #[test]
    fn an_archive_with_another_hash_is_rejected_and_nothing_is_written() {
        let zip = archive(&[("pkg/capi/lib.bin", b"library")]);
        let package = RuntimePackage {
            url: "",
            sha256: "00",
            members: &[("pkg/capi/lib.bin", "lib.bin")],
            library: "lib.bin",
        };
        let dir = tempfile::tempdir().unwrap();
        assert!(verify_and_unpack(&zip, &package, dir.path()).is_err());
        assert!(!runtime_dir(dir.path()).exists());
    }

    #[test]
    fn an_archive_without_the_library_is_an_error() {
        let zip = archive(&[("pkg/capi/other.py", b"x")]);
        let package = RuntimePackage {
            url: "",
            sha256: Box::leak(sha256(&zip).into_boxed_str()),
            members: &[("pkg/capi/lib.bin", "lib.bin")],
            library: "lib.bin",
        };
        let dir = tempfile::tempdir().unwrap();
        assert!(verify_and_unpack(&zip, &package, dir.path()).is_err());
    }

    #[test]
    fn a_model_with_another_hash_is_not_stored() {
        let dir = tempfile::tempdir().unwrap();
        let model = Remote {
            url: "",
            sha256: "00",
            file: "m.onnx",
        };
        assert!(verify_and_store(b"model", &model, dir.path()).is_err());
        assert!(!dir.path().join("m.onnx").exists());
    }

    /// Needs the real archive of this platform: download `RUNTIME.url`
    /// and run `MOONLENS_ORT_ARCHIVE=/path cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_archive_verifies_and_unpacks() {
        let zip = std::fs::read(std::env::var("MOONLENS_ORT_ARCHIVE").unwrap()).unwrap();
        let package = RUNTIME.as_ref().unwrap();
        let dir = tempfile::tempdir().unwrap();
        verify_and_unpack(&zip, package, dir.path()).unwrap();
        assert!(runtime_dir(dir.path()).join(package.library).is_file());
    }

    #[test]
    fn nothing_counts_as_installed_in_an_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!installed(dir.path()));
    }
}
