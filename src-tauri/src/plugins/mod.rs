//! Plugin subsystem.
//!
//! Two flavors coexist:
//!
//! - **WASM Component plugins** via `wasmtime` + the Component Model. These
//!   are sandboxed, cross-platform, and can be written in any language that
//!   can target a WIT interface. Good for pure computation (custom
//!   translators, intent classifiers, OCR post-processors).
//! - **Sidecar plugins** run as child processes and speak JSON-RPC over
//!   stdio. Good for things that need OS privileges (audio capture, GPU
//!   inference) or heavy native deps.
//!
//! Both expose the same [`crate::providers::Provider`] trait so the pipeline
//! does not care about the transport.

pub mod manifest;
pub mod registry;
pub mod sidecar_host;
pub mod wasm_host;

pub use manifest::{PluginKind, PluginManifest};
pub use registry::{PluginDescriptor, PluginRegistry};
