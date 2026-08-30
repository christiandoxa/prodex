use super::{
    AppServerBrokerAffinityKeyKind, AppServerBrokerCommitBoundary,
    AppServerBrokerContinuationDecision, AppServerBrokerFrameKind, AppServerBrokerLifecycleStage,
    AppServerBrokerPolicyHint, AppServerBrokerPolicyHintMode, AppServerBrokerRotationWindow,
    AppServerBrokerRoutingHint,
};
use prodex_mojo_core::rich::{
    AppServerBrokerValidationInput, app_server_broker_classify_wire,
    app_server_broker_lifecycle_validation_reason, app_server_broker_normalize_method,
    app_server_broker_plan_affinity, app_server_broker_response_schema,
};
use serde_json::{Map, Value};

pub(super) struct AppServerBrokerMojoAffinityPlan {
    pub(super) keys: Vec<AppServerBrokerAffinityKeyKind>,
    pub(super) decision: AppServerBrokerContinuationDecision,
    pub(super) policy_hint: AppServerBrokerPolicyHint,
}

pub(super) fn wire_plan(
    object: &Map<String, Value>,
) -> prodex_mojo_core::rich::AppServerBrokerWirePlan {
    let jsonrpc_state = match object.get("jsonrpc") {
        None => 0,
        Some(value) if value.as_str() == Some("2.0") => 1,
        Some(_) => 2,
    };
    let id_kind = match object.get("id") {
        None => 0,
        Some(value) if value.is_string() || value.is_number() || value.is_null() => 1,
        Some(_) => 2,
    };
    let params_kind = match object.get("params") {
        None => 0,
        Some(value) if value.is_object() || value.is_array() => 1,
        Some(_) => 2,
    };
    let error_kind = match object.get("error") {
        None => 0,
        Some(value) if value.is_object() => 1,
        Some(_) => 2,
    };
    let error_code_kind = object.get("error").map_or(0, |error| {
        error
            .as_object()
            .and_then(|error| error.get("code"))
            .filter(|code| code.as_i64().is_some() || code.as_u64().is_some())
            .map_or(2, |_| 1)
    });
    let error_message_kind = object.get("error").map_or(0, |error| {
        error
            .as_object()
            .and_then(|error| error.get("message"))
            .filter(|value| value.is_string())
            .map_or(2, |_| 1)
    });
    let method = object.get("method").and_then(Value::as_str);
    let method_kind = match object.get("method") {
        None => 0,
        Some(value) if value.is_string() => 1,
        Some(_) => 2,
    };
    app_server_broker_classify_wire(
        prodex_mojo_core::rich::AppServerBrokerWireInput {
            jsonrpc_state,
            id_kind,
            params_kind,
            error_kind,
            error_code_kind,
            error_message_kind,
            method_kind,
            has_result: flag(object.contains_key("result")),
            has_error: flag(object.contains_key("error")),
        },
        method,
    )
    .unwrap_or_else(|error| panic!("Mojo app-server broker wire planning failed: {error:?}"))
}

fn flag(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub(super) fn frame_kind(code: i64) -> AppServerBrokerFrameKind {
    match code {
        0 => AppServerBrokerFrameKind::Invalid,
        1 => AppServerBrokerFrameKind::Batch,
        2 => AppServerBrokerFrameKind::Request,
        3 => AppServerBrokerFrameKind::Notification,
        4 => AppServerBrokerFrameKind::Response,
        _ => panic!("Mojo app-server broker returned unknown frame kind"),
    }
}

pub(super) fn invalid_reason(code: i64) -> Option<&'static str> {
    match code {
        0 => None,
        1 => Some("non_jsonrpc_version"),
        2 => Some("empty_batch"),
        3 => Some("batch_too_large"),
        4 => Some("nested_batch"),
        5 => Some("invalid_batch_member"),
        6 => Some("non_object_frame"),
        7 => Some("non_scalar_id"),
        8 => Some("non_container_params"),
        9 => Some("non_object_error"),
        10 => Some("non_integer_error_code"),
        11 => Some("non_string_error_message"),
        12 => Some("non_string_method"),
        13 => Some("invalid_method_name"),
        14 => Some("result_with_error"),
        15 => Some("missing_response_id"),
        16 => Some("method_with_result_or_error"),
        17 => Some("missing_method_and_response_payload"),
        _ => panic!("Mojo app-server broker returned unknown invalid reason"),
    }
}

