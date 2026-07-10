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

pub(crate) fn app_server_broker_affinity_keys(value: &Value) -> Vec<AppServerBrokerAffinityKey> {
    if let Some(binding) = app_server_broker_lifecycle_binding(value) {
        let mut keys = Vec::new();
        match binding.stage {
            AppServerBrokerLifecycleStage::InitializeRequest
            | AppServerBrokerLifecycleStage::InitializedNotification => {
                if let Some(session_id) = binding.metadata.session_id {
                    keys.push(AppServerBrokerAffinityKey {
                        kind: AppServerBrokerAffinityKeyKind::Session,
                        value: session_id,
                    });
                }
            }
            AppServerBrokerLifecycleStage::ThreadStartRequest
            | AppServerBrokerLifecycleStage::ThreadStartedNotification
            | AppServerBrokerLifecycleStage::ThreadResumeRequest
            | AppServerBrokerLifecycleStage::ThreadForkRequest => {
                if let Some(thread_id) = binding.metadata.thread_id {
                    keys.push(AppServerBrokerAffinityKey {
                        kind: AppServerBrokerAffinityKeyKind::Thread,
                        value: thread_id,
                    });
                }
                if let Some(session_id) = binding.metadata.session_id {
                    keys.push(AppServerBrokerAffinityKey {
                        kind: AppServerBrokerAffinityKeyKind::Session,
                        value: session_id,
                    });
                }
            }
            AppServerBrokerLifecycleStage::TurnStartRequest
            | AppServerBrokerLifecycleStage::TurnStartedNotification
            | AppServerBrokerLifecycleStage::TurnCompletedNotification
            | AppServerBrokerLifecycleStage::TurnInterruptRequest => {
                if let Some(turn_id) = binding.metadata.turn_id {
                    keys.push(AppServerBrokerAffinityKey {
                        kind: AppServerBrokerAffinityKeyKind::Turn,
                        value: turn_id,
                    });
                }
                if let Some(thread_id) = binding.metadata.thread_id {
                    keys.push(AppServerBrokerAffinityKey {
                        kind: AppServerBrokerAffinityKeyKind::Thread,
                        value: thread_id,
                    });
                }
                if let Some(session_id) = binding.metadata.session_id {
                    keys.push(AppServerBrokerAffinityKey {
                        kind: AppServerBrokerAffinityKeyKind::Session,
                        value: session_id,
                    });
                }
            }
        }
        return keys;
    }

    let summary = app_server_broker_diagnostic_summary(value);
    if !matches!(summary.frame_kind, AppServerBrokerFrameKind::Response) {
        return Vec::new();
    }

    let mut keys = Vec::new();
    if let Some(turn_id) = summary.metadata.turn_id {
        keys.push(AppServerBrokerAffinityKey {
            kind: AppServerBrokerAffinityKeyKind::Turn,
            value: turn_id,
        });
    }
    if let Some(thread_id) = summary.metadata.thread_id {
        keys.push(AppServerBrokerAffinityKey {
            kind: AppServerBrokerAffinityKeyKind::Thread,
            value: thread_id,
        });
    }
    if let Some(session_id) = summary.metadata.session_id {
        keys.push(AppServerBrokerAffinityKey {
            kind: AppServerBrokerAffinityKeyKind::Session,
            value: session_id,
        });
    }
    keys
}

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
