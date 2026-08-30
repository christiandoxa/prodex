//! Lifecycle method and schema hints for app-server broker frames.

use super::AppServerBrokerFrameKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerMethod {
    Initialize,
    Initialized,
    ThreadStart,
    ThreadResume,
    ThreadFork,
    ThreadRevert,
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
            "thread/revert" => Self::ThreadRevert,
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
            Self::ThreadRevert => "thread/revert",
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
    ThreadQueueRequest,
    ThreadQueueChangedNotification,
    ThreadRevertRequest,
    ThreadRevertedNotification,
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
            Self::ThreadQueueRequest => "thread_queue_request",
            Self::ThreadQueueChangedNotification => "thread_queue_changed_notification",
            Self::ThreadRevertRequest => "thread_revert_request",
            Self::ThreadRevertedNotification => "thread_reverted_notification",
            Self::TurnStartRequest => "turn_start_request",
            Self::TurnStartedNotification => "turn_started_notification",
            Self::TurnCompletedNotification => "turn_completed_notification",
            Self::TurnInterruptRequest => "turn_interrupt_request",
        }
    }
}

pub(crate) fn app_server_broker_lifecycle_methods() -> [&'static str; 19] {
    [
        AppServerBrokerMethod::Initialize.label(),
        AppServerBrokerMethod::Initialized.label(),
        AppServerBrokerMethod::ThreadStart.label(),
        "thread/started",
        AppServerBrokerMethod::ThreadResume.label(),
        AppServerBrokerMethod::ThreadFork.label(),
        "thread/queue/add",
        "thread/queue/list",
        "thread/queue/update",
        "thread/queue/delete",
        "thread/queue/reorder",
        "thread/queue/start",
        "thread/queue/changed",
        AppServerBrokerMethod::ThreadRevert.label(),
        "thread/reverted",
        AppServerBrokerMethod::TurnStart.label(),
        "turn/started",
        "turn/completed",
        AppServerBrokerMethod::TurnInterrupt.label(),
    ]
}

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_is_lifecycle_method(method: &str) -> bool {
    super::mojo::method_plan(Some(method), AppServerBrokerFrameKind::Invalid).method_kind == 1
}

#[cfg(not(feature = "mojo-core"))]
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
    #[cfg(feature = "mojo-core")]
    {
        super::mojo::lifecycle_stage(super::mojo::method_plan(method, frame_kind).lifecycle_stage)
    }
    #[cfg(not(feature = "mojo-core"))]
    {
        let method = method?.trim();
        match frame_kind {
            AppServerBrokerFrameKind::Request => app_server_broker_request_stage(method),
            AppServerBrokerFrameKind::Notification => app_server_broker_notification_stage(method),
            AppServerBrokerFrameKind::Batch
            | AppServerBrokerFrameKind::Invalid
            | AppServerBrokerFrameKind::Response => None,
        }
    }
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_request_stage(method: &str) -> Option<AppServerBrokerLifecycleStage> {
    if method.eq_ignore_ascii_case("initialize") {
        Some(AppServerBrokerLifecycleStage::InitializeRequest)
    } else if method.eq_ignore_ascii_case("thread/start") {
        Some(AppServerBrokerLifecycleStage::ThreadStartRequest)
    } else if method.eq_ignore_ascii_case("thread/resume") {
        Some(AppServerBrokerLifecycleStage::ThreadResumeRequest)
    } else if method.eq_ignore_ascii_case("thread/fork") {
        Some(AppServerBrokerLifecycleStage::ThreadForkRequest)
    } else if matches!(
        method.to_ascii_lowercase().as_str(),
        "thread/queue/add"
            | "thread/queue/list"
            | "thread/queue/update"
            | "thread/queue/delete"
            | "thread/queue/reorder"
            | "thread/queue/start"
    ) {
        Some(AppServerBrokerLifecycleStage::ThreadQueueRequest)
    } else if method.eq_ignore_ascii_case("thread/revert") {
        Some(AppServerBrokerLifecycleStage::ThreadRevertRequest)
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

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_notification_stage(method: &str) -> Option<AppServerBrokerLifecycleStage> {
    if method.eq_ignore_ascii_case("notifications/initialized")
        || method.eq_ignore_ascii_case("initialized")
    {
        Some(AppServerBrokerLifecycleStage::InitializedNotification)
    } else if method.eq_ignore_ascii_case("thread/started") {
        Some(AppServerBrokerLifecycleStage::ThreadStartedNotification)
    } else if method.eq_ignore_ascii_case("thread/queue/changed") {
        Some(AppServerBrokerLifecycleStage::ThreadQueueChangedNotification)
    } else if method.eq_ignore_ascii_case("thread/reverted") {
        Some(AppServerBrokerLifecycleStage::ThreadRevertedNotification)
    } else if method.eq_ignore_ascii_case("turn/started") {
        Some(AppServerBrokerLifecycleStage::TurnStartedNotification)
    } else if method.eq_ignore_ascii_case("turn/completed") {
        Some(AppServerBrokerLifecycleStage::TurnCompletedNotification)
    } else {
        None
    }
}

pub(crate) fn app_server_broker_lifecycle_schema_file(
    method: Option<&str>,
    frame_kind: AppServerBrokerFrameKind,
) -> Option<&'static str> {
    #[cfg(feature = "mojo-core")]
    {
        super::mojo::lifecycle_schema(super::mojo::method_plan(method, frame_kind).lifecycle_schema)
    }
    #[cfg(not(feature = "mojo-core"))]
    {
        match app_server_broker_lifecycle_stage(method, frame_kind)? {
            AppServerBrokerLifecycleStage::ThreadStartRequest => Some("ThreadStartParams.json"),
            AppServerBrokerLifecycleStage::ThreadStartedNotification => {
                Some("ThreadStartedNotification.json")
            }
            AppServerBrokerLifecycleStage::ThreadResumeRequest => Some("ThreadResumeParams.json"),
            AppServerBrokerLifecycleStage::ThreadForkRequest => Some("ThreadForkParams.json"),
            AppServerBrokerLifecycleStage::ThreadQueueRequest
            | AppServerBrokerLifecycleStage::ThreadQueueChangedNotification
            | AppServerBrokerLifecycleStage::ThreadRevertRequest
            | AppServerBrokerLifecycleStage::ThreadRevertedNotification => None,
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
}

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_lifecycle_response_schema_file(
    request_stage: &str,
) -> Option<&'static str> {
    super::mojo::response_schema(request_stage)
}

#[cfg(not(feature = "mojo-core"))]
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
