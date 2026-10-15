use super::*;
use sha2::{Digest as _, Sha256};
use std::fmt::Write as _;

pub fn smart_context_hash_text(text: &str) -> String {
    format!("sc2:{}", smart_context_sha256_hex(text.as_bytes()))
}

pub fn smart_context_hash_matches_text(hash: &str, text: &str) -> bool {
    if hash.starts_with("sc2:") {
        return hash == smart_context_hash_text(text);
    }
    hash.strip_prefix("sc:").is_some_and(|value| {
        value
            == prodex_mojo_core::rich::smart_context_legacy_fnv_text(text.as_bytes(), false)
                .expect("compiled Mojo legacy Smart Context digest failed")
    })
}

pub fn smart_context_normalized_command_output_hash_text(text: &str) -> String {
    let normalized = smart_context_normalize_volatile_command_output(text);
    prodex_mojo_core::rich::smart_context_legacy_fnv_text(normalized.as_ref().as_bytes(), true)
        .expect("compiled Mojo Smart Context volatile digest failed")
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
