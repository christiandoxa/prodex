use super::*;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub fn smart_context_hash_text(text: &str) -> String {
    format!("sc:{:016x}", smart_context_fnv1a64(text.as_bytes()))
}

pub fn smart_context_normalized_command_output_hash_text(text: &str) -> String {
    let normalized = smart_context_normalize_volatile_command_output(text);
    format!(
        "scv:{:016x}",
        smart_context_fnv1a64(normalized.as_ref().as_bytes())
    )
}

pub(in crate::smart_context) fn smart_context_fingerprint_map(
    fingerprints: impl IntoIterator<Item = SmartContextFingerprint>,
) -> BTreeMap<(SmartContextFingerprintKind, String), SmartContextFingerprint> {
    fingerprints
        .into_iter()
        .map(|fingerprint| ((fingerprint.kind, fingerprint.id.clone()), fingerprint))
        .collect()
}

pub(in crate::smart_context) fn smart_context_available_artifacts_by_hash_and_len(
    artifacts: impl IntoIterator<Item = SmartContextArtifactRef>,
) -> BTreeMap<(String, usize), SmartContextArtifactRef> {
    let mut artifacts = artifacts
        .into_iter()
        .filter(|artifact| non_empty(&artifact.id) && non_empty(&artifact.content_hash))
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.content_hash
            .cmp(&right.content_hash)
            .then_with(|| left.byte_len.cmp(&right.byte_len))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut available = BTreeMap::new();
    for artifact in artifacts {
        available
            .entry((artifact.content_hash.clone(), artifact.byte_len))
            .or_insert(artifact);
    }

    available
}

pub(in crate::smart_context) fn smart_context_summary_prefix(
    text: &str,
    byte_limit: usize,
) -> String {
    let mut summary = String::new();
    for value in text.chars() {
        let next_len = summary.len() + value.len_utf8();
        if next_len > byte_limit {
            break;
        }
        summary.push(value);
    }
    summary
}

pub(in crate::smart_context) fn smart_context_artifact_marker_line(
    kind: &str,
    artifact: &SmartContextArtifactRef,
) -> String {
    let reference = smart_context_short_artifact_ref(&artifact.id);
    let kind = match kind {
        "artifact" => "art",
        other => other,
    };
    format!(
        "psc {kind} {reference} b={} lines=#Lx-Ly",
        artifact.byte_len
    )
}

pub(in crate::smart_context) fn smart_context_short_artifact_label(id: &str) -> &str {
    id.strip_prefix("sc:").unwrap_or(id)
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

pub(in crate::smart_context) fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}
