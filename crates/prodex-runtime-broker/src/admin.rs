use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerAdminError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeBrokerAdminError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeBrokerActivationSuccess {
    pub ok: bool,
    pub current_profile: String,
}

pub fn runtime_broker_admin_not_enabled_error() -> RuntimeBrokerAdminError {
    RuntimeBrokerAdminError::new(
        404,
        "not_found",
        "runtime broker admin endpoint is not enabled for this proxy",
    )
}

pub fn runtime_broker_admin_forbidden_error() -> RuntimeBrokerAdminError {
    RuntimeBrokerAdminError::new(
        403,
        "forbidden",
        "missing or invalid runtime broker admin token",
    )
}

pub fn runtime_broker_validate_admin_token(
    provided_token: Option<&str>,
    expected_token: &str,
) -> Result<(), RuntimeBrokerAdminError> {
    if provided_token == Some(expected_token) {
        Ok(())
    } else {
        Err(runtime_broker_admin_forbidden_error())
    }
}

pub fn runtime_broker_validate_activation_method(
    method: &str,
) -> Result<(), RuntimeBrokerAdminError> {
    if method == "POST" {
        Ok(())
    } else {
        Err(RuntimeBrokerAdminError::new(
            405,
            "method_not_allowed",
            "runtime broker activation requires POST",
        ))
    }
}

pub fn runtime_broker_validate_activation_profile(
    current_profile: Option<&str>,
) -> Result<String, RuntimeBrokerAdminError> {
    current_profile
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            RuntimeBrokerAdminError::new(
                400,
                "invalid_request",
                "runtime broker activation requires a non-empty current_profile",
            )
        })
}

pub fn runtime_broker_activation_success(
    current_profile: impl Into<String>,
) -> RuntimeBrokerActivationSuccess {
    RuntimeBrokerActivationSuccess {
        ok: true,
        current_profile: current_profile.into(),
    }
}

pub fn runtime_broker_activation_profile_from_json(
    body: &[u8],
) -> Result<String, RuntimeBrokerAdminError> {
    let current_profile = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("current_profile")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    runtime_broker_validate_activation_profile(current_profile.as_deref())
}
