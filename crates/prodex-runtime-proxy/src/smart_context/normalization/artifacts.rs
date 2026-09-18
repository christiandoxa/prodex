use super::*;
use sha2::{Digest as _, Sha256};
use std::cmp::Ordering;
#[cfg(any(not(feature = "mojo"), test))]
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn smart_context_hash_text(text: &str) -> String {
    format!("sc2:{}", smart_context_sha256_hex(text.as_bytes()))
}

pub fn smart_context_hash_matches_text(hash: &str, text: &str) -> bool {
    if hash.starts_with("sc2:") {
        return hash == smart_context_hash_text(text);
    }
    hash.strip_prefix("sc:")
        .is_some_and(|value| value == format!("{:016x}", smart_context_fnv1a64(text.as_bytes())))
}

pub fn smart_context_normalized_command_output_hash_text(text: &str) -> String {
    let normalized = smart_context_normalize_volatile_command_output(text);
    format!(
        "scv:{:016x}",
        smart_context_fnv1a64(normalized.as_ref().as_bytes())
    )
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_fingerprint_map(
    fingerprints: impl IntoIterator<Item = SmartContextFingerprint>,
) -> BTreeMap<(SmartContextFingerprintKind, String), SmartContextFingerprint> {
    fingerprints
        .into_iter()
        .map(|fingerprint| ((fingerprint.kind, fingerprint.id.clone()), fingerprint))
        .collect()
}

pub(in crate::smart_context) fn smart_context_capsule_order(
    left: &SmartContextMemoryCapsule,
    right: &SmartContextMemoryCapsule,
) -> Ordering {
    right
        .relevance
        .partial_cmp(&left.relevance)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.token_cost.cmp(&right.token_cost))
        .then_with(|| left.id.cmp(&right.id))
}

pub(in crate::smart_context) fn smart_context_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(in crate::smart_context) fn smart_context_sha256_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn smart_context_sha256_hex(bytes: &[u8]) -> String {
    let digest = smart_context_sha256_digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

pub(in crate::smart_context) fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}
