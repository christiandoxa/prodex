#[cfg(test)]
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppServerBrokerMethod {
    Initialize,
    Initialized,
    ThreadStart,
    ThreadResume,
    ThreadFork,
    TurnStart,
    TurnCancel,
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
            "turn/cancel" => Self::TurnCancel,
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
            Self::TurnCancel => "turn/cancel",
            #[cfg(test)]
            Self::Other => "other",
        }
    }
}

pub(crate) fn app_server_broker_lifecycle_methods() -> [&'static str; 7] {
    [
        AppServerBrokerMethod::Initialize.label(),
        AppServerBrokerMethod::Initialized.label(),
        AppServerBrokerMethod::ThreadStart.label(),
        AppServerBrokerMethod::ThreadResume.label(),
        AppServerBrokerMethod::ThreadFork.label(),
        AppServerBrokerMethod::TurnStart.label(),
        AppServerBrokerMethod::TurnCancel.label(),
    ]
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppServerBrokerRequest {
    pub id: Option<Value>,
    pub method: AppServerBrokerMethod,
}

#[cfg(test)]
pub(crate) fn parse_app_server_broker_request(value: &Value) -> Option<AppServerBrokerRequest> {
    let object = value.as_object()?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return None;
    }
    let method = object.get("method").and_then(Value::as_str)?;
    Some(AppServerBrokerRequest {
        id: object.get("id").cloned(),
        method: AppServerBrokerMethod::parse(method),
    })
}

pub(crate) fn app_server_broker_contract_json() -> serde_json::Value {
    serde_json::json!({
        "object": "app_server_broker.contract",
        "enabled_by_default": false,
        "status": "skeleton",
        "transport": ["stdio-planned"],
        "jsonrpc": "2.0",
        "default_mode": "direct-passthrough",
        "lifecycle_methods": app_server_broker_lifecycle_methods(),
        "affinity": {
            "thread_session_owner_required": true,
            "continuation_affinity_wins": true,
            "rotate_only_before_turn_commit": true
        },
        "errors": {
            "quota": "json-rpc-error-planned",
            "rate_limit": "json-rpc-error-planned",
            "overload": "json-rpc-error-planned"
        }
    })
}
