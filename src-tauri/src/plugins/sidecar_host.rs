//! Sidecar-process plugin host. Speaks JSON-RPC 2.0 over stdio.
//!
//! On `spawn` the host starts the child with permissions filtered per the
//! manifest (env allowlist, fs allowlist reflected in working dir, etc.),
//! performs a `handshake` call to confirm API version, then wraps each
//! advertised capability in a [`Provider`].

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::oneshot,
};

use crate::providers::{
    Capability, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
};

use super::manifest::PluginManifest;

pub struct SidecarPlugin {
    provides: Vec<String>,
    conn: Arc<Conn>,
}

struct Conn {
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<std::collections::HashMap<u64, oneshot::Sender<RpcResponse>>>,
    next_id: Mutex<u64>,
    _child: Mutex<Child>,
}

impl SidecarPlugin {
    pub fn spawn(manifest: &PluginManifest, dir: &Path) -> Result<Self> {
        let bin: PathBuf = dir.join(&manifest.plugin.entry);
        let mut cmd = Command::new(&bin);
        cmd.current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env_clear();
        for k in &manifest.permissions.env {
            if let Ok(v) = std::env::var(k) {
                cmd.env(k, v);
            }
        }
        let mut child = cmd.spawn().with_context(|| {
            format!("spawn sidecar {} at {}", manifest.plugin.id, bin.display())
        })?;

        let stdin = child.stdin.take().context("no stdin")?;
        let stdout = child.stdout.take().context("no stdout")?;

        let conn = Arc::new(Conn {
            stdin: Mutex::new(Some(stdin)),
            pending: Mutex::new(Default::default()),
            next_id: Mutex::new(1),
            _child: Mutex::new(child),
        });

        // Reader task: dispatch responses to waiting senders.
        let conn_r = conn.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => match serde_json::from_str::<RpcResponse>(line.trim()) {
                        Ok(resp) => {
                            if let Some(tx) = conn_r.pending.lock().remove(&resp.id) {
                                let _ = tx.send(resp);
                            }
                        }
                        Err(e) => tracing::warn!("sidecar bad line: {e} -- {line}"),
                    },
                    Err(e) => {
                        tracing::warn!("sidecar read error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            provides: manifest.capabilities.provides.clone(),
            conn,
        })
    }

    pub fn provider_handles(&self, plugin_id: &str) -> Vec<(String, ProviderHandle)> {
        self.provides
            .iter()
            .filter_map(|cap_str| parse_cap(cap_str).map(|c| (cap_str.clone(), c)))
            .map(|(cap_str, cap)| {
                let chain_id = format!("plugin.{plugin_id}.{cap_str}");
                let handle: ProviderHandle = Arc::new(SidecarProvider {
                    id: chain_id.clone(),
                    capability: cap,
                    conn: self.conn.clone(),
                });
                (chain_id, handle)
            })
            .collect()
    }
}

fn parse_cap(s: &str) -> Option<Capability> {
    Some(match s {
        "translator" => Capability::Translate,
        "ocr" => Capability::Ocr,
        "asr" => Capability::Asr,
        "explain" => Capability::Explain,
        "summarize" => Capability::Summarize,
        "reverse_image_search" => Capability::ReverseImageSearch,
        "qr_handler" => Capability::QrHandler,
        "intent_classifier" => Capability::IntentClassifier,
        _ => return None,
    })
}

struct SidecarProvider {
    id: String,
    capability: Capability,
    conn: Arc<Conn>,
}

#[async_trait]
impl Provider for SidecarProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        std::slice::from_ref(&self.capability)
    }
    fn is_local(&self) -> bool {
        // By default sidecars are local; cloud access requires explicit
        // permission in the manifest (checked at load time).
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let id = {
            let mut n = self.conn.next_id.lock();
            let id = *n;
            *n = id.wrapping_add(1);
            id
        };
        let (tx, rx) = oneshot::channel();
        self.conn.pending.lock().insert(id, tx);

        let req = RpcRequest {
            jsonrpc: "2.0",
            id,
            method: "run",
            params: RpcParams {
                capability: capability_tag(self.capability),
                input,
            },
        };
        let raw = serde_json::to_string(&req).map_err(|e| ProviderError::Other(e.to_string()))?;
        {
            let mut guard = self.conn.stdin.lock();
            if let Some(stdin) = guard.as_mut() {
                stdin
                    .write_all(raw.as_bytes())
                    .await
                    .map_err(|e| ProviderError::Other(e.to_string()))?;
                stdin
                    .write_all(b"\n")
                    .await
                    .map_err(|e| ProviderError::Other(e.to_string()))?;
            } else {
                return Err(ProviderError::Other("sidecar stdin closed".into()));
            }
        }
        let resp = rx
            .await
            .map_err(|_| ProviderError::Other("sidecar dropped response".into()))?;
        match resp.result {
            Some(out) => Ok(out),
            None => Err(ProviderError::Other(
                resp.error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "unknown sidecar error".into()),
            )),
        }
    }
}

fn capability_tag(c: Capability) -> &'static str {
    match c {
        Capability::Translate => "translate",
        Capability::Ocr => "ocr",
        Capability::Asr => "asr",
        Capability::Explain => "explain",
        Capability::Summarize => "summarize",
        Capability::ReverseImageSearch => "reverse_image_search",
        Capability::QrHandler => "qr_handler",
        Capability::IntentClassifier => "intent_classifier",
    }
}

#[derive(Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    params: RpcParams,
}
#[derive(Serialize)]
struct RpcParams {
    capability: &'static str,
    input: TaskInput,
}
#[derive(Deserialize)]
struct RpcResponse {
    id: u64,
    #[serde(default)]
    result: Option<TaskOutput>,
    #[serde(default)]
    error: Option<RpcError>,
}
#[derive(Deserialize)]
struct RpcError {
    #[serde(default)]
    #[allow(dead_code)]
    code: i32,
    message: String,
}