pub(super) fn lifecycle_stage(code: i64) -> Option<AppServerBrokerLifecycleStage> {
    match code {
        0 => None,
        1 => Some(AppServerBrokerLifecycleStage::InitializeRequest),
        2 => Some(AppServerBrokerLifecycleStage::InitializedNotification),
        3 => Some(AppServerBrokerLifecycleStage::ThreadStartRequest),
        4 => Some(AppServerBrokerLifecycleStage::ThreadStartedNotification),
        5 => Some(AppServerBrokerLifecycleStage::ThreadResumeRequest),
        6 => Some(AppServerBrokerLifecycleStage::ThreadForkRequest),
        7 => Some(AppServerBrokerLifecycleStage::ThreadQueueRequest),
        8 => Some(AppServerBrokerLifecycleStage::ThreadQueueChangedNotification),
        9 => Some(AppServerBrokerLifecycleStage::ThreadRevertRequest),
        10 => Some(AppServerBrokerLifecycleStage::ThreadRevertedNotification),
        11 => Some(AppServerBrokerLifecycleStage::TurnStartRequest),
        12 => Some(AppServerBrokerLifecycleStage::TurnStartedNotification),
        13 => Some(AppServerBrokerLifecycleStage::TurnCompletedNotification),
        14 => Some(AppServerBrokerLifecycleStage::TurnInterruptRequest),
        _ => panic!("Mojo app-server broker returned unknown lifecycle stage"),
    }
}

pub(super) fn lifecycle_schema(code: i64) -> Option<&'static str> {
    match code {
        0 => None,
        1 => Some("ThreadStartParams.json"),
        2 => Some("ThreadStartedNotification.json"),
        3 => Some("ThreadResumeParams.json"),
        4 => Some("ThreadForkParams.json"),
        5 => Some("TurnStartParams.json"),
        6 => Some("TurnStartedNotification.json"),
        7 => Some("TurnCompletedNotification.json"),
        8 => Some("TurnInterruptParams.json"),
        _ => panic!("Mojo app-server broker returned unknown lifecycle schema"),
    }
}

pub(super) fn method_plan(
    method: Option<&str>,
    frame_kind: AppServerBrokerFrameKind,
) -> prodex_mojo_core::rich::AppServerBrokerMethodPlan {
    app_server_broker_normalize_method(method, frame_kind_code(frame_kind))
        .unwrap_or_else(|error| panic!("Mojo app-server broker method planning failed: {error:?}"))
}

pub(super) fn response_schema(request_stage: &str) -> Option<&'static str> {
    match app_server_broker_response_schema(request_stage).unwrap_or_else(|error| {
        panic!("Mojo app-server broker response planning failed: {error:?}")
    }) {
        0 => None,
        1 => Some("ThreadStartResponse.json"),
        2 => Some("ThreadResumeResponse.json"),
        3 => Some("ThreadForkResponse.json"),
        4 => Some("TurnStartResponse.json"),
        5 => Some("TurnInterruptResponse.json"),
        _ => panic!("Mojo app-server broker returned unknown response schema"),
    }
}

pub(super) fn affinity_plan(value: &Value) -> AppServerBrokerMojoAffinityPlan {
    let summary = super::app_server_broker_diagnostic_summary(value);
    let method_plan = method_plan(summary.method.as_deref(), summary.frame_kind);
    let plan = app_server_broker_plan_affinity(
        frame_kind_code(summary.frame_kind),
        method_plan.lifecycle_stage,
        summary.method.as_deref(),
        summary.metadata.session_id.is_some(),
        summary.metadata.thread_id.is_some(),
        summary.metadata.turn_id.is_some(),
    )
    .unwrap_or_else(|error| panic!("Mojo app-server broker affinity planning failed: {error:?}"));
    let keys = plan
        .key_kinds
        .into_iter()
        .take(plan.key_count)
        .map(affinity_kind)
        .collect();
    AppServerBrokerMojoAffinityPlan {
        keys,
        decision: continuation_decision(plan.decision),
        policy_hint: AppServerBrokerPolicyHint {
            mode: policy_mode(plan.mode),
            routing_hint: routing_hint(plan.routing_hint),
            commit_boundary: commit_boundary(plan.commit_boundary),
            rotation_window: rotation_window(plan.rotation_window),
        },
    }
}

