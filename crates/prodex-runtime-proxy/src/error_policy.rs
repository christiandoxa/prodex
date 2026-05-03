const RUNTIME_JSON_SCAN_LIMIT: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHttpErrorPhase {
    PreCommit,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHttpErrorClass {
    Quota,
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
}

#[derive(Clone, Copy)]
enum RuntimeHttpErrorSignal {
    ExplicitQuota,
    ExplicitOverload,
    TransientStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeQuotaMatchMode {
    ExplicitCode,
    UsageMessage,
}

#[derive(Clone, Copy)]
struct RuntimeHttpErrorRule {
    name: &'static str,
    statuses: &'static [u16],
    signal: RuntimeHttpErrorSignal,
    class: RuntimeHttpErrorClass,
    precommit_action: RuntimeHttpErrorAction,
}

const RUNTIME_TRANSIENT_HTTP_STATUSES: &[u16] = &[500, 502, 503, 504, 529];

const RUNTIME_HTTP_ERROR_RULES: &[RuntimeHttpErrorRule] = &[
    RuntimeHttpErrorRule {
        name: "explicit_quota",
        statuses: &[403, 429],
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
        class: RuntimeHttpErrorClass::Quota,
        precommit_action: RuntimeHttpErrorAction::RotateProfile,
    },
    RuntimeHttpErrorRule {
        name: "explicit_overload",
        statuses: RUNTIME_TRANSIENT_HTTP_STATUSES,
        signal: RuntimeHttpErrorSignal::ExplicitOverload,
        class: RuntimeHttpErrorClass::Overload,
        precommit_action: RuntimeHttpErrorAction::RetryProfile,
    },
    RuntimeHttpErrorRule {
        name: "transient_5xx",
        statuses: RUNTIME_TRANSIENT_HTTP_STATUSES,
        signal: RuntimeHttpErrorSignal::TransientStatus,
        class: RuntimeHttpErrorClass::TransientServer,
        precommit_action: RuntimeHttpErrorAction::RetryProfile,
    },
];

#[derive(Clone, Copy)]
struct RuntimePayloadCodeRule {
    code: &'static str,
    signal: RuntimeHttpErrorSignal,
}

const RUNTIME_PAYLOAD_CODE_RULES: &[RuntimePayloadCodeRule] = &[
    RuntimePayloadCodeRule {
        code: "insufficient_quota",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "rate_limit_exceeded",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "usage_limit_reached",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "server_is_overloaded",
        signal: RuntimeHttpErrorSignal::ExplicitOverload,
    },
    RuntimePayloadCodeRule {
        code: "slow_down",
        signal: RuntimeHttpErrorSignal::ExplicitOverload,
    },
];

impl RuntimeHttpErrorRule {
    fn matches(self, status: u16, body: &[u8]) -> Option<String> {
        if !self.statuses.contains(&status) {
            return None;
        }

        match self.signal {
            RuntimeHttpErrorSignal::ExplicitQuota => runtime_error_signal_message_from_body(
                body,
                RuntimeHttpErrorSignal::ExplicitQuota,
                RuntimeQuotaMatchMode::ExplicitCode,
            ),
            RuntimeHttpErrorSignal::ExplicitOverload => runtime_error_signal_message_from_body(
                body,
                RuntimeHttpErrorSignal::ExplicitOverload,
                RuntimeQuotaMatchMode::ExplicitCode,
            ),
            RuntimeHttpErrorSignal::TransientStatus => {
                Some(runtime_transient_http_error_message(status, body))
            }
        }
    }
}

impl RuntimeHttpErrorAction {
    pub fn rotates_profile(self) -> bool {
        matches!(self, Self::RotateProfile)
    }

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
        }
    }

    pub fn may_retry_or_rotate(&self) -> bool {
        self.action.retries_profile()
    }
}

pub fn runtime_http_error_policy(
    status: u16,
    body: &[u8],
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    for rule in RUNTIME_HTTP_ERROR_RULES {
        let Some(message) = rule.matches(status, body) else {
            continue;
        };

        return RuntimeHttpErrorPolicy {
            class: rule.class,
            action: match phase {
                RuntimeHttpErrorPhase::PreCommit => rule.precommit_action,
                RuntimeHttpErrorPhase::Committed => RuntimeHttpErrorAction::PassThrough,
            },
            rule: Some(rule.name),
            message: Some(message),
        };
    }

    RuntimeHttpErrorPolicy::pass_through()
}

