use super::*;
use crate::runtime_background::{
    runtime_load_json_file_or_default, runtime_save_merged_json_file,
    schedule_runtime_smart_context_token_calibration_save_job,
};
use anyhow::Result;
use redaction::redaction_redact_secret_like_text;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextTokenCalibrationObservation {
    pub(super) bucket_key: runtime_proxy_crate::SmartContextTokenCalibrationBucketKey,
    pub(super) usage: RuntimeTokenUsage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedTokenCalibration {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    pub(super) token_usage_history: Vec<RuntimeSmartContextPersistedTokenUsage>,
    #[serde(default)]
    pub(super) token_calibration_history:
        Vec<RuntimeSmartContextPersistedTokenCalibrationObservation>,
    #[serde(default)]
    pub(super) rewrite_safety_history: Vec<RuntimeSmartContextPersistedRewriteSafetyObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) artifact_aliases: Vec<RuntimeSmartContextPersistedArtifactAlias>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) static_section_fingerprints:
        Vec<RuntimeSmartContextPersistedStaticSectionFingerprint>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedTokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedTokenCalibrationObservation {
    #[serde(default)]
    bucket_key: RuntimeSmartContextPersistedTokenCalibrationBucketKey,
    #[serde(default)]
    usage: RuntimeSmartContextPersistedTokenUsage,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedRewriteSafetyObservation {
    #[serde(default)]
    safe: bool,
    #[serde(default)]
    saved_tokens: u64,
    #[serde(default)]
    observed_at_unix_secs: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedArtifactAlias {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) alias: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedStaticSectionFingerprint {
    #[serde(default)]
    item_id: String,
    #[serde(default)]
    heading: String,
    #[serde(default)]
    ordinal: usize,
    #[serde(default)]
    content_hash: String,
    #[serde(default)]
    byte_len: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct RuntimeSmartContextPersistedTokenCalibrationBucketKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
}

impl From<RuntimeSmartContextPersistedTokenUsage> for RuntimeTokenUsage {
    fn from(value: RuntimeSmartContextPersistedTokenUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: value.cached_input_tokens,
            output_tokens: value.output_tokens,
            reasoning_tokens: value.reasoning_tokens,
        }
    }
}

impl From<RuntimeTokenUsage> for RuntimeSmartContextPersistedTokenUsage {
    fn from(value: RuntimeTokenUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: value.cached_input_tokens,
            output_tokens: value.output_tokens,
            reasoning_tokens: value.reasoning_tokens,
        }
    }
}

impl From<RuntimeSmartContextPersistedTokenCalibrationBucketKey>
    for runtime_proxy_crate::SmartContextTokenCalibrationBucketKey
{
    fn from(value: RuntimeSmartContextPersistedTokenCalibrationBucketKey) -> Self {
        Self {
            route: value.route,
            model: value.model,
            profile: value.profile,
            transport: value.transport,
        }
    }
}

impl From<runtime_proxy_crate::SmartContextTokenCalibrationBucketKey>
    for RuntimeSmartContextPersistedTokenCalibrationBucketKey
{
    fn from(value: runtime_proxy_crate::SmartContextTokenCalibrationBucketKey) -> Self {
        Self {
            route: value.route,
            model: value.model,
            profile: value.profile,
            transport: value.transport,
        }
    }
}

impl From<RuntimeSmartContextPersistedTokenCalibrationObservation>
    for RuntimeSmartContextTokenCalibrationObservation
{
    fn from(value: RuntimeSmartContextPersistedTokenCalibrationObservation) -> Self {
        Self {
            bucket_key: value.bucket_key.into(),
            usage: value.usage.into(),
        }
    }
}

impl From<&RuntimeSmartContextTokenCalibrationObservation>
    for RuntimeSmartContextPersistedTokenCalibrationObservation
{
    fn from(value: &RuntimeSmartContextTokenCalibrationObservation) -> Self {
        Self {
            bucket_key: value.bucket_key.clone().into(),
            usage: value.usage.into(),
        }
    }
}

