use std::time::Duration;

mod rate_limit_header;
mod retry_after;
mod signal;
mod stream;

pub use rate_limit_header::runtime_http_error_policy_with_headers;
pub use retry_after::{runtime_retry_after_from_headers, runtime_retry_after_from_message};
pub use signal::runtime_error_signal_message_from_value;
pub use stream::{
    runtime_http_error_action_label, runtime_http_error_class_label, runtime_stream_error_policy,
    runtime_stream_error_policy_from_value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHttpErrorPhase {
    PreCommit,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHttpErrorClass {
    Quota,
    RateLimited,
    ProfileUnavailable,
    Overload,
    TransientServer,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHttpErrorAction {
    PassThrough,
    RotateProfile,
    RetryProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHttpErrorPolicy {
    pub class: RuntimeHttpErrorClass,
    pub action: RuntimeHttpErrorAction,
    pub rule: Option<&'static str>,
    pub message: Option<String>,
    pub retry_after: Option<Duration>,
}

impl RuntimeHttpErrorAction {
    pub fn retries_profile(self) -> bool {
        matches!(self, Self::RotateProfile | Self::RetryProfile)
    }
}

impl RuntimeHttpErrorPolicy {
    pub fn pass_through() -> Self {
        Self {
            class: RuntimeHttpErrorClass::Other,
            action: RuntimeHttpErrorAction::PassThrough,
            rule: None,
            message: None,
            retry_after: None,
        }
    }

    pub fn may_retry_or_rotate(&self) -> bool {
        self.action.retries_profile()
    }
}

fn runtime_error_policy_match(
    class: RuntimeHttpErrorClass,
    precommit_action: RuntimeHttpErrorAction,
    rule: &'static str,
    message: String,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    RuntimeHttpErrorPolicy {
        class,
        action: match phase {
            RuntimeHttpErrorPhase::PreCommit => precommit_action,
            RuntimeHttpErrorPhase::Committed => RuntimeHttpErrorAction::PassThrough,
        },
        rule: Some(rule),
        retry_after: (class == RuntimeHttpErrorClass::RateLimited)
            .then(|| runtime_retry_after_from_message(&message))
            .flatten(),
        message: Some(message),
    }
}

pub fn runtime_http_error_policy(
    status: u16,
    body: &[u8],
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    runtime_error_policy_from_mojo(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_HTTP,
        status,
        phase,
        body,
    )
}

fn runtime_error_policy_from_mojo(
    operation: i64,
    status: u16,
    phase: RuntimeHttpErrorPhase,
    body: &[u8],
) -> RuntimeHttpErrorPolicy {
    let phase = match phase {
        RuntimeHttpErrorPhase::PreCommit => 0,
        RuntimeHttpErrorPhase::Committed => 1,
    };
    let Ok((class, action, message)) =
        prodex_mojo_core::MojoError::rich_runtime_error_policy(operation, status, phase, body)
    else {
        return RuntimeHttpErrorPolicy::pass_through();
    };
    let (class, rule) = match class {
        1 => (RuntimeHttpErrorClass::Quota, "explicit_quota"),
        2 => (RuntimeHttpErrorClass::RateLimited, "rate_limited"),
        3 => (
            RuntimeHttpErrorClass::ProfileUnavailable,
            "profile_unavailable",
        ),
        4 => (RuntimeHttpErrorClass::Overload, "explicit_overload"),
        5 => (RuntimeHttpErrorClass::TransientServer, "transient_5xx"),
        _ => return RuntimeHttpErrorPolicy::pass_through(),
    };
    let action = match action {
        0 => RuntimeHttpErrorAction::PassThrough,
        1 => RuntimeHttpErrorAction::RotateProfile,
        2 => RuntimeHttpErrorAction::RetryProfile,
        _ => return RuntimeHttpErrorPolicy::pass_through(),
    };
    RuntimeHttpErrorPolicy {
        class,
        action,
        rule: Some(rule),
        retry_after: (class == RuntimeHttpErrorClass::RateLimited)
            .then(|| runtime_retry_after_from_message(&message))
            .flatten(),
        message: Some(message),
    }
}

pub fn runtime_error_signal_message_from_text(
    text: &str,
    signal: RuntimeHttpErrorClass,
) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (mode, expected_class) = match signal {
        RuntimeHttpErrorClass::Quota => (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_QUOTA, 1),
        RuntimeHttpErrorClass::RateLimited => {
            (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_RATE, 2)
        }
        RuntimeHttpErrorClass::ProfileUnavailable => {
            (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_PROFILE, 3)
        }
        RuntimeHttpErrorClass::Overload => {
            (prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_OVERLOAD, 4)
        }
        RuntimeHttpErrorClass::TransientServer | RuntimeHttpErrorClass::Other => return None,
    };
    prodex_mojo_core::MojoError::rich_runtime_error_policy(mode, 0, 0, trimmed.as_bytes())
        .ok()
        .filter(|(class, _, _)| *class == expected_class)
        .map(|_| trimmed.to_string())
}

pub fn runtime_quota_payload_code(code: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_CODE_QUOTA,
        0,
        0,
        code.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 1)
}

pub fn runtime_rate_limit_payload_code(code: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_CODE_RATE,
        0,
        0,
        code.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 2)
}

pub fn runtime_overload_payload_code(code: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_CODE_OVERLOAD,
        0,
        0,
        code.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 4)
}

pub fn runtime_usage_limit_text_message(message: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_QUOTA,
        0,
        0,
        message.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 1)
}

pub fn runtime_authoritative_usage_limit_text_message(message: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA,
        0,
        0,
        message.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 1)
}

pub fn runtime_overload_text_message(message: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_OVERLOAD,
        0,
        0,
        message.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 4)
}

pub fn runtime_workspace_credit_exhausted_text_message(message: &str) -> bool {
    prodex_mojo_core::MojoError::rich_runtime_error_policy(
        prodex_mojo_core::rich::RUNTIME_ERROR_MODE_TEXT_WORKSPACE,
        0,
        0,
        message.as_bytes(),
    )
    .is_ok_and(|(class, _, _)| class == 1)
}

#[cfg(test)]
#[path = "../tests/src/error_policy.rs"]
mod tests;