pub fn runtime_http_error_class_label(class: RuntimeHttpErrorClass) -> &'static str {
    match class {
        RuntimeHttpErrorClass::Quota => "quota",
        RuntimeHttpErrorClass::Overload => "overload",
        RuntimeHttpErrorClass::TransientServer => "transient_5xx",
        RuntimeHttpErrorClass::Other => "other",
    }
}

pub fn runtime_http_error_action_label(action: RuntimeHttpErrorAction) -> &'static str {
    match action {
        RuntimeHttpErrorAction::PassThrough => "pass_through",
        RuntimeHttpErrorAction::RotateProfile => "rotate_profile",
        RuntimeHttpErrorAction::RetryProfile => "retry_profile",
    }
}

pub fn runtime_error_signal_message_from_value(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorClass,
) -> Option<String> {
    match signal {
        RuntimeHttpErrorClass::Quota => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitQuota,
                RuntimeQuotaMatchMode::UsageMessage,
            )
        }),
        RuntimeHttpErrorClass::Overload => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitOverload,
                RuntimeQuotaMatchMode::ExplicitCode,
            )
        }),
        RuntimeHttpErrorClass::TransientServer | RuntimeHttpErrorClass::Other => None,
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

    match signal {
        RuntimeHttpErrorClass::Quota => {
            runtime_usage_limit_text_message(trimmed).then(|| trimmed.to_string())
        }
        RuntimeHttpErrorClass::Overload => {
            runtime_overload_text_message(trimmed).then(|| trimmed.to_string())
        }
        RuntimeHttpErrorClass::TransientServer | RuntimeHttpErrorClass::Other => None,
    }
}

pub fn runtime_quota_payload_code(code: &str) -> bool {
    runtime_payload_code_matches(code, RuntimeHttpErrorSignal::ExplicitQuota)
}

pub fn runtime_overload_payload_code(code: &str) -> bool {
    runtime_payload_code_matches(code, RuntimeHttpErrorSignal::ExplicitOverload)
}

pub fn runtime_usage_limit_text_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    runtime_text_has_payload_code(message, RuntimeHttpErrorSignal::ExplicitQuota)
        || lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("the usage limit has been reached")
        || lower.contains("usage limit has been reached")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
}

pub fn runtime_overload_text_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("selected model is at capacity")
        || (lower.contains("model is at capacity")
            && (lower.contains("try a different model") || lower.contains("please try again")))
        || lower.contains("backend under high demand")
        || lower.contains("experiencing high demand")
        || lower.contains("server is overloaded")
        || lower.contains("currently overloaded")
}

fn runtime_error_signal_message_from_body(
    body: &[u8],
    signal: RuntimeHttpErrorSignal,
    quota_mode: RuntimeQuotaMatchMode,
) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = runtime_json_find(&value, |candidate| {
            runtime_error_signal_candidate(candidate, signal, quota_mode)
        })
    {
        return Some(message);
    }

    runtime_utf8_text(body).and_then(|text| match signal {
        RuntimeHttpErrorSignal::ExplicitQuota => match quota_mode {
            RuntimeQuotaMatchMode::ExplicitCode => {
                runtime_text_has_payload_code(text, RuntimeHttpErrorSignal::ExplicitQuota)
                    .then(|| text.to_string())
            }
            RuntimeQuotaMatchMode::UsageMessage => {
                runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::Quota)
            }
        },
        RuntimeHttpErrorSignal::ExplicitOverload => {
            runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::Overload)
        }
        RuntimeHttpErrorSignal::TransientStatus => None,
    })
}

