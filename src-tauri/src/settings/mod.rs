//! Hierarchical settings system.
//!
//! Design goals:
//! - Everything user-visible is configurable from TOML; no hardcoded magic.
//! - Layered resolution: `default.toml` (embedded) -> `user.toml` (on disk)
//!   -> active profile overlay. Later layers win per-key.
//! - Schema is generated with `schemars` so the frontend can render a form
//!   automatically. New providers/plugins do not require manual UI work.
//! - Hot reload via `notify`; subscribers receive a fresh snapshot.

pub mod migrate;
pub mod profiles;
pub mod rules;
pub mod schema;
pub mod store;
#[cfg(test)]
mod tests;
pub mod validate;
pub mod watcher;

pub use schema::Settings;
pub use store::SettingsStore;
pub use watcher::Watcher;
