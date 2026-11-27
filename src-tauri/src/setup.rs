//! First-run setup: read the PC's hardware and pick the model each
//! feature should use.
//!
//! Translation quality comes first: the 2B PLaMo model is recommended
//! whenever the PC can hold it, and the lighter models are only offered
//! to PCs that would otherwise struggle. The detected hardware never
//! leaves the PC.

use crate::i18n::Text;
use serde::Serialize;

use crate::gpu::GpuDevice;
use crate::providers::{llm_models, whisper_models};

const GIB: u64 = 1 << 30;

/// The hardware the recommendation is based on.
#[derive(Debug, Clone, Serialize)]
pub struct SystemProfile {
    pub os: &'static str,
    /// CPU model name ("" when the OS does not report it).
    pub cpu_name: String,
    /// Logical cores.
    pub cpu_cores: usize,
    /// Installed RAM in bytes.
    pub ram_total: u64,
    /// GPUs the built-in engines can offload to.
    pub gpus: Vec<GpuDevice>,
    /// Free space where the models are stored (None: unknown).
    pub disk_free: Option<u64>,
    /// `performance.use_gpu_if_available`.
    pub use_gpu: bool,
}

impl SystemProfile {
    pub fn detect(use_gpu: bool) -> Self {
        use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_memory(MemoryRefreshKind::nothing().with_ram())
                .with_cpu(CpuRefreshKind::nothing()),
        );
        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default();
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(sys.cpus().len().max(1));
        let disk_free = crate::providers::ocr_ocrs::default_models_dir()
            .and_then(|d| crate::providers::model_store::free_space(&d));
        Self {
            os: std::env::consts::OS,
            cpu_name,
            cpu_cores,
            ram_total: sys.total_memory(),
            gpus: crate::gpu::devices(),
            disk_free,
            use_gpu,
        }
    }

    /// The GPU inference will run on, if any: a discrete card, or the
    /// unified-memory GPU of a Mac (integrated GPUs elsewhere are often
    /// slower than the CPU for this work, so they are not counted).
    pub fn accelerator(&self) -> Option<&GpuDevice> {
        if !self.use_gpu {
            return None;
        }
        self.gpus
            .iter()
            .filter(|g| !g.integrated || self.os == "macos")
            .max_by_key(|g| g.memory_total)
    }

    /// Memory the translation model can live in: the GPU's own memory
    /// when it is offloaded, system RAM otherwise.
    fn model_memory(&self) -> u64 {
        match self.accelerator() {
            Some(g) if !g.integrated => g.memory_total.max(self.ram_total / 2),
            _ => self.ram_total,
        }
    }
}

/// One feature's recommended model and the reason shown to the user.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Pick {
    pub model: &'static str,
    pub reason: Text,
}

/// The model each feature should use on this PC.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Recommendation {
    pub translate: Pick,
    pub assistant: Pick,
    pub ocr: Pick,
    pub asr: Pick,
}

/// OCR has a single choice: PP-OCRv5, installed together with the
/// Latin-only ocrs fallback.
pub const OCR_MODEL_ID: &str = "ppocr";

