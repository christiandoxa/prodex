//! Lifecycle method and schema hints for app-server broker frames.

use super::AppServerBrokerFrameKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerMethod {
    Initialize,
    Initialized,
    ThreadStart,
    ThreadResume,
    ThreadFork,
    TurnStart,
    TurnInterrupt,
    #[cfg(test)]
    Other,
}

impl AppServerBrokerMethod {
    #[cfg(test)]
    pub(crate) fn parse(method: &str) -> Self {
        match method {
            "initialize" => Self::Initialize,
            "initialized" | "notifications/initialized" => Self::Initialized,
            "thread/start" => Self::ThreadStart,
            "thread/resume" => Self::ThreadResume,
            "thread/fork" => Self::ThreadFork,
            "turn/start" => Self::TurnStart,
            "turn/interrupt" | "turn/cancel" => Self::TurnInterrupt,
            _ => Self::Other,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Initialized => "initialized",
            Self::ThreadStart => "thread/start",
            Self::ThreadResume => "thread/resume",
            Self::ThreadFork => "thread/fork",
            Self::TurnStart => "turn/start",
            Self::TurnInterrupt => "turn/interrupt",
            #[cfg(test)]
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerLifecycleStage {
    InitializeRequest,
    InitializedNotification,
    ThreadStartRequest,
    ThreadStartedNotification,
    ThreadResumeRequest,
    ThreadForkRequest,
    TurnStartRequest,
    TurnStartedNotification,
    TurnCompletedNotification,
    TurnInterruptRequest,
}

impl AppServerBrokerLifecycleStage {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::InitializeRequest => "initialize_request",
            Self::InitializedNotification => "initialized_notification",
            Self::ThreadStartRequest => "thread_start_request",
            Self::ThreadStartedNotification => "thread_started_notification",
            Self::ThreadResumeRequest => "thread_resume_request",
            Self::ThreadForkRequest => "thread_fork_request",
            Self::TurnStartRequest => "turn_start_request",
            Self::TurnStartedNotification => "turn_started_notification",
            Self::TurnCompletedNotification => "turn_completed_notification",
            Self::TurnInterruptRequest => "turn_interrupt_request",
        }
    }
}

pub(crate) fn app_server_broker_lifecycle_methods() -> [&'static str; 10] {
    [
        AppServerBrokerMethod::Initialize.label(),
        AppServerBrokerMethod::Initialized.label(),
        AppServerBrokerMethod::ThreadStart.label(),
        "thread/started",
        AppServerBrokerMethod::ThreadResume.label(),
        AppServerBrokerMethod::ThreadFork.label(),
        AppServerBrokerMethod::TurnStart.label(),
        "turn/started",
        "turn/completed",
        AppServerBrokerMethod::TurnInterrupt.label(),
    ]
}

pub(crate) fn app_server_broker_is_lifecycle_method(method: &str) -> bool {
    let method = method.trim();
    method.eq_ignore_ascii_case("notifications/initialized")
        || method.eq_ignore_ascii_case("turn/cancel")
        || app_server_broker_lifecycle_methods()
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(method))
}

pub(crate) fn app_server_broker_lifecycle_stage(
    method: Option<&str>,
    frame_kind: AppServerBrokerFrameKind,
) -> Option<AppServerBrokerLifecycleStage> {
    let method = method?.trim();
    match frame_kind {
        AppServerBrokerFrameKind::Request => {
            if method.eq_ignore_ascii_case("initialize") {
                Some(AppServerBrokerLifecycleStage::InitializeRequest)
            } else if method.eq_ignore_ascii_case("thread/start") {
                Some(AppServerBrokerLifecycleStage::ThreadStartRequest)
            } else if method.eq_ignore_ascii_case("thread/resume") {
                Some(AppServerBrokerLifecycleStage::ThreadResumeRequest)
            } else if method.eq_ignore_ascii_case("thread/fork") {
                Some(AppServerBrokerLifecycleStage::ThreadForkRequest)
            } else if method.eq_ignore_ascii_case("turn/start") {
                Some(AppServerBrokerLifecycleStage::TurnStartRequest)
            } else if method.eq_ignore_ascii_case("turn/interrupt")
                || method.eq_ignore_ascii_case("turn/cancel")
            {
                Some(AppServerBrokerLifecycleStage::TurnInterruptRequest)
            } else {
                None
            }
        }
        AppServerBrokerFrameKind::Notification => {
            if method.eq_ignore_ascii_case("notifications/initialized")
                || method.eq_ignore_ascii_case("initialized")
            {
                Some(AppServerBrokerLifecycleStage::InitializedNotification)
            } else if method.eq_ignore_ascii_case("thread/started") {
                Some(AppServerBrokerLifecycleStage::ThreadStartedNotification)
            } else if method.eq_ignore_ascii_case("turn/started") {
                Some(AppServerBrokerLifecycleStage::TurnStartedNotification)
            } else if method.eq_ignore_ascii_case("turn/completed") {
                Some(AppServerBrokerLifecycleStage::TurnCompletedNotification)
            } else {
                None
            }
        }
        AppServerBrokerFrameKind::Invalid | AppServerBrokerFrameKind::Response => None,
    }
}

pub(crate) fn app_server_broker_lifecycle_schema_file(
    method: Option<&str>,
    frame_kind: AppServerBrokerFrameKind,
) -> Option<&'static str> {
    match app_server_broker_lifecycle_stage(method, frame_kind)? {
        AppServerBrokerLifecycleStage::ThreadStartRequest => Some("ThreadStartParams.json"),
        AppServerBrokerLifecycleStage::ThreadStartedNotification => {
            Some("ThreadStartedNotification.json")
        }
        AppServerBrokerLifecycleStage::ThreadResumeRequest => Some("ThreadResumeParams.json"),
        AppServerBrokerLifecycleStage::ThreadForkRequest => Some("ThreadForkParams.json"),
        AppServerBrokerLifecycleStage::TurnStartRequest => Some("TurnStartParams.json"),
        AppServerBrokerLifecycleStage::TurnStartedNotification => {
            Some("TurnStartedNotification.json")
        }
        AppServerBrokerLifecycleStage::TurnCompletedNotification => {
            Some("TurnCompletedNotification.json")
        }
        AppServerBrokerLifecycleStage::TurnInterruptRequest => Some("TurnInterruptParams.json"),
        AppServerBrokerLifecycleStage::InitializeRequest
        | AppServerBrokerLifecycleStage::InitializedNotification => None,
    }
}

pub(crate) fn app_server_broker_lifecycle_response_schema_file(
    request_stage: &str,
) -> Option<&'static str> {
    match request_stage {
        "thread_start_request" => Some("ThreadStartResponse.json"),
        "thread_resume_request" => Some("ThreadResumeResponse.json"),
        "thread_fork_request" => Some("ThreadForkResponse.json"),
        "turn_start_request" => Some("TurnStartResponse.json"),
        "turn_interrupt_request" => Some("TurnInterruptResponse.json"),
        _ => None,
    }
}
