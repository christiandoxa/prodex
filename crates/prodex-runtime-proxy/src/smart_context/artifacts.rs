use super::*;
use std::collections::BTreeMap;

pub fn smart_context_artifact_marker(
    artifact: &SmartContextArtifactRef,
    compacted: &str,
) -> String {
    let marker = smart_context_artifact_marker_line("artifact", artifact);
    if compacted.is_empty() {
        return marker;
    }
    format!("{marker}\n{compacted}")
}

pub fn smart_context_artifact_reference_marker(artifact: &SmartContextArtifactRef) -> String {
    format!(
        "psc rep {} b={}",
        smart_context_short_artifact_ref(&artifact.id),
        artifact.byte_len,
    )
}

pub fn smart_context_short_artifact_ref(id: &str) -> String {
    format!(
        "{SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX}{}",
        smart_context_short_artifact_label(id)
    )
}

pub fn smart_context_short_artifact_line_ref(id: &str, start: usize, end: usize) -> String {
    format!("{}#L{start}-L{end}", smart_context_short_artifact_ref(id))
}

pub fn smart_context_condense_tool_outputs(
    outputs: impl IntoIterator<Item = SmartContextToolOutput>,
    inline_byte_limit: usize,
) -> Vec<SmartContextCondensedToolOutput> {
    outputs
        .into_iter()
        .map(|output| {
            let content_hash = smart_context_hash_text(&output.text);
            if output.text.len() <= inline_byte_limit {
                return SmartContextCondensedToolOutput::Inline {
                    call_id: output.call_id,
                    text: output.text,
                    content_hash,
                };
            }

            match output.artifact {
                Some(artifact) if artifact.content_hash == content_hash => {
                    SmartContextCondensedToolOutput::ArtifactBacked {
                        call_id: output.call_id,
                        summary: smart_context_summary_prefix(&output.text, inline_byte_limit),
                        artifact,
                        content_hash,
                    }
                }
                _ => SmartContextCondensedToolOutput::Inline {
                    call_id: output.call_id,
                    text: output.text,
                    content_hash,
                },
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextConversationItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextHashRef {
    pub id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextDedupeItem {
    Keep {
        id: String,
        content_hash: String,
    },
    Duplicate {
        id: String,
        ref_id: String,
        content_hash: String,
    },
}

pub fn smart_context_conversation_dedupe(
    items: impl IntoIterator<Item = SmartContextConversationItem>,
) -> Vec<SmartContextDedupeItem> {
    let mut seen = BTreeMap::<String, String>::new();
    items
        .into_iter()
        .map(|item| {
            let content_hash = smart_context_normalized_command_output_hash_text(&item.text);
            if let Some(ref_id) = seen.get(&content_hash) {
                SmartContextDedupeItem::Duplicate {
                    id: item.id,
                    ref_id: ref_id.clone(),
                    content_hash,
                }
            } else {
                seen.insert(content_hash.clone(), item.id.clone());
                SmartContextDedupeItem::Keep {
                    id: item.id,
                    content_hash,
                }
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCrossTurnDuplicateKeepReason {
    ExactnessRequired,
    BelowMinByteThreshold,
    MissingArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextCrossTurnDuplicateRefAction {
    Keep {
        id: String,
        content_hash: String,
        byte_len: usize,
        reason: SmartContextCrossTurnDuplicateKeepReason,
    },
    ReplaceWithArtifactRef {
        id: String,
        artifact: SmartContextArtifactRef,
        content_hash: String,
        byte_len: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCrossTurnDuplicateRefPlan {
    pub actions: Vec<SmartContextCrossTurnDuplicateRefAction>,
    pub replaced_items: usize,
    pub replaced_bytes: usize,
}

pub fn smart_context_cross_turn_duplicate_ref_plan(
    items: impl IntoIterator<Item = SmartContextConversationItem>,
    available_artifacts: impl IntoIterator<Item = SmartContextArtifactRef>,
    min_replacement_bytes: usize,
    exactness_guard: &SmartContextExactnessGuard,
) -> SmartContextCrossTurnDuplicateRefPlan {
    let artifacts = smart_context_available_artifacts_by_hash_and_len(available_artifacts);
    let exactness_allows = exactness_guard.decision == SmartContextExactnessDecision::Allow;
    let mut actions = Vec::new();
    let mut replaced_items = 0usize;
    let mut replaced_bytes = 0usize;

    for item in items {
        let byte_len = item.text.len();
        let content_hash = smart_context_hash_text(&item.text);
        let action = if !exactness_allows {
            SmartContextCrossTurnDuplicateRefAction::Keep {
                id: item.id,
                content_hash,
                byte_len,
                reason: SmartContextCrossTurnDuplicateKeepReason::ExactnessRequired,
            }
        } else if byte_len < min_replacement_bytes {
            SmartContextCrossTurnDuplicateRefAction::Keep {
                id: item.id,
                content_hash,
                byte_len,
                reason: SmartContextCrossTurnDuplicateKeepReason::BelowMinByteThreshold,
            }
        } else if let Some(artifact) = artifacts.get(&(content_hash.clone(), byte_len)) {
            replaced_items += 1;
            replaced_bytes = replaced_bytes.saturating_add(byte_len);
            SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
                id: item.id,
                artifact: artifact.clone(),
                content_hash,
                byte_len,
            }
        } else {
            SmartContextCrossTurnDuplicateRefAction::Keep {
                id: item.id,
                content_hash,
                byte_len,
                reason: SmartContextCrossTurnDuplicateKeepReason::MissingArtifact,
            }
        };
        actions.push(action);
    }

    SmartContextCrossTurnDuplicateRefPlan {
        actions,
        replaced_items,
        replaced_bytes,
    }
}

pub fn smart_context_hash_refs(
    items: impl IntoIterator<Item = SmartContextConversationItem>,
) -> Vec<SmartContextHashRef> {
    items
        .into_iter()
        .map(|item| SmartContextHashRef {
            id: item.id,
            content_hash: smart_context_hash_text(&item.text),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextTokenBudgetTier {
    Exact,
    Large,
    Condensed,
    Minimal,
}
