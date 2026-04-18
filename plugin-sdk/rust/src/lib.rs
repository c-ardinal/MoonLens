//! Rust helpers for writing MoonLens plugins.
//!
//! Two flavors are supported:
//!
//! - **Component (WASM) plugins**: implement the traits emitted by
//!   `wit-bindgen` from `wit/moonlens.wit`. Use `wit_bindgen::generate!`
//!   in your crate.
//! - **Sidecar plugins**: implement the `Provider` trait below and call
//!   [`run_sidecar`] from your `main`. The binary will then speak JSON-RPC
//!   over stdio to the MoonLens host.
//!
//! This crate deliberately does not pull in any heavy dependencies; a
//! sidecar written against it compiles to a small standalone binary.

use std::io::{BufRead, Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// The provider trait every sidecar plugin implements.
pub trait Provider: Send + Sync {
    fn capabilities(&self) -> &[Capability];
    fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Translate,
    Ocr,
    Asr,
    Explain,
    Summarize,
    ReverseImageSearch,
    QrHandler,
    IntentClassifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskInput {
    Translate {
        text: String,
        source_language: String,
        target_language: String,
    },
    Ocr {
        image_png: Vec<u8>,
        languages: Vec<String>,
    },
    Asr {
        audio_pcm16_mono_16k: Vec<u8>,
        language_hint: Option<String>,
    },
    Explain {
        prompt: String,
        image_png: Option<Vec<u8>>,
    },
    Summarize {
        text: String,
    },
    ReverseImageSearch {
        image_url: String,
    },
    QrHandler {
        payload: String,
    },
    IntentClassifier {
        image_png: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOutput {
    Text(String),
    TextChunk { delta: String, done: bool },
    Ocr { lines: Vec<OcrLine> },
    Transcript { segments: Vec<TranscriptSegment> },
    OpenedExternalUrl(String),
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: [f32; 4],
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_s: f32,
    pub end_s: f32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderError {
    Timeout(u32),
    Offline,
    RateLimit,
    UnsupportedLanguage(String, String),
    LowConfidence,
    Other(String),
}

/// Main event loop for sidecar plugins. Reads newline-delimited JSON-RPC 2.0
/// requests from stdin, calls into `provider.run`, and writes responses.
pub fn run_sidecar<P: Provider>(provider: P) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("sidecar: bad request: {e}");
                continue;
            }
        };
        let resp = match provider.run(req.params.input) {
            Ok(output) => RpcResponse {
                jsonrpc: "2.0".into(),
                id: req.id,
                result: Some(output),
                error: None,
            },
            Err(e) => RpcResponse {
                jsonrpc: "2.0".into(),
                id: req.id,
                result: None,
                error: Some(RpcError {
                    code: -1,
                    message: match e {
                        ProviderError::Timeout(ms) => format!("timeout {ms}ms"),
                        ProviderError::Offline => "offline".into(),
                        ProviderError::RateLimit => "rate_limit".into(),
                        ProviderError::UnsupportedLanguage(a, b) => {
                            format!("unsupported_language:{a}->{b}")
                        }
                        ProviderError::LowConfidence => "low_confidence".into(),
                        ProviderError::Other(s) => s,
                    },
                }),
            },
        };
        let raw = serde_json::to_string(&resp)?;
        writeln!(out, "{raw}")?;
        out.flush()?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: u64,
    #[allow(dead_code)]
    method: String,
    params: RpcParams,
}
#[derive(Deserialize)]
struct RpcParams {
    #[allow(dead_code)]
    capability: String,
    input: TaskInput,
}
#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<TaskOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}
#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_input_translate_round_trips_through_json() {
        let input = TaskInput::Translate {
            text: "hi".into(),
            source_language: "en".into(),
            target_language: "ja".into(),
        };
        let raw = serde_json::to_string(&input).unwrap();
        let parsed: TaskInput = serde_json::from_str(&raw).unwrap();
        match parsed {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
            } => {
                assert_eq!(text, "hi");
                assert_eq!(source_language, "en");
                assert_eq!(target_language, "ja");
            }
            _ => panic!("wrong variant after round trip"),
        }
    }

    #[test]
    fn task_output_text_round_trips() {
        let out = TaskOutput::Text("hola".into());
        let raw = serde_json::to_string(&out).unwrap();
        let parsed: TaskOutput = serde_json::from_str(&raw).unwrap();
        match parsed {
            TaskOutput::Text(t) => assert_eq!(t, "hola"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn request_deserialization_accepts_host_wire_format() {
        // Exact shape the host `sidecar_host` emits.
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"run","params":{"capability":"translate","input":{"Translate":{"text":"hi","source_language":"en","target_language":"ja"}}}}"#;
        let req: RpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "run");
        assert_eq!(req.params.capability, "translate");
    }

    #[test]
    fn response_serialization_shape_is_compatible_with_host() {
        let resp = RpcResponse {
            jsonrpc: "2.0".into(),
            id: 1,
            result: Some(TaskOutput::Text("hello".into())),
            error: None,
        };
        let raw = serde_json::to_string(&resp).unwrap();
        assert!(raw.contains("\"id\":1"));
        assert!(raw.contains("\"result\""));
        assert!(!raw.contains("\"error\""));
    }

    #[test]
    fn provider_error_variants_serialize_as_tagged_enum() {
        let e = ProviderError::Offline;
        let raw = serde_json::to_string(&e).unwrap();
        assert_eq!(raw, "\"Offline\"");
        let timeout = serde_json::to_string(&ProviderError::Timeout(500)).unwrap();
        assert_eq!(timeout, "{\"Timeout\":500}");
    }
}
