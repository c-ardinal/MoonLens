//! Minimal fixture plugin used by the WASM Component integration test.
//!
//! Implements the `moonlens:plugin/provider` world with a single
//! `translator` capability. Translate requests echo the text back with
//! a `[wasm-hello]` prefix; every other variant returns `LowConfidence`
//! so the host can exercise the error path too.

#[allow(warnings)]
mod bindings;

use bindings::exports::moonlens::plugin::provider::{
    Capability, Guest, ProviderError, TaskInput, TaskOutput,
};

struct Plugin;

impl Guest for Plugin {
    fn list_capabilities() -> Vec<Capability> {
        vec![Capability::Translate]
    }

    fn run(input: TaskInput) -> Result<TaskOutput, ProviderError> {
        match input {
            TaskInput::Translate(t) => Ok(TaskOutput::Text(format!("[wasm-hello] {}", t.text))),
            TaskInput::Summarize(text) => Ok(TaskOutput::Text(format!("sum: {text}"))),
            _ => Err(ProviderError::LowConfidence),
        }
    }
}

bindings::export!(Plugin with_types_in bindings);