impl From<RuntimeSmartContextPersistedRewriteSafetyObservation>
    for RuntimeSmartContextRewriteSafetyRecord
{
    fn from(value: RuntimeSmartContextPersistedRewriteSafetyObservation) -> Self {
        Self {
            observation: RuntimeSmartContextRewriteSafetyObservation {
                safe: value.safe,
                saved_tokens: value.saved_tokens,
            },
            observed_at_unix_secs: value.observed_at_unix_secs,
        }
    }
}

impl From<RuntimeSmartContextRewriteSafetyRecord>
    for RuntimeSmartContextPersistedRewriteSafetyObservation
{
    fn from(value: RuntimeSmartContextRewriteSafetyRecord) -> Self {
        Self {
            safe: value.observation.safe,
            saved_tokens: value.observation.saved_tokens,
            observed_at_unix_secs: value.observed_at_unix_secs,
        }
    }
}

impl From<RuntimeSmartContextStaticSectionFingerprint>
    for RuntimeSmartContextPersistedStaticSectionFingerprint
{
    fn from(value: RuntimeSmartContextStaticSectionFingerprint) -> Self {
        Self {
            item_id: value.item_id,
            heading: value.heading,
            ordinal: value.ordinal,
            content_hash: value.content_hash,
            byte_len: value.byte_len,
        }
    }
}

impl From<RuntimeSmartContextPersistedStaticSectionFingerprint>
    for RuntimeSmartContextStaticSectionFingerprint
{
    fn from(value: RuntimeSmartContextPersistedStaticSectionFingerprint) -> Self {
        Self {
            item_id: value.item_id,
            heading: value.heading,
            ordinal: value.ordinal,
            content_hash: value.content_hash,
            byte_len: value.byte_len,
        }
    }
}

pub(super) fn runtime_smart_context_token_calibration_path(artifact_path: &Path) -> PathBuf {
    let mut path = artifact_path.to_path_buf();
    let file_name = artifact_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| format!("{value}.token-calibration.json"))
        .unwrap_or_else(|| "smart-context-token-calibration.json".to_string());
    path.set_file_name(file_name);
    path
}

pub(super) fn runtime_smart_context_load_token_calibration_for_artifact_path(
    artifact_path: &Path,
) -> RuntimeSmartContextPersistedTokenCalibration {
    let path = runtime_smart_context_token_calibration_path(artifact_path);
    runtime_smart_context_load_token_calibration_path(&path)
}

pub(super) fn runtime_smart_context_token_calibration_snapshot(
    state: &RuntimeSmartContextProxyState,
) -> RuntimeSmartContextPersistedTokenCalibration {
    let now = runtime_smart_context_unix_secs_now();
    RuntimeSmartContextPersistedTokenCalibration {
        version: SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION,
        token_usage_history: state
            .token_usage_history
            .iter()
            .copied()
            .map(RuntimeSmartContextPersistedTokenUsage::from)
            .collect(),
        token_calibration_history: state
            .token_calibration_history
            .iter()
            .map(RuntimeSmartContextPersistedTokenCalibrationObservation::from)
            .collect(),
        rewrite_safety_history: state
            .rewrite_safety_history
            .iter()
            .copied()
            .filter(|record| runtime_smart_context_rewrite_safety_record_fresh(*record, now))
            .map(RuntimeSmartContextPersistedRewriteSafetyObservation::from)
            .collect(),
        artifact_aliases: runtime_smart_context_persisted_artifact_aliases(state),
        static_section_fingerprints: state
            .static_section_fingerprints
            .values()
            .take(SMART_CONTEXT_PERSISTED_STATIC_SECTION_LIMIT)
            .cloned()
            .map(RuntimeSmartContextPersistedStaticSectionFingerprint::from)
            .collect(),
    }
}

