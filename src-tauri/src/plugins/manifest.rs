//! Plugin `plugin.toml` manifest schema.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub settings: Option<PluginSettingsBlock>,
    #[serde(default)]
    pub ui: Option<UiBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub kind: PluginKind,
    pub entry: String,
    pub api_version: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub update_url: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    Wasm,
    Sidecar,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Capabilities {
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub consumes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Permissions {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub fs_read: Vec<String>,
    #[serde(default)]
    pub fs_write: Vec<String>,
    #[serde(default)]
    pub audio_capture: bool,
    #[serde(default)]
    pub screen_capture: bool,
    #[serde(default)]
    pub clipboard: bool,
    #[serde(default)]
    pub subprocess: bool,
    #[serde(default)]
    pub env: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginSettingsBlock {
    pub schema: String,
    #[serde(default)]
    pub defaults: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UiBlock {
    pub settings_page: Option<String>,
}
