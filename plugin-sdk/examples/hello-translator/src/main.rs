//! Minimal MoonLens sidecar plugin: echoes the input text, optionally
//! uppercased. Useful as a working template.

use moonlens_plugin_sdk::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

struct Hello;

impl Provider for Hello {
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }

    fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        match input {
            TaskInput::Translate { text, .. } => Ok(TaskOutput::Text(format!("[hello] {text}"))),
            _ => Err(ProviderError::Other(
                "hello-translator only handles Translate".into(),
            )),
        }
    }
}

fn main() -> anyhow::Result<()> {
    moonlens_plugin_sdk::run_sidecar(Hello)
}
