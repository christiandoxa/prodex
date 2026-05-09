use super::*;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactLineRangeRef {
    pub artifact_id: String,
    pub artifact_content_hash: String,
    pub artifact_byte_len: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub excerpt_hash: String,
    pub excerpt_byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactLineRange {
    pub reference: SmartContextArtifactLineRangeRef,
    pub excerpt: String,
}

pub fn smart_context_artifact_line_range(
    artifact: &SmartContextArtifactRef,
    artifact_text: &str,
    start_line: usize,
    end_line: usize,
) -> Option<SmartContextArtifactLineRange> {
    if artifact.content_hash != smart_context_hash_text(artifact_text) {
        return None;
    }

    let excerpt = smart_context_extract_line_range(artifact_text, start_line, end_line)?;
    let reference = SmartContextArtifactLineRangeRef {
        artifact_id: artifact.id.clone(),
        artifact_content_hash: artifact.content_hash.clone(),
        artifact_byte_len: artifact.byte_len,
        start_line,
        end_line,
        excerpt_hash: smart_context_hash_text(&excerpt),
        excerpt_byte_len: excerpt.len(),
    };

    Some(SmartContextArtifactLineRange { reference, excerpt })
}

pub fn smart_context_extract_line_range(
    text: &str,
    start_line: usize,
    end_line: usize,
) -> Option<String> {
    if start_line == 0 || end_line < start_line {
        return None;
    }

    let mut selected = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line_number > end_line {
            break;
        }
        if line_number >= start_line {
            selected.push(line);
        }
    }

    (!selected.is_empty()).then(|| selected.join("\n"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextFingerprintKind {
    StaticContext,
    ConversationTurn,
    ToolOutput,
    Artifact,
    MemoryCapsule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextFingerprintInput {
    pub id: String,
    pub kind: SmartContextFingerprintKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextFingerprint {
    pub id: String,
    pub kind: SmartContextFingerprintKind,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStableStaticContextItem {
    pub id: String,
    pub canonical_text: String,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextPromptCacheFingerprint {
    pub content_hash: String,
    pub items: Vec<SmartContextStableStaticContextItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextFingerprintChange {
    Added {
        fingerprint: SmartContextFingerprint,
    },
    Removed {
        fingerprint: SmartContextFingerprint,
    },
    Unchanged {
        fingerprint: SmartContextFingerprint,
    },
    Changed {
        before: SmartContextFingerprint,
        after: SmartContextFingerprint,
    },
}

pub fn smart_context_fingerprint(input: SmartContextFingerprintInput) -> SmartContextFingerprint {
    SmartContextFingerprint {
        id: input.id,
        kind: input.kind,
        content_hash: smart_context_hash_text(&input.text),
        byte_len: input.text.len(),
    }
}

pub fn smart_context_fingerprints(
    inputs: impl IntoIterator<Item = SmartContextFingerprintInput>,
) -> Vec<SmartContextFingerprint> {
    inputs.into_iter().map(smart_context_fingerprint).collect()
}

pub fn smart_context_stabilize_static_context_text(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !smart_context_static_context_noise_line(line))
        .map(|line| smart_context_normalize_volatile_static_context(line).into_owned())
        .collect::<Vec<_>>();

    let Some(start) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .unwrap_or(start);

    lines[start..=end].join("\n")
}

pub fn smart_context_stabilize_static_context_items(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> Vec<SmartContextStableStaticContextItem> {
    let mut items = items
        .into_iter()
        .filter_map(|item| {
            let id = smart_context_stabilize_static_context_id(&item.id);
            let canonical_text = smart_context_stabilize_static_context_text(&item.text);
            if id.is_empty() && canonical_text.is_empty() {
                return None;
            }
            let content_hash = smart_context_hash_text(&canonical_text);
            Some(SmartContextStableStaticContextItem {
                id,
                byte_len: canonical_text.len(),
                canonical_text,
                content_hash,
            })
        })
        .collect::<Vec<_>>();

    items.sort_by(smart_context_static_context_item_order);
    items
}

pub fn smart_context_static_context_prompt_cache_fingerprint(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> SmartContextStaticContextPromptCacheFingerprint {
    let items = smart_context_stabilize_static_context_items(items);
    let payload = smart_context_static_context_prompt_cache_payload(&items);

    SmartContextStaticContextPromptCacheFingerprint {
        content_hash: format!("scpc:{:016x}", smart_context_fnv1a64(payload.as_bytes())),
        items,
    }
}

pub fn smart_context_fingerprint_delta(
    previous: impl IntoIterator<Item = SmartContextFingerprint>,
    current: impl IntoIterator<Item = SmartContextFingerprint>,
) -> Vec<SmartContextFingerprintChange> {
    let previous = smart_context_fingerprint_map(previous);
    let current = smart_context_fingerprint_map(current);
    let mut keys = BTreeSet::new();
    keys.extend(previous.keys().cloned());
    keys.extend(current.keys().cloned());

    keys.into_iter()
        .filter_map(|key| match (previous.get(&key), current.get(&key)) {
            (None, Some(after)) => Some(SmartContextFingerprintChange::Added {
                fingerprint: after.clone(),
            }),
            (Some(before), None) => Some(SmartContextFingerprintChange::Removed {
                fingerprint: before.clone(),
            }),
            (Some(before), Some(after)) if before.content_hash == after.content_hash => {
                Some(SmartContextFingerprintChange::Unchanged {
                    fingerprint: after.clone(),
                })
            }
            (Some(before), Some(after)) => Some(SmartContextFingerprintChange::Changed {
                before: before.clone(),
                after: after.clone(),
            }),
            (None, None) => None,
        })
        .collect()
}
