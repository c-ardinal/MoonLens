//! Turn internal errors into the short Japanese messages the UI shows.
//!
//! Commands return errors as `"<code>|<message>"`. The code lets the
//! frontend offer the fix next to the message (for example a button that
//! opens the model downloads for `model_missing`); the message is shown
//! as is. Codes:
//!
//! | code              | meaning                                    |
//! | ----------------- | ------------------------------------------ |
//! | `model_missing`   | a translation / OCR / assistant model is not downloaded |
//! | `asr_unavailable` | the speech recognition engine cannot be reached |
//! | `cpu_unsupported` | the CPU cannot run the bundled engine      |
//! | `timeout`         | a step took longer than its time limit     |
//! | `network`         | a download could not reach the server      |
//! | `disk_full`       | not enough free space for a download       |
//! | `integrity`       | a download did not match its pinned hash   |
//! | `error`           | anything else (raw detail appended)        |

use crate::providers::{Capability, ProviderError};

pub fn coded(code: &str, message: impl AsRef<str>) -> String {
    format!("{code}|{}", message.as_ref())
}

/// Message for a failed pipeline task.
pub fn provider(e: &ProviderError, cap: Capability) -> String {
    match e {
        ProviderError::NotInstalled(what) => match cap {
            Capability::Ocr => coded("model_missing", "OCR モデル未導入"),
            Capability::Explain | Capability::Summarize => {
                coded("model_missing", format!("説明用モデル未導入：{what}"))
            }
            Capability::Asr => coded("asr_unavailable", "音声認識エンジン未導入"),
            _ => coded("model_missing", format!("翻訳モデル未導入：{what}")),
        },
        ProviderError::Timeout(ms) if *ms > 0 => coded(
            "timeout",
            format!("制限時間（{} 秒）超過。範囲を縮小して再試行", ms / 1000),
        ),
        ProviderError::Timeout(_) => coded("timeout", "制限時間超過。範囲を縮小して再試行"),
        ProviderError::Other(m) if m.contains("AVX2") => {
            coded("cpu_unsupported", "非対応 CPU（AVX2・FMA 必須）")
        }
        ProviderError::Other(m) if m.starts_with("no provider configured") => {
            coded("error", "この機能のプロバイダー未設定")
        }
        other => coded("error", format!("処理失敗：{other}")),
    }
}

/// Message for a failed model download.
pub fn download(e: &anyhow::Error) -> String {
    let raw = format!("{e:#}");
    let lower = raw.to_lowercase();
    if lower.contains("no space left")
        || lower.contains("os error 28")
        || lower.contains("os error 112")
        || lower.contains("disk full")
    {
        coded("disk_full", "空き容量不足")
    } else if lower.contains("integrity") {
        coded("integrity", "整合性検証失敗。再ダウンロードしてください")
    } else if lower.contains("error sending request")
        || lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("timed out")
        || lower.contains("error decoding response body")
        || lower.contains("incomplete")
    {
        coded(
            "network",
            "ネットワーク接続失敗。接続確認後に再試行（途中から再開）",
        )
    } else if lower.contains("status") && (lower.contains("404") || lower.contains("403")) {
        coded("network", format!("配布元からの取得失敗：{raw}"))
    } else {
        coded("error", format!("ダウンロード失敗：{raw}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_models_point_at_the_download() {
        let e = ProviderError::NotInstalled("PLaMo 3 Translate 2B".into());
        assert_eq!(
            provider(&e, Capability::Translate),
            "model_missing|翻訳モデル未導入：PLaMo 3 Translate 2B"
        );
        assert!(provider(&e, Capability::Ocr).starts_with("model_missing|OCR"));
        assert!(provider(&e, Capability::Explain).starts_with("model_missing|説明用"));
    }

    #[test]
    fn download_errors_are_classified() {
        let net = anyhow::anyhow!("error sending request for url (https://huggingface.co/x)");
        assert!(download(&net).starts_with("network|"));
        let full = anyhow::anyhow!("No space left on device (os error 28)");
        assert!(download(&full).starts_with("disk_full|"));
        let bad = anyhow::anyhow!("PLaMo failed its integrity check");
        assert!(download(&bad).starts_with("integrity|"));
    }
}