pub(crate) fn validation_reason(input: AppServerBrokerValidationInput<'_>) -> Option<&'static str> {
    let reason = app_server_broker_lifecycle_validation_reason(input)
        .unwrap_or_else(|error| panic!("Mojo app-server broker validation failed: {error:?}"));
    match reason {
        None | Some(0) => None,
        Some(1) => Some("lifecycle_missing_thread_id"),
        Some(2) => Some("lifecycle_missing_thread_object_id"),
        Some(3) => Some("lifecycle_missing_thread_context"),
        Some(4) => Some("lifecycle_missing_thread_status"),
        Some(5) => Some("lifecycle_invalid_thread_status"),
        Some(6) => Some("lifecycle_missing_turn_input"),
        Some(7) => Some("lifecycle_missing_turn_items"),
        Some(8) => Some("lifecycle_invalid_turn_status"),
        Some(9) => Some("lifecycle_missing_turn_status"),
        Some(10) => Some("lifecycle_response_missing_thread_id"),
        Some(11) => Some("lifecycle_response_missing_thread_status"),
        Some(12) => Some("lifecycle_response_invalid_thread_status"),
        Some(13) => Some("lifecycle_response_missing_thread_context"),
        Some(14) => Some("lifecycle_response_invalid_thread_context"),
        Some(15) => Some("lifecycle_response_missing_thread_object_context"),
        Some(16) => Some("lifecycle_response_missing_turn_id"),
        Some(17) => Some("lifecycle_response_missing_turn_items"),
        Some(18) => Some("lifecycle_response_missing_turn_status"),
        Some(19) => Some("lifecycle_response_invalid_turn_status"),
        Some(_) => panic!("Mojo app-server broker returned unknown validation reason"),
    }
}

fn frame_kind_code(kind: AppServerBrokerFrameKind) -> i64 {
    match kind {
        AppServerBrokerFrameKind::Invalid => 0,
        AppServerBrokerFrameKind::Batch => 1,
        AppServerBrokerFrameKind::Request => 2,
        AppServerBrokerFrameKind::Notification => 3,
        AppServerBrokerFrameKind::Response => 4,
    }
}

fn affinity_kind(kind: i64) -> AppServerBrokerAffinityKeyKind {
    match kind {
        1 => AppServerBrokerAffinityKeyKind::Session,
        2 => AppServerBrokerAffinityKeyKind::Thread,
        3 => AppServerBrokerAffinityKeyKind::Turn,
        _ => panic!("Mojo app-server broker returned unknown affinity key"),
    }
}

fn continuation_decision(decision: i64) -> AppServerBrokerContinuationDecision {
    match decision {
        0 => AppServerBrokerContinuationDecision::Fresh,
        1 => AppServerBrokerContinuationDecision::ContinueSession,
        2 => AppServerBrokerContinuationDecision::ContinueThread,
        3 => AppServerBrokerContinuationDecision::ContinueTurn,
        _ => panic!("Mojo app-server broker returned unknown continuation decision"),
    }
}

fn policy_mode(mode: i64) -> AppServerBrokerPolicyHintMode {
    match mode {
        0 => AppServerBrokerPolicyHintMode::FreshSelectionOk,
        1 => AppServerBrokerPolicyHintMode::PreserveSessionAffinity,
        2 => AppServerBrokerPolicyHintMode::PreserveThreadAffinity,
        3 => AppServerBrokerPolicyHintMode::PreserveTurnAffinity,
        _ => panic!("Mojo app-server broker returned unknown policy mode"),
    }
}

fn routing_hint(hint: i64) -> AppServerBrokerRoutingHint {
    match hint {
        0 => AppServerBrokerRoutingHint::FreshSelectOk,
        1 => AppServerBrokerRoutingHint::PreserveSessionOwner,
        2 => AppServerBrokerRoutingHint::PreserveThreadOwner,
        3 => AppServerBrokerRoutingHint::PreserveTurnOwner,
        _ => panic!("Mojo app-server broker returned unknown routing hint"),
    }
}

fn commit_boundary(boundary: i64) -> AppServerBrokerCommitBoundary {
    match boundary {
        0 => AppServerBrokerCommitBoundary::Precommit,
        1 => AppServerBrokerCommitBoundary::TurnCommitted,
        _ => panic!("Mojo app-server broker returned unknown commit boundary"),
    }
}

fn rotation_window(window: i64) -> AppServerBrokerRotationWindow {
    match window {
        0 => AppServerBrokerRotationWindow::Open,
        1 => AppServerBrokerRotationWindow::Closed,
        _ => panic!("Mojo app-server broker returned unknown rotation window"),
    }
}
