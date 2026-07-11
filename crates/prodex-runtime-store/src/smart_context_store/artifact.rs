use super::*;

pub fn runtime_smart_context_artifact_content_hash(content: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

pub fn runtime_smart_context_artifact_from_content(
    key: impl Into<String>,
    content: impl Into<String>,
    now: i64,
) -> RuntimeSmartContextArtifact {
    let content = content.into();
    RuntimeSmartContextArtifact {
        key: key.into(),
        content_hash: runtime_smart_context_artifact_content_hash(content.as_bytes()),
        byte_len: content.len(),
        created_at: now,
        last_accessed_at: now,
        content,
    }
}

pub fn runtime_smart_context_upsert_artifact(
    store: &mut RuntimeSmartContextArtifactStore,
    key: impl Into<String>,
    content: impl Into<String>,
    now: i64,
) -> RuntimeSmartContextArtifact {
    let key = key.into();
    let content = content.into();
    let content_hash = runtime_smart_context_artifact_content_hash(content.as_bytes());
    let created_at = store
        .artifacts
        .get(&key)
        .filter(|artifact| artifact.content_hash == content_hash)
        .map(|artifact| artifact.created_at)
        .unwrap_or(now);
    let artifact = RuntimeSmartContextArtifact {
        key: key.clone(),
        content_hash,
        byte_len: content.len(),
        created_at,
        last_accessed_at: now,
        content,
    };
    store.artifacts.insert(key, artifact.clone());
    artifact
}

pub fn runtime_smart_context_touch_artifact<'a>(
    store: &'a mut RuntimeSmartContextArtifactStore,
    key: &str,
    now: i64,
) -> Option<&'a RuntimeSmartContextArtifact> {
    store.artifacts.get_mut(key).map(|artifact| {
        artifact.last_accessed_at = artifact.last_accessed_at.max(now);
        &*artifact
    })
}

pub fn runtime_smart_context_extract_line_range(
    content: &str,
    range: RuntimeSmartContextLineRange,
) -> Option<RuntimeSmartContextExtractedLineRange> {
    if range.start_line == 0 || range.end_line < range.start_line {
        return None;
    }

    let mut extracted = String::new();
    let mut end_line = None;
    for (index, line) in content.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        if line_number < range.start_line {
            continue;
        }
        if line_number > range.end_line {
            break;
        }
        extracted.push_str(line);
        end_line = Some(line_number);
    }

    end_line.map(|end_line| RuntimeSmartContextExtractedLineRange {
        start_line: range.start_line,
        end_line,
        content: extracted,
    })
}

pub fn runtime_smart_context_artifact_line_range(
    artifact: &RuntimeSmartContextArtifact,
    range: RuntimeSmartContextLineRange,
) -> Option<RuntimeSmartContextExtractedLineRange> {
    runtime_smart_context_extract_line_range(&artifact.content, range)
}

pub fn merge_runtime_smart_context_artifact_stores(
    mut existing: RuntimeSmartContextArtifactStore,
    incoming: RuntimeSmartContextArtifactStore,
) -> RuntimeSmartContextArtifactStore {
    existing.version = crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION;
    for (key, mut incoming_artifact) in incoming.artifacts {
        incoming_artifact.key = key.clone();
        existing
            .artifacts
            .entry(key)
            .and_modify(|current| {
                *current = runtime_smart_context_merge_artifact(current, &incoming_artifact);
            })
            .or_insert(incoming_artifact);
    }
    existing
}

pub fn compact_runtime_smart_context_artifact_store(
    mut store: RuntimeSmartContextArtifactStore,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> RuntimeSmartContextArtifactStore {
    store.version = crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION;
    store.artifacts.retain(|key, artifact| {
        artifact.key == *key
            && runtime_smart_context_artifact_should_retain_for_ttl(artifact, now, policy)
    });

    if store.artifacts.len() > policy.max_entries {
        let excess = store.artifacts.len() - policy.max_entries;
        let mut coldest = store
            .artifacts
            .iter()
            .map(|(key, artifact)| {
                (
                    key.clone(),
                    (
                        runtime_smart_context_artifact_retention_time(artifact),
                        artifact.created_at,
                        artifact.byte_len,
                    ),
                )
            })
            .collect::<Vec<_>>();
        coldest.sort_by_key(|(_, retention)| *retention);
        for (key, _) in coldest.into_iter().take(excess) {
            store.artifacts.remove(&key);
        }
    }

    store
}

pub(super) fn runtime_smart_context_merge_artifact(
    current: &RuntimeSmartContextArtifact,
    incoming: &RuntimeSmartContextArtifact,
) -> RuntimeSmartContextArtifact {
    if current.content_hash == incoming.content_hash && current.content == incoming.content {
        let mut merged = if incoming.last_accessed_at >= current.last_accessed_at {
            incoming.clone()
        } else {
            current.clone()
        };
        merged.created_at = current.created_at.min(incoming.created_at);
        merged.last_accessed_at = current.last_accessed_at.max(incoming.last_accessed_at);
        return merged;
    }

    let incoming_rank = (
        runtime_smart_context_artifact_retention_time(incoming),
        incoming.created_at,
        incoming.byte_len,
    );
    let current_rank = (
        runtime_smart_context_artifact_retention_time(current),
        current.created_at,
        current.byte_len,
    );
    if incoming_rank >= current_rank {
        incoming.clone()
    } else {
        current.clone()
    }
}

pub(super) fn runtime_smart_context_artifact_retention_time(
    artifact: &RuntimeSmartContextArtifact,
) -> i64 {
    artifact.created_at.max(artifact.last_accessed_at)
}

pub(super) fn runtime_smart_context_artifact_should_retain_for_ttl(
    artifact: &RuntimeSmartContextArtifact,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> bool {
    policy.ttl_seconds <= 0
        || now.saturating_sub(runtime_smart_context_artifact_retention_time(artifact))
            <= policy.ttl_seconds
}

pub(super) fn runtime_smart_context_validate_artifact(
    artifact: &RuntimeSmartContextArtifact,
) -> Result<(), RuntimeSmartContextArtifactStoreJsonError> {
    if artifact.key.is_empty() {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(
            "artifact key must not be empty",
        ));
    }
    if artifact.byte_len != artifact.content.len() {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "artifact {} byte_len mismatch",
            artifact.key
        )));
    }
    let content_hash = runtime_smart_context_artifact_content_hash(artifact.content.as_bytes());
    if artifact.content_hash != content_hash {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "artifact {} content_hash mismatch",
            artifact.key
        )));
    }
    Ok(())
}
