//! Adapter over the hardware introspection used to recommend a
//! model tier on first launch.
//!
//! The pre-7.x code called `sysinfo::System` directly inside the
//! `model_tier_status` IPC. Tests for "what does a 2 GB-RAM laptop
//! see?" therefore needed an actual 2 GB-RAM laptop. The trait here
//! lets us inject a [`FixedProbe`] that returns whatever
//! [`HostCapabilities`] the test wants.

use serde::Serialize;

/// Snapshot of the host's translation-relevant capabilities. We
/// deliberately keep this narrow: only fields the tier picker
/// consults belong here. Adding e.g. `gpu_vram` later is fine; do
/// not add storage usage or network speed because those are not
/// tier inputs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct HostCapabilities {
    /// Total installed RAM in GB (binary base, i.e. divide by 1024^3).
    pub total_ram_gb: f64,
    /// Number of logical CPU cores (`sysinfo`'s `cpus().len()`).
    pub cpu_cores: usize,
    /// Whether the inference runtime can offload to a GPU. Derived
    /// at compile time from the active Cargo feature set: any of
    /// `embedded-llm-vulkan` / `embedded-llm-cuda` / `embedded-llm-metal`
    /// enables it. Tests use [`FixedProbe`] to override.
    pub gpu_available: bool,
}

impl HostCapabilities {
    /// Hardware bucket for the translation router. Maps the
    /// boolean GPU flag onto the `translation_router::Hardware`
    /// enum without dragging that module into the probe layer.
    pub fn hw_kind(&self) -> crate::translation_router::Hardware {
        if self.gpu_available {
            crate::translation_router::Hardware::Gpu
        } else {
            crate::translation_router::Hardware::Cpu
        }
    }
}

/// True when the binary was compiled with at least one GPU
/// backend feature enabled. This is a compile-time signal — it
/// does NOT verify that a physical GPU is present at runtime
/// (llama.cpp falls back to CPU silently when its requested
/// backend cannot allocate a context). For the router, that
/// degradation is fine: choosing Pivot on a host where GPU was
/// expected but is missing still produces correct output, just
/// slower than predicted.
pub const GPU_FEATURE_ENABLED: bool = cfg!(any(
    feature = "embedded-llm-vulkan",
    feature = "embedded-llm-cuda",
    feature = "embedded-llm-metal",
));

/// One-shot host capability probe. Production reads from `sysinfo`,
/// tests inject [`FixedProbe`].
pub trait HardwareProbe: Send + Sync {
    /// Read the host's capabilities. Implementations are expected to
    /// be cheap (sub-millisecond) — the IPC layer calls this on every
    /// `model_tier_status` request.
    fn capabilities(&self) -> HostCapabilities;
}

/// Production probe backed by [`sysinfo`]. Kept as a unit struct so
/// it can be `Default`-constructed wherever production code wires
/// the service.
#[derive(Default, Debug, Clone, Copy)]
pub struct SysinfoProbe;

impl HardwareProbe for SysinfoProbe {
    fn capabilities(&self) -> HostCapabilities {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu();
        let bytes = sys.total_memory();
        let total_ram_gb = (bytes as f64) / (1024.0 * 1024.0 * 1024.0);
        let cpu_cores = sys.cpus().len().max(1);
        HostCapabilities {
            total_ram_gb,
            cpu_cores,
            gpu_available: GPU_FEATURE_ENABLED,
        }
    }
}

/// Test probe that returns a canned [`HostCapabilities`]. Constructed
/// at test setup and shared via `Arc<dyn HardwareProbe>` so the
/// service under test does not need to know about the test type.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct FixedProbe {
    pub caps: HostCapabilities,
}

#[cfg(test)]
impl FixedProbe {
    pub fn new(total_ram_gb: f64, cpu_cores: usize) -> Self {
        Self {
            caps: HostCapabilities {
                total_ram_gb,
                cpu_cores,
                // Tests default to CPU mode so router decisions
                // are deterministic across CI machines (the GPU
                // flag would otherwise depend on which feature
                // set the test was compiled with).
                gpu_available: false,
            },
        }
    }

    /// Override the GPU flag for tests that exercise the router's
    /// Pivot decisions. Returns `self` for chaining.
    pub fn with_gpu(mut self, gpu_available: bool) -> Self {
        self.caps.gpu_available = gpu_available;
        self
    }
}

#[cfg(test)]
impl HardwareProbe for FixedProbe {
    fn capabilities(&self) -> HostCapabilities {
        self.caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_registry::{recommended_class_for_ram, ModelTierClass};

    #[test]
    fn fixed_probe_returns_canned_values() {
        let probe = FixedProbe::new(2.0, 4);
        let caps = probe.capabilities();
        assert_eq!(caps.total_ram_gb, 2.0);
        assert_eq!(caps.cpu_cores, 4);
        // FixedProbe defaults to CPU mode so router tests built on
        // top of it are deterministic across CI configurations.
        assert!(!caps.gpu_available);
    }

    #[test]
    fn with_gpu_override_flips_hw_kind() {
        use crate::translation_router::Hardware;
        let cpu = FixedProbe::new(16.0, 8).capabilities();
        assert_eq!(cpu.hw_kind(), Hardware::Cpu);

        let gpu = FixedProbe::new(16.0, 8).with_gpu(true).capabilities();
        assert_eq!(gpu.hw_kind(), Hardware::Gpu);
    }

    #[test]
    fn fixed_probe_drives_compact_recommendation_on_low_ram() {
        // The point of the trait is that we can sweep the
        // recommendation logic across "tiny laptop", "office laptop",
        // "workstation" without owning all three. Here we ride the
        // probe to confirm the ladder.
        let cases: &[(f64, ModelTierClass)] = &[
            (1.0, ModelTierClass::Compact),
            (4.0, ModelTierClass::Compact),
            (5.99, ModelTierClass::Compact),
            (6.0, ModelTierClass::Standard),
            (8.0, ModelTierClass::Standard),
            (11.99, ModelTierClass::Standard),
            (12.0, ModelTierClass::Quality),
            (16.0, ModelTierClass::Quality),
            (128.0, ModelTierClass::Quality),
        ];
        for &(ram, expected) in cases {
            let probe = FixedProbe::new(ram, 4);
            let caps = probe.capabilities();
            assert_eq!(
                recommended_class_for_ram(caps.total_ram_gb),
                expected,
                "ram = {ram}",
            );
        }
    }

    #[test]
    fn sysinfo_probe_returns_realistic_values_on_the_host_running_tests() {
        // Sanity check that the production impl returns SOMETHING
        // sensible — we cannot pin the values (they depend on the
        // CI runner) but we can guard against "0 RAM, 0 cores"
        // breakage that would mean the impl is silently broken.
        let probe = SysinfoProbe;
        let caps = probe.capabilities();
        assert!(
            caps.total_ram_gb > 0.5,
            "host RAM probe returned implausibly small value: {}",
            caps.total_ram_gb
        );
        assert!(
            caps.cpu_cores >= 1,
            "host CPU probe returned 0 cores: {}",
            caps.cpu_cores
        );
    }
}
