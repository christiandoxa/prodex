use redaction::redaction_redact_secret_like_text;
use serde_json::Value;

#[path = "app_server_broker_protocol/affinity.rs"]
mod affinity;
#[path = "app_server_broker_protocol/lifecycle.rs"]
mod lifecycle;
#[path = "app_server_broker_protocol/metadata.rs"]
mod metadata;
#[path = "app_server_broker_protocol/report.rs"]
mod report;
#[path = "app_server_broker_protocol/wire.rs"]
mod wire;

pub(crate) use affinity::{
    app_server_broker_affinity_keys, app_server_broker_allows_provider_switch,
    app_server_broker_continuation_affinity_summary_json, app_server_broker_continuation_decision,
    app_server_broker_lifecycle_binding, app_server_broker_policy_hint,
    app_server_broker_policy_hint_json,
};
pub(crate) use lifecycle::{
    AppServerBrokerLifecycleStage, AppServerBrokerMethod, app_server_broker_is_lifecycle_method,
    app_server_broker_lifecycle_methods, app_server_broker_lifecycle_response_schema_file,
    app_server_broker_lifecycle_schema_file, app_server_broker_lifecycle_stage,
};
pub(crate) use metadata::{AppServerBrokerMetadata, app_server_broker_metadata_from_value};
#[cfg(test)]
pub(crate) use report::app_server_broker_request_summary_json;
pub(crate) use report::{
    app_server_broker_diagnostic_summary_json, app_server_broker_response_summary_json,
};
use wire::app_server_broker_has_valid_wire_jsonrpc;
pub(crate) use wire::{app_server_broker_frame_kind, app_server_broker_invalid_reason};

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppServerBrokerRequest {
    pub id: Option<Value>,
    pub raw_method: String,
    pub method: AppServerBrokerMethod,
    pub metadata: AppServerBrokerMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppServerBrokerDiagnosticSummary {
    pub valid_jsonrpc: bool,
    pub frame_kind: AppServerBrokerFrameKind,
    pub id: Option<Value>,
    pub method: Option<String>,
    pub invalid_reason: Option<&'static str>,
    pub metadata: AppServerBrokerMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppServerBrokerLifecycleBinding {
    pub stage: AppServerBrokerLifecycleStage,
    pub metadata: AppServerBrokerMetadata,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerAffinityKeyKind {
    Session,
    Thread,
    Turn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppServerBrokerAffinityKey {
    pub kind: AppServerBrokerAffinityKeyKind,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerContinuationOwnerKind {
    None,
    Session,
    Thread,
    Turn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerContinuationDecision {
    Fresh,
    ContinueSession,
    ContinueThread,
    ContinueTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerPolicyHintMode {
    FreshSelectionOk,
    PreserveSessionAffinity,
    PreserveThreadAffinity,
    PreserveTurnAffinity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerCommitBoundary {
    Precommit,
    TurnCommitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerRotationWindow {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerRoutingHint {
    FreshSelectOk,
    PreserveSessionOwner,
    PreserveThreadOwner,
    PreserveTurnOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AppServerBrokerPolicyHint {
    pub mode: AppServerBrokerPolicyHintMode,
    pub routing_hint: AppServerBrokerRoutingHint,
    pub commit_boundary: AppServerBrokerCommitBoundary,
    pub rotation_window: AppServerBrokerRotationWindow,
}

impl AppServerBrokerPolicyHint {
    pub(crate) const fn turn_committed(self) -> bool {
        matches!(
            self.commit_boundary,
            AppServerBrokerCommitBoundary::TurnCommitted
        )
    }

    pub(crate) const fn affinity_required(self) -> bool {
        !matches!(self.mode, AppServerBrokerPolicyHintMode::FreshSelectionOk)
    }

    pub(crate) const fn rotation_allowed(self) -> bool {
        matches!(self.rotation_window, AppServerBrokerRotationWindow::Open)
    }

    pub(crate) const fn preserved_owner_kind(self) -> Option<AppServerBrokerContinuationOwnerKind> {
        match self.routing_hint {
            AppServerBrokerRoutingHint::FreshSelectOk => None,
            AppServerBrokerRoutingHint::PreserveSessionOwner => {
                Some(AppServerBrokerContinuationOwnerKind::Session)
            }
            AppServerBrokerRoutingHint::PreserveThreadOwner => {
                Some(AppServerBrokerContinuationOwnerKind::Thread)
            }
            AppServerBrokerRoutingHint::PreserveTurnOwner => {
                Some(AppServerBrokerContinuationOwnerKind::Turn)
            }
        }
    }

    pub(crate) const fn preserves_owner(self) -> bool {
        self.preserved_owner_kind().is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerFrameKind {
    Invalid,
    Request,
    Notification,
    Response,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerMethodKind {
    Absent,
    Lifecycle,
    Other,
}

impl AppServerBrokerFrameKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::Request => "request",
            Self::Notification => "notification",
            Self::Response => "response",
        }
    }
}

impl AppServerBrokerMethodKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Lifecycle => "lifecycle",
            Self::Other => "other",
        }
    }
}

impl AppServerBrokerAffinityKeyKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Thread => "thread",
            Self::Turn => "turn",
        }
    }
}

impl AppServerBrokerContinuationOwnerKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Session => "session",
            Self::Thread => "thread",
            Self::Turn => "turn",
        }
    }
}

impl AppServerBrokerContinuationDecision {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::ContinueSession => "continue-session",
            Self::ContinueThread => "continue-thread",
            Self::ContinueTurn => "continue-turn",
        }
    }
}

