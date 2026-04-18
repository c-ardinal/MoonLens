//! Plugin signature verification.
//!
//! The signature scheme is intentionally small: a plugin's
//! `plugin.toml [plugin].signature` holds a base64-encoded Ed25519
//! signature over the SHA-256 digest of the plugin's `entry` file. The
//! keys of every publisher the user trusts live in
//! `settings.plugins.trusted_publishers` as
//! `"publisher_id=<base64 pubkey>"` strings.
//!
//! Enforcement is off by default. When `settings.plugins.require_signature`
//! is true, any plugin that fails to verify is refused regardless of
//! the `enabled` flag.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// Parse a `trusted_publishers` entry formatted as
/// `"<publisher_id>=<base64 pubkey>"`. Unknown shapes are rejected so
/// typos in settings do not silently disable verification.
pub fn parse_trusted(entry: &str) -> Result<(String, VerifyingKey)> {
    let (id, key) = entry
        .split_once('=')
        .ok_or_else(|| anyhow!("trusted publisher must be `id=base64pubkey`"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(key.trim())
        .with_context(|| format!("base64 decode pubkey for {id}"))?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("pubkey must be 32 bytes"))?;
    let key = VerifyingKey::from_bytes(&arr).context("pubkey is not a valid Ed25519 key")?;
    Ok((id.trim().to_string(), key))
}

/// Compute the SHA-256 digest of a file as a byte vector.
pub fn sha256_file(path: &Path) -> Result<Vec<u8>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read {} for hashing", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

/// Pure digest over arbitrary bytes. Exposed so tests can assert
/// against known vectors without touching the filesystem.
pub fn sha256_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().to_vec()
}

/// Verify that `signature` is an Ed25519 signature over `digest` made
/// by any of the provided trusted keys. Returns the matching publisher
/// id on success.
pub fn verify_digest(
    digest: &[u8],
    signature_b64: &str,
    trusted: &[(String, VerifyingKey)],
) -> Result<String> {
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .context("signature is not valid base64")?;
    let sig_array: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("signature must be 64 bytes"))?;
    let sig = Signature::from_bytes(&sig_array);
    for (id, key) in trusted {
        if key.verify(digest, &sig).is_ok() {
            return Ok(id.clone());
        }
    }
    Err(anyhow!("no trusted key accepted the signature"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn make_keypair() -> (SigningKey, VerifyingKey) {
        // Deterministic key from a fixed seed; keeps the test
        // reproducible across runs without pulling `rand` in.
        let seed = [7u8; 32];
        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn parse_trusted_accepts_valid_entry() {
        let (_sk, vk) = make_keypair();
        let entry = format!("acme={}", b64(vk.as_bytes()));
        let (id, parsed_vk) = parse_trusted(&entry).unwrap();
        assert_eq!(id, "acme");
        assert_eq!(parsed_vk.as_bytes(), vk.as_bytes());
    }

    #[test]
    fn parse_trusted_tolerates_whitespace() {
        let (_sk, vk) = make_keypair();
        let entry = format!("  acme  =  {}  ", b64(vk.as_bytes()));
        let (id, _) = parse_trusted(&entry).unwrap();
        assert_eq!(id, "acme");
    }

    #[test]
    fn parse_trusted_rejects_missing_separator() {
        assert!(parse_trusted("acme-without-equals").is_err());
    }

    #[test]
    fn parse_trusted_rejects_short_key() {
        assert!(parse_trusted("acme=shortkey").is_err());
    }

    #[test]
    fn sha256_is_deterministic() {
        let a = sha256_bytes(b"hello");
        let b = sha256_bytes(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn verify_digest_accepts_a_known_signer() {
        let (sk, vk) = make_keypair();
        let digest = sha256_bytes(b"plugin.wasm contents");
        let sig = sk.sign(&digest);
        let trusted = vec![("acme".to_string(), vk)];
        let id = verify_digest(&digest, &b64(sig.to_bytes().as_ref()), &trusted).unwrap();
        assert_eq!(id, "acme");
    }

    #[test]
    fn verify_digest_rejects_tampered_payload() {
        let (sk, vk) = make_keypair();
        let digest = sha256_bytes(b"plugin.wasm contents");
        let sig = sk.sign(&digest);
        // Flip one byte of the digest; signature should no longer verify.
        let mut tampered = digest.clone();
        tampered[0] ^= 0xFF;
        let trusted = vec![("acme".to_string(), vk)];
        assert!(verify_digest(&tampered, &b64(sig.to_bytes().as_ref()), &trusted).is_err());
    }

    #[test]
    fn verify_digest_rejects_unknown_signer() {
        let (sk_a, _) = make_keypair();
        let digest = sha256_bytes(b"x");
        let sig = sk_a.sign(&digest);
        // Trust list contains a DIFFERENT verifying key.
        let seed_b = [9u8; 32];
        let vk_b = SigningKey::from_bytes(&seed_b).verifying_key();
        let trusted = vec![("beta".to_string(), vk_b)];
        assert!(verify_digest(&digest, &b64(sig.to_bytes().as_ref()), &trusted).is_err());
    }

    #[test]
    fn verify_digest_accepts_any_match_from_multiple_trusted() {
        let (sk_a, vk_a) = make_keypair();
        let digest = sha256_bytes(b"y");
        let sig = sk_a.sign(&digest);
        // First trusted entry is a different key; second one is the
        // signer. Verification must still succeed.
        let seed_other = [3u8; 32];
        let vk_other = SigningKey::from_bytes(&seed_other).verifying_key();
        let trusted = vec![("other".to_string(), vk_other), ("acme".to_string(), vk_a)];
        let id = verify_digest(&digest, &b64(sig.to_bytes().as_ref()), &trusted).unwrap();
        assert_eq!(id, "acme");
    }
}