pub(super) fn schedule_runtime_smart_context_token_calibration_save(
    shared: &RuntimeRotationProxyShared,
    path: PathBuf,
    snapshot: RuntimeSmartContextPersistedTokenCalibration,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "smart_context_token_calibration_save_suppressed",
                [
                    runtime_proxy_log_field("role", "follower"),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("path", path.display().to_string()),
                ],
            ),
        );
        return;
    }

    if cfg!(test) {
        match runtime_smart_context_save_token_calibration_snapshot(&path, snapshot.clone()) {
            Ok(()) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_ok",
                    [
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field(
                            "samples",
                            snapshot.token_calibration_history.len().to_string(),
                        ),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_error",
                    [
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field(
                            "error",
                            runtime_smart_context_token_calibration_error_log_value(&err),
                        ),
                    ],
                ),
            ),
        }
        return;
    }

    let sample_count = snapshot.token_calibration_history.len();
    let save_snapshot = snapshot;
    let backlog = schedule_runtime_smart_context_token_calibration_save_job(
        path.clone(),
        shared.log_path.clone(),
        reason.to_string(),
        sample_count,
        SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_DELAY_MS,
        move |path| runtime_smart_context_save_token_calibration_snapshot(path, save_snapshot),
    );

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_token_calibration_save_queued",
            [
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field(
                    "ready_in_ms",
                    SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_DELAY_MS.to_string(),
                ),
            ],
        ),
    );
}

fn runtime_smart_context_token_calibration_error_log_value(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&format!("{err:#}")).replace('\n', " ")
}

fn runtime_smart_context_save_token_calibration_snapshot(
    path: &Path,
    snapshot: RuntimeSmartContextPersistedTokenCalibration,
) -> Result<()> {
    runtime_save_merged_json_file(
        path,
        snapshot,
        |calibration: &RuntimeSmartContextPersistedTokenCalibration| {
            calibration.version == SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION
        },
        runtime_smart_context_merge_persisted_token_calibration,
    )
}

fn runtime_smart_context_load_token_calibration_path(
    path: &Path,
) -> RuntimeSmartContextPersistedTokenCalibration {
    runtime_load_json_file_or_default(
        path,
        |calibration: &RuntimeSmartContextPersistedTokenCalibration| {
            calibration.version == SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION
        },
    )
}

fn runtime_smart_context_merge_persisted_token_calibration(
    mut existing: RuntimeSmartContextPersistedTokenCalibration,
    incoming: RuntimeSmartContextPersistedTokenCalibration,
) -> RuntimeSmartContextPersistedTokenCalibration {
    existing.version = SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION;
    runtime_smart_context_extend_unique_tail(
        &mut existing.token_usage_history,
        incoming.token_usage_history,
    );
    runtime_smart_context_truncate_vec_tail(
        &mut existing.token_usage_history,
        SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT,
    );

    runtime_smart_context_extend_unique_tail(
        &mut existing.token_calibration_history,
        incoming.token_calibration_history,
    );
    runtime_smart_context_truncate_vec_tail(
        &mut existing.token_calibration_history,
        SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT,
    );

    let now = runtime_smart_context_unix_secs_now();
    existing
        .rewrite_safety_history
        .extend(incoming.rewrite_safety_history);
    existing.rewrite_safety_history.retain(|record| {
        runtime_smart_context_rewrite_safety_record_fresh(
            RuntimeSmartContextRewriteSafetyRecord::from(*record),
            now,
        )
    });
    existing
        .rewrite_safety_history
        .sort_by_key(|record| record.observed_at_unix_secs);
    runtime_smart_context_truncate_vec_tail(
        &mut existing.rewrite_safety_history,
        SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT,
    );

    existing.artifact_aliases = runtime_smart_context_merge_persisted_artifact_aliases(
        existing.artifact_aliases,
        incoming.artifact_aliases,
    );
    existing.static_section_fingerprints =
        runtime_smart_context_merge_persisted_static_section_fingerprints(
            existing.static_section_fingerprints,
            incoming.static_section_fingerprints,
        );
    existing
}

fn runtime_smart_context_truncate_vec_tail<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let overflow = items.len().saturating_sub(limit);
        items.drain(0..overflow);
    }
}

fn runtime_smart_context_extend_unique_tail<T: PartialEq>(items: &mut Vec<T>, incoming: Vec<T>) {
    for item in incoming {
        if !items.contains(&item) {
            items.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_context_token_calibration_error_log_value_redacts_secret_like_chain() {
        let err = anyhow::anyhow!(
            "calibration save failed\nAuthorization: Bearer calibration-token\napi_key=calibration-key"
        )
        .context("smart context token calibration save failed");
        let message = runtime_smart_context_token_calibration_error_log_value(&err);

        assert!(!message.contains('\n'));
        assert!(message.contains("smart context token calibration save failed"));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("calibration-token"));
        assert!(!message.contains("calibration-key"));
    }
}