fn runtime_error_signal_candidate(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorSignal,
    quota_mode: RuntimeQuotaMatchMode,
) -> Option<String> {
    match value {
        serde_json::Value::String(message) => match signal {
            RuntimeHttpErrorSignal::ExplicitQuota => match quota_mode {
                RuntimeQuotaMatchMode::ExplicitCode => {
                    runtime_text_has_payload_code(message, RuntimeHttpErrorSignal::ExplicitQuota)
                        .then(|| message.to_string())
                }
                RuntimeQuotaMatchMode::UsageMessage => {
                    runtime_usage_limit_text_message(message).then(|| message.to_string())
                }
            },
            RuntimeHttpErrorSignal::ExplicitOverload => {
                runtime_error_signal_message_from_text(message, RuntimeHttpErrorClass::Overload)
            }
            RuntimeHttpErrorSignal::TransientStatus => None,
        },
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);
            let error_type = map.get("type").and_then(serde_json::Value::as_str);
            let explicit_code = code
                .into_iter()
                .chain(error_type)
                .any(|code| runtime_payload_code_matches(code, signal));

            match signal {
                RuntimeHttpErrorSignal::ExplicitQuota if explicit_code => Some(
                    message
                        .unwrap_or("Upstream Codex account quota was exhausted.")
                        .to_string(),
                ),
                RuntimeHttpErrorSignal::ExplicitQuota
                    if quota_mode == RuntimeQuotaMatchMode::UsageMessage
                        && message.is_some_and(runtime_usage_limit_text_message) =>
                {
                    Some(
                        message
                            .unwrap_or("Upstream Codex account quota was exhausted.")
                            .to_string(),
                    )
                }
                RuntimeHttpErrorSignal::ExplicitOverload if explicit_code => Some(
                    message
                        .unwrap_or("Upstream Codex backend is currently overloaded.")
                        .to_string(),
                ),
                RuntimeHttpErrorSignal::ExplicitOverload => message
                    .filter(|message| runtime_overload_text_message(message))
                    .map(str::to_string),
                RuntimeHttpErrorSignal::ExplicitQuota | RuntimeHttpErrorSignal::TransientStatus => {
                    None
                }
            }
        }
        _ => None,
    }
}

fn runtime_payload_code_matches(code: &str, signal: RuntimeHttpErrorSignal) -> bool {
    RUNTIME_PAYLOAD_CODE_RULES.iter().any(|rule| {
        matches!(
            (rule.signal, signal),
            (
                RuntimeHttpErrorSignal::ExplicitQuota,
                RuntimeHttpErrorSignal::ExplicitQuota
            ) | (
                RuntimeHttpErrorSignal::ExplicitOverload,
                RuntimeHttpErrorSignal::ExplicitOverload
            )
        ) && rule.code.eq_ignore_ascii_case(code.trim())
    })
}

fn runtime_text_has_payload_code(text: &str, signal: RuntimeHttpErrorSignal) -> bool {
    let lower = text.to_ascii_lowercase();
    RUNTIME_PAYLOAD_CODE_RULES.iter().any(|rule| {
        matches!(
            (rule.signal, signal),
            (
                RuntimeHttpErrorSignal::ExplicitQuota,
                RuntimeHttpErrorSignal::ExplicitQuota
            ) | (
                RuntimeHttpErrorSignal::ExplicitOverload,
                RuntimeHttpErrorSignal::ExplicitOverload
            )
        ) && lower.contains(rule.code)
    })
}

fn runtime_transient_http_error_message(status: u16, body: &[u8]) -> String {
    runtime_utf8_text(body)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| match status {
            500 => "Upstream Codex backend is currently experiencing high demand.".to_string(),
            status => format!("Upstream Codex backend returned transient HTTP {status}."),
        })
}

fn runtime_utf8_text(body: &[u8]) -> Option<&str> {
    std::str::from_utf8(body).ok().map(str::trim)
}

fn runtime_json_find<T, F>(root: &serde_json::Value, mut candidate: F) -> Option<T>
where
    F: FnMut(&serde_json::Value) -> Option<T>,
{
    let mut stack = vec![root];
    let mut visited = 0usize;

    while let Some(value) = stack.pop() {
        if let Some(result) = candidate(value) {
            return Some(result);
        }

        visited += 1;
        if visited >= RUNTIME_JSON_SCAN_LIMIT {
            break;
        }

        match value {
            serde_json::Value::Array(values) => stack.extend(values.iter().rev()),
            serde_json::Value::Object(map) => stack.extend(map.values().rev()),
            _ => {}
        }
    }

    None
}

#[cfg(test)]
#[path = "../tests/src/error_policy.rs"]
mod tests;
