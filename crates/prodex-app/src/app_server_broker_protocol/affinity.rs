//! Affinity and safe-rotation policy helpers for app-server broker frames.

use super::report::app_server_broker_affinity_key_json;
use super::*;

pub(crate) fn app_server_broker_lifecycle_binding(
    value: &Value,
) -> Option<AppServerBrokerLifecycleBinding> {
    let summary = app_server_broker_diagnostic_summary(value);
    let stage = app_server_broker_lifecycle_stage(summary.method.as_deref(), summary.frame_kind)?;
    Some(AppServerBrokerLifecycleBinding {
        stage,
        metadata: summary.metadata,
    })
}

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_affinity_keys(value: &Value) -> Vec<AppServerBrokerAffinityKey> {
    let summary = app_server_broker_diagnostic_summary(value);
    super::mojo::affinity_plan(value)
        .keys
        .into_iter()
        .filter_map(|kind| {
            let value = match kind {
                AppServerBrokerAffinityKeyKind::Session => summary.metadata.session_id.clone(),
                AppServerBrokerAffinityKeyKind::Thread => summary.metadata.thread_id.clone(),
                AppServerBrokerAffinityKeyKind::Turn => summary.metadata.turn_id.clone(),
            }?;
            Some(AppServerBrokerAffinityKey { kind, value })
        })
        .collect()
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_affinity_keys(value: &Value) -> Vec<AppServerBrokerAffinityKey> {
    if let Some(binding) = app_server_broker_lifecycle_binding(value) {
        return app_server_broker_lifecycle_affinity_keys(binding);
    }

    let summary = app_server_broker_diagnostic_summary(value);
    if app_server_broker_thread_affinity_method(summary.method.as_deref(), summary.frame_kind) {
        return app_server_broker_response_affinity_keys(summary.metadata);
    }
    if !matches!(summary.frame_kind, AppServerBrokerFrameKind::Response) {
        return Vec::new();
    }
    app_server_broker_response_affinity_keys(summary.metadata)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_thread_affinity_method(
    method: Option<&str>,
    frame_kind: AppServerBrokerFrameKind,
) -> bool {
    if !matches!(
        frame_kind,
        AppServerBrokerFrameKind::Request | AppServerBrokerFrameKind::Notification
    ) {
        return false;
    }
    let Some(method) = method else {
        return false;
    };
    [
        "thread/archive",
        "thread/delete",
        "thread/unarchive",
        "thread/read",
        "thread/rollback",
        "thread/compact/start",
        "thread/settings/update",
        "thread/metadata/update",
        "thread/section/move",
        "thread/memoryMode/set",
        "thread/archived",
        "thread/deleted",
        "thread/unarchived",
        "thread/closed",
        "turn/steer",
    ]
    .iter()
    .any(|candidate| candidate.eq_ignore_ascii_case(method.trim()))
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_lifecycle_affinity_keys(
    binding: AppServerBrokerLifecycleBinding,
) -> Vec<AppServerBrokerAffinityKey> {
    let mut keys = Vec::new();
    match binding.stage {
        AppServerBrokerLifecycleStage::InitializeRequest
        | AppServerBrokerLifecycleStage::InitializedNotification => {
            push_affinity_key(
                &mut keys,
                AppServerBrokerAffinityKeyKind::Session,
                binding.metadata.session_id,
            );
        }
        AppServerBrokerLifecycleStage::ThreadStartRequest
        | AppServerBrokerLifecycleStage::ThreadStartedNotification
        | AppServerBrokerLifecycleStage::ThreadResumeRequest
        | AppServerBrokerLifecycleStage::ThreadForkRequest
        | AppServerBrokerLifecycleStage::ThreadQueueRequest
        | AppServerBrokerLifecycleStage::ThreadQueueChangedNotification
        | AppServerBrokerLifecycleStage::ThreadRevertRequest
        | AppServerBrokerLifecycleStage::ThreadRevertedNotification => {
            push_affinity_key(
                &mut keys,
                AppServerBrokerAffinityKeyKind::Thread,
                binding.metadata.thread_id,
            );
            push_affinity_key(
                &mut keys,
                AppServerBrokerAffinityKeyKind::Session,
                binding.metadata.session_id,
            );
        }
        AppServerBrokerLifecycleStage::TurnStartRequest
        | AppServerBrokerLifecycleStage::TurnStartedNotification
        | AppServerBrokerLifecycleStage::TurnCompletedNotification
        | AppServerBrokerLifecycleStage::TurnInterruptRequest => {
            push_affinity_key(
                &mut keys,
                AppServerBrokerAffinityKeyKind::Turn,
                binding.metadata.turn_id,
            );
            push_affinity_key(
                &mut keys,
                AppServerBrokerAffinityKeyKind::Thread,
                binding.metadata.thread_id,
            );
            push_affinity_key(
                &mut keys,
                AppServerBrokerAffinityKeyKind::Session,
                binding.metadata.session_id,
            );
        }
    }
    keys
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_response_affinity_keys(
    metadata: AppServerBrokerMetadata,
) -> Vec<AppServerBrokerAffinityKey> {
    let mut keys = Vec::new();
    push_affinity_key(
        &mut keys,
        AppServerBrokerAffinityKeyKind::Turn,
        metadata.turn_id,
    );
    push_affinity_key(
        &mut keys,
        AppServerBrokerAffinityKeyKind::Thread,
        metadata.thread_id,
    );
    push_affinity_key(
        &mut keys,
        AppServerBrokerAffinityKeyKind::Session,
        metadata.session_id,
    );
    keys
}

#[cfg(not(feature = "mojo-core"))]
fn push_affinity_key(
    keys: &mut Vec<AppServerBrokerAffinityKey>,
    kind: AppServerBrokerAffinityKeyKind,
    value: Option<String>,
) {
    if let Some(value) = value {
        keys.push(AppServerBrokerAffinityKey { kind, value });
    }
}

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_continuation_affinity_summary_json(value: &Value) -> Value {
    let binding = app_server_broker_lifecycle_binding(value);
    let keys = app_server_broker_affinity_keys(value);
    let primary = keys.first().map(app_server_broker_affinity_key_json);
    let owner_kind = match keys.first().map(|key| key.kind) {
        Some(AppServerBrokerAffinityKeyKind::Session) => {
            AppServerBrokerContinuationOwnerKind::Session
        }
        Some(AppServerBrokerAffinityKeyKind::Thread) => {
            AppServerBrokerContinuationOwnerKind::Thread
        }
        Some(AppServerBrokerAffinityKeyKind::Turn) => AppServerBrokerContinuationOwnerKind::Turn,
        None => AppServerBrokerContinuationOwnerKind::None,
    };
    serde_json::json!({
        "stage": binding.as_ref().map(|binding| binding.stage.label()),
        "owner_kind": owner_kind.label(),
        "owner": primary,
        "primary": primary,
        "key_count": keys.len(),
        "has_turn": keys.iter().any(|key| matches!(key.kind, AppServerBrokerAffinityKeyKind::Turn)),
        "has_thread": keys.iter().any(|key| matches!(key.kind, AppServerBrokerAffinityKeyKind::Thread)),
        "has_session": keys.iter().any(|key| matches!(key.kind, AppServerBrokerAffinityKeyKind::Session)),
    })
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_continuation_affinity_summary_json(value: &Value) -> Value {
    let binding = app_server_broker_lifecycle_binding(value);
    let keys = app_server_broker_affinity_keys(value);
    let primary = keys.first().map(app_server_broker_affinity_key_json);
    let owner_kind = match keys.first().map(|key| key.kind) {
        Some(AppServerBrokerAffinityKeyKind::Session) => {
            AppServerBrokerContinuationOwnerKind::Session
        }
        Some(AppServerBrokerAffinityKeyKind::Thread) => {
            AppServerBrokerContinuationOwnerKind::Thread
        }
        Some(AppServerBrokerAffinityKeyKind::Turn) => AppServerBrokerContinuationOwnerKind::Turn,
        None => AppServerBrokerContinuationOwnerKind::None,
    };
    let has_turn = keys
        .iter()
        .any(|key| matches!(key.kind, AppServerBrokerAffinityKeyKind::Turn));
    let has_thread = keys
        .iter()
        .any(|key| matches!(key.kind, AppServerBrokerAffinityKeyKind::Thread));
    let has_session = keys
        .iter()
        .any(|key| matches!(key.kind, AppServerBrokerAffinityKeyKind::Session));
    serde_json::json!({
        "stage": binding.as_ref().map(|binding| binding.stage.label()),
        "owner_kind": owner_kind.label(),
        "owner": primary,
        "primary": primary,
        "key_count": keys.len(),
        "has_turn": has_turn,
        "has_thread": has_thread,
        "has_session": has_session,
    })
}

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_continuation_decision(
    value: &Value,
) -> AppServerBrokerContinuationDecision {
    super::mojo::affinity_plan(value).decision
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_continuation_decision(
    value: &Value,
) -> AppServerBrokerContinuationDecision {
    match app_server_broker_affinity_keys(value)
        .first()
        .map(|key| key.kind)
    {
        Some(AppServerBrokerAffinityKeyKind::Turn) => {
            AppServerBrokerContinuationDecision::ContinueTurn
        }
        Some(AppServerBrokerAffinityKeyKind::Thread) => {
            AppServerBrokerContinuationDecision::ContinueThread
        }
        Some(AppServerBrokerAffinityKeyKind::Session) => {
            AppServerBrokerContinuationDecision::ContinueSession
        }
        None => AppServerBrokerContinuationDecision::Fresh,
    }
}

pub(crate) fn app_server_broker_policy_hint_json(value: &Value) -> Value {
    let hint = app_server_broker_policy_hint(value);
    serde_json::json!({
        "mode": hint.mode.label(),
        "routing_hint": hint.routing_hint.label(),
        "preserved_owner_kind": hint.preserved_owner_kind().map(|kind| kind.label()),
        "commit_boundary": hint.commit_boundary.label(),
        "rotation_window": hint.rotation_window.label(),
        "turn_committed": hint.turn_committed(),
        "affinity_required": hint.affinity_required(),
        "rotation_allowed": hint.rotation_allowed(),
        "preserves_owner": hint.preserves_owner(),
    })
}

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_policy_hint(value: &Value) -> AppServerBrokerPolicyHint {
    super::mojo::affinity_plan(value).policy_hint
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_policy_hint(value: &Value) -> AppServerBrokerPolicyHint {
    let commit_boundary = app_server_broker_commit_boundary(value);
    let rotation_window = app_server_broker_rotation_window(value);
    let decision = app_server_broker_continuation_decision(value);
    let mode = match decision {
        AppServerBrokerContinuationDecision::Fresh => {
            AppServerBrokerPolicyHintMode::FreshSelectionOk
        }
        AppServerBrokerContinuationDecision::ContinueSession => {
            AppServerBrokerPolicyHintMode::PreserveSessionAffinity
        }
        AppServerBrokerContinuationDecision::ContinueThread => {
            AppServerBrokerPolicyHintMode::PreserveThreadAffinity
        }
        AppServerBrokerContinuationDecision::ContinueTurn => {
            AppServerBrokerPolicyHintMode::PreserveTurnAffinity
        }
    };
    let routing_hint = match decision {
        AppServerBrokerContinuationDecision::Fresh => AppServerBrokerRoutingHint::FreshSelectOk,
        AppServerBrokerContinuationDecision::ContinueSession => {
            AppServerBrokerRoutingHint::PreserveSessionOwner
        }
        AppServerBrokerContinuationDecision::ContinueThread => {
            AppServerBrokerRoutingHint::PreserveThreadOwner
        }
        AppServerBrokerContinuationDecision::ContinueTurn => {
            AppServerBrokerRoutingHint::PreserveTurnOwner
        }
    };
    AppServerBrokerPolicyHint {
        mode,
        routing_hint,
        commit_boundary,
        rotation_window,
    }
}

#[cfg(test)]
pub(crate) fn app_server_broker_allows_provider_switch(
    value: &Value,
    explicit_override: bool,
) -> bool {
    explicit_override || app_server_broker_policy_hint(value).rotation_allowed()
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_commit_boundary(value: &Value) -> AppServerBrokerCommitBoundary {
    if let Some(binding) = app_server_broker_lifecycle_binding(value) {
        return match binding.stage {
            AppServerBrokerLifecycleStage::TurnStartedNotification
            | AppServerBrokerLifecycleStage::TurnCompletedNotification
            | AppServerBrokerLifecycleStage::TurnInterruptRequest => {
                AppServerBrokerCommitBoundary::TurnCommitted
            }
            AppServerBrokerLifecycleStage::InitializeRequest
            | AppServerBrokerLifecycleStage::InitializedNotification
            | AppServerBrokerLifecycleStage::ThreadStartRequest
            | AppServerBrokerLifecycleStage::ThreadStartedNotification
            | AppServerBrokerLifecycleStage::ThreadResumeRequest
            | AppServerBrokerLifecycleStage::ThreadForkRequest
            | AppServerBrokerLifecycleStage::ThreadQueueRequest
            | AppServerBrokerLifecycleStage::ThreadQueueChangedNotification
            | AppServerBrokerLifecycleStage::ThreadRevertRequest
            | AppServerBrokerLifecycleStage::ThreadRevertedNotification
            | AppServerBrokerLifecycleStage::TurnStartRequest => {
                AppServerBrokerCommitBoundary::Precommit
            }
        };
    }

    let summary = app_server_broker_diagnostic_summary(value);
    if matches!(summary.frame_kind, AppServerBrokerFrameKind::Response)
        && summary.metadata.turn_id.is_some()
    {
        AppServerBrokerCommitBoundary::TurnCommitted
    } else {
        AppServerBrokerCommitBoundary::Precommit
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_rotation_window(value: &Value) -> AppServerBrokerRotationWindow {
    let commit_boundary = app_server_broker_commit_boundary(value);
    let decision = app_server_broker_continuation_decision(value);
    if matches!(decision, AppServerBrokerContinuationDecision::Fresh)
        && matches!(commit_boundary, AppServerBrokerCommitBoundary::Precommit)
    {
        AppServerBrokerRotationWindow::Open
    } else {
        AppServerBrokerRotationWindow::Closed
    }
}