impl AppServerBrokerPolicyHintMode {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::FreshSelectionOk => "fresh-selection-ok",
            Self::PreserveSessionAffinity => "preserve-session-affinity",
            Self::PreserveThreadAffinity => "preserve-thread-affinity",
            Self::PreserveTurnAffinity => "preserve-turn-affinity",
        }
    }
}

impl AppServerBrokerCommitBoundary {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Precommit => "precommit",
            Self::TurnCommitted => "turn-committed",
        }
    }
}

impl AppServerBrokerRotationWindow {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

impl AppServerBrokerRoutingHint {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::FreshSelectOk => "fresh-select-ok",
            Self::PreserveSessionOwner => "preserve-session-owner",
            Self::PreserveThreadOwner => "preserve-thread-owner",
            Self::PreserveTurnOwner => "preserve-turn-owner",
        }
    }
}

pub(crate) fn app_server_broker_continuation_decision_kinds() -> [&'static str; 4] {
    [
        AppServerBrokerContinuationDecision::Fresh.label(),
        AppServerBrokerContinuationDecision::ContinueSession.label(),
        AppServerBrokerContinuationDecision::ContinueThread.label(),
        AppServerBrokerContinuationDecision::ContinueTurn.label(),
    ]
}

pub(crate) fn app_server_broker_policy_modes() -> [&'static str; 4] {
    [
        AppServerBrokerPolicyHintMode::FreshSelectionOk.label(),
        AppServerBrokerPolicyHintMode::PreserveSessionAffinity.label(),
        AppServerBrokerPolicyHintMode::PreserveThreadAffinity.label(),
        AppServerBrokerPolicyHintMode::PreserveTurnAffinity.label(),
    ]
}

pub(crate) fn app_server_broker_routing_hints() -> [&'static str; 4] {
    [
        AppServerBrokerRoutingHint::FreshSelectOk.label(),
        AppServerBrokerRoutingHint::PreserveSessionOwner.label(),
        AppServerBrokerRoutingHint::PreserveThreadOwner.label(),
        AppServerBrokerRoutingHint::PreserveTurnOwner.label(),
    ]
}

pub(crate) fn app_server_broker_commit_boundaries() -> [&'static str; 2] {
    [
        AppServerBrokerCommitBoundary::Precommit.label(),
        AppServerBrokerCommitBoundary::TurnCommitted.label(),
    ]
}

pub(crate) fn app_server_broker_rotation_windows() -> [&'static str; 2] {
    [
        AppServerBrokerRotationWindow::Open.label(),
        AppServerBrokerRotationWindow::Closed.label(),
    ]
}

#[cfg(test)]
pub(crate) fn parse_app_server_broker_request(value: &Value) -> Option<AppServerBrokerRequest> {
    let object = value.as_object()?;
    if !matches!(
        app_server_broker_frame_kind(value),
        AppServerBrokerFrameKind::Request | AppServerBrokerFrameKind::Notification
    ) {
        return None;
    }
    let method = object.get("method").and_then(Value::as_str)?;
    Some(AppServerBrokerRequest {
        id: object.get("id").cloned(),
        raw_method: method.to_string(),
        method: AppServerBrokerMethod::parse(method),
        metadata: app_server_broker_metadata_from_value(value),
    })
}

pub(crate) fn app_server_broker_diagnostic_summary(
    value: &Value,
) -> AppServerBrokerDiagnosticSummary {
    let object = value.as_object();
    let valid_jsonrpc = object.is_some_and(app_server_broker_has_valid_wire_jsonrpc);
    let method = object
        .and_then(|object| object.get("method"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let frame_kind = app_server_broker_frame_kind(value);
    AppServerBrokerDiagnosticSummary {
        valid_jsonrpc,
        frame_kind,
        id: object.and_then(|object| object.get("id")).cloned(),
        method,
        invalid_reason: app_server_broker_invalid_reason(value),
        metadata: app_server_broker_metadata_from_value(value),
    }
}

pub(crate) fn app_server_broker_method_kind(method: Option<&str>) -> AppServerBrokerMethodKind {
    match method {
        Some(method) if app_server_broker_is_lifecycle_method(method) => {
            AppServerBrokerMethodKind::Lifecycle
        }
        Some(_) => AppServerBrokerMethodKind::Other,
        None => AppServerBrokerMethodKind::Absent,
    }
}