pub fn recommend(p: &SystemProfile) -> Recommendation {
    let gpu = p.accelerator();
    let mem = p.model_memory();

    // PLaMo 2B (Q4_K_M, 1.7 GB) needs about 2.5 GB at run time. Only a
    // PC that cannot spare that gets the 0.8B model.
    let translate = if mem >= 6 * GIB || gpu.is_some_and(|g| g.memory_total >= 3 * GIB) {
        Pick {
            model: llm_models::DEFAULT_TRANSLATE_MODEL_ID,
            reason: Text::new("setup:high_quality_and_fast_runs_comfortably_on"),
        }
    } else {
        Pick {
            model: "plamo-3-translate-0.8b",
            reason: Text::new("setup:low_memory_model_chosen_for_this_pcs"),
        }
    };

    // Gemma 4 E4B (4.6 GB file) wants a large GPU or plenty of RAM.
    let assistant = if gpu.is_some_and(|g| g.memory_total >= 8 * GIB) || p.ram_total >= 16 * GIB {
        Pick {
            model: llm_models::DEFAULT_ASSIST_MODEL_ID,
            reason: Text::new("setup:high_quality_explanations_and_summaries_runs"),
        }
    } else {
        Pick {
            model: "gemma-4-e2b",
            reason: Text::new("setup:lightweight_model_chosen_for_this_pcs_memory"),
        }
    };

    // large-v3-turbo decodes in well under a second on a GPU and in a
    // few seconds on an 8-core CPU; slower PCs fall behind live speech.
    let asr = if gpu.is_some_and(|g| g.memory_total >= 2 * GIB)
        || (p.cpu_cores >= 8 && p.ram_total >= 8 * GIB)
    {
        Pick {
            model: whisper_models::DEFAULT_MODEL_ID,
            reason: Text::new("setup:accurate_in_japanese_and_english_this_pc"),
        }
    } else {
        Pick {
            model: "whisper-small",
            reason: Text::new("setup:fast_model_chosen_for_this_pcs_cpu"),
        }
    };

    Recommendation {
        translate,
        assistant,
        ocr: Pick {
            model: OCR_MODEL_ID,
            reason: Text::new("setup:reads_japanese_chinese_and_english"),
        },
        asr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pc(ram_gib: u64, cores: usize, gpus: Vec<GpuDevice>) -> SystemProfile {
        SystemProfile {
            os: "windows",
            cpu_name: String::new(),
            cpu_cores: cores,
            ram_total: ram_gib * GIB,
            gpus,
            disk_free: None,
            use_gpu: true,
        }
    }

    fn gpu(vram_gib: u64, integrated: bool) -> GpuDevice {
        GpuDevice {
            name: "GPU".into(),
            backend: "Vulkan".into(),
            memory_total: vram_gib * GIB,
            integrated,
        }
    }

    #[test]
    fn gaming_pc_gets_the_best_models() {
        let r = recommend(&pc(32, 16, vec![gpu(12, false)]));
        assert_eq!(r.translate.model, "plamo-3-translate-2b");
        assert_eq!(r.assistant.model, "gemma-4-e4b");
        assert_eq!(r.asr.model, "whisper-large-v3-turbo");
        assert_eq!(r.ocr.model, "ppocr");
    }

    #[test]
    fn typical_8gb_laptop_keeps_plamo_2b() {
        // Quality first: 8 GB of RAM still runs the 2B model.
        let r = recommend(&pc(8, 4, vec![gpu(4, true)]));
        assert_eq!(r.translate.model, "plamo-3-translate-2b");
        assert_eq!(r.assistant.model, "gemma-4-e2b");
        assert_eq!(r.asr.model, "whisper-small");
    }

    #[test]
    fn low_memory_pc_gets_light_models() {
        let r = recommend(&pc(4, 2, vec![]));
        assert_eq!(r.translate.model, "plamo-3-translate-0.8b");
        assert_eq!(r.assistant.model, "gemma-4-e2b");
        assert_eq!(r.asr.model, "whisper-small");
    }

    #[test]
    fn small_ram_with_a_discrete_gpu_keeps_plamo_2b() {
        let r = recommend(&pc(4, 4, vec![gpu(4, false)]));
        assert_eq!(r.translate.model, "plamo-3-translate-2b");
        assert_eq!(r.asr.model, "whisper-large-v3-turbo");
    }

    #[test]
    fn gpu_switched_off_is_ignored() {
        let mut p = pc(8, 4, vec![gpu(12, false)]);
        p.use_gpu = false;
        assert!(p.accelerator().is_none());
        assert_eq!(recommend(&p).asr.model, "whisper-small");
    }

    #[test]
    fn mac_unified_memory_counts_as_a_gpu() {
        let mut p = pc(16, 8, vec![gpu(11, true)]);
        p.os = "macos";
        assert!(p.accelerator().is_some());
        let r = recommend(&p);
        assert_eq!(r.assistant.model, "gemma-4-e4b");
        assert_eq!(r.asr.model, "whisper-large-v3-turbo");
    }

    #[test]
    fn every_pick_is_in_its_catalog() {
        for p in [pc(2, 1, vec![]), pc(64, 32, vec![gpu(24, false)])] {
            let r = recommend(&p);
            assert!(llm_models::find(r.translate.model).is_some());
            let a = llm_models::find(r.assistant.model).unwrap();
            assert_eq!(a.style, llm_models::PromptStyle::ChatTemplate);
            assert!(whisper_models::find(r.asr.model).is_some());
        }
    }
}
