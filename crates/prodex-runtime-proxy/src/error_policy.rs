use std::time::Duration;
mod rate_limit_header;
pub use rate_limit_header::runtime_http_error_policy_with_headers;

const RUNTIME_JSON_SCAN_LIMIT: usize = 2_048;
const RUNTIME_RETRY_AFTER_CAP: Duration = Duration::from_secs(300);

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeHttpErrorSignal {
    ExplicitQuota,
    ExplicitRateLimit,
    ExplicitProfileUnavailable,
    ExplicitOverload,
    TransientStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeSignalMatchMode {
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
        name: "profile_unavailable",
        statuses: &[402, 403],
        signal: RuntimeHttpErrorSignal::ExplicitProfileUnavailable,
        class: RuntimeHttpErrorClass::ProfileUnavailable,
        precommit_action: RuntimeHttpErrorAction::RotateProfile,
    },
    RuntimeHttpErrorRule {
        name: "rate_limited",
        statuses: &[429],
        signal: RuntimeHttpErrorSignal::ExplicitRateLimit,
        class: RuntimeHttpErrorClass::RateLimited,
        precommit_action: RuntimeHttpErrorAction::RetryProfile,
    },
    RuntimeHttpErrorRule {
        name: "explicit_quota",
        statuses: &[402, 403, 429],
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

const RUNTIME_STREAM_ERROR_RULES: &[(RuntimeHttpErrorClass, RuntimeHttpErrorAction, &str)] = &[
    (
        RuntimeHttpErrorClass::ProfileUnavailable,
        RuntimeHttpErrorAction::RotateProfile,
        "profile_unavailable",
    ),
    (
        RuntimeHttpErrorClass::Overload,
        RuntimeHttpErrorAction::RetryProfile,
        "explicit_overload",
    ),
    (
        RuntimeHttpErrorClass::Quota,
        RuntimeHttpErrorAction::RotateProfile,
        "explicit_quota",
    ),
    (
        RuntimeHttpErrorClass::RateLimited,
        RuntimeHttpErrorAction::RetryProfile,
        "rate_limited",
    ),
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
        code: "quota_exhausted",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "quota_exceeded",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "resource_exhausted",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "rate_limit_exceeded",
        signal: RuntimeHttpErrorSignal::ExplicitRateLimit,
    },
    RuntimePayloadCodeRule {
        code: "rate_limit_exceeded_error",
        signal: RuntimeHttpErrorSignal::ExplicitRateLimit,
    },
    RuntimePayloadCodeRule {
        code: "usage_limit_reached",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "usage_not_included",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "workspace_member_credits_depleted",
        signal: RuntimeHttpErrorSignal::ExplicitQuota,
    },
    RuntimePayloadCodeRule {
        code: "deactivated_workspace",
        signal: RuntimeHttpErrorSignal::ExplicitProfileUnavailable,
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

        let message = match self.signal {
            RuntimeHttpErrorSignal::ExplicitQuota => runtime_error_signal_message_from_body(
                body,
                RuntimeHttpErrorSignal::ExplicitQuota,
                if status == 429 {
                    RuntimeSignalMatchMode::ExplicitCode
                } else {
                    RuntimeSignalMatchMode::UsageMessage
                },
            ),
            RuntimeHttpErrorSignal::ExplicitRateLimit => runtime_error_signal_message_from_body(
                body,
                RuntimeHttpErrorSignal::ExplicitRateLimit,
                RuntimeSignalMatchMode::ExplicitCode,
            ),
            RuntimeHttpErrorSignal::ExplicitProfileUnavailable => {
                runtime_error_signal_message_from_body(
                    body,
                    RuntimeHttpErrorSignal::ExplicitProfileUnavailable,
                    RuntimeSignalMatchMode::ExplicitCode,
                )
            }
            RuntimeHttpErrorSignal::ExplicitOverload => runtime_error_signal_message_from_body(
                body,
                RuntimeHttpErrorSignal::ExplicitOverload,
                RuntimeSignalMatchMode::ExplicitCode,
            ),
            RuntimeHttpErrorSignal::TransientStatus => {
                Some(runtime_transient_http_error_message(status, body))
            }
        };
        if message.is_some() {
            return message;
        }

        // A canonical Codex usage-limit message is authoritative even when a 429
        // response omitted its structured error code. Generic 429 bodies remain
        // pass-through to avoid turning ordinary throttling into quota exhaustion.
        (self.signal == RuntimeHttpErrorSignal::ExplicitQuota && status == 429)
            .then(|| {
                runtime_error_signal_message_from_body(
                    body,
                    RuntimeHttpErrorSignal::ExplicitQuota,
                    RuntimeSignalMatchMode::UsageMessage,
                )
                .filter(|message| runtime_authoritative_usage_limit_text_message(message))
            })
            .flatten()
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

// Rust-only transport-boundary parsing: this consumes raw provider bytes and returns a Rust
// duration before any deterministic Mojo planning boundary is reached.
pub fn runtime_retry_after_from_message(message: &str) -> Option<Duration> {
    let lower = message.to_ascii_lowercase();
    let start = lower.find("try again in")? + "try again in".len();
    runtime_retry_after_duration_token(&lower[start..])
}

pub fn runtime_retry_after_from_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Option<Duration> {
    headers
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .filter_map(|(_, value)| std::str::from_utf8(value).ok())
        .filter_map(runtime_retry_after_header_value)
        .max()
}

fn runtime_retry_after_header_value(value: &str) -> Option<Duration> {
    let seconds = value.trim().parse::<u64>().ok()?;
    (seconds > 0).then(|| Duration::from_secs(seconds).min(RUNTIME_RETRY_AFTER_CAP))
}

fn runtime_retry_after_duration_token(value: &str) -> Option<Duration> {
    let value = value.trim_start();
    let number_len = value
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    if number_len == 0 {
        return None;
    }
    let number = &value[..number_len];
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let whole = whole.parse::<u128>().ok()?;
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let suffix = value[number_len..].trim_start();
    let millis = if suffix.starts_with("ms") {
        whole.checked_add(u128::from(fraction.bytes().any(|byte| byte != b'0')))?
    } else if suffix.starts_with('s') || suffix.starts_with("second") {
        whole
            .checked_mul(1_000)?
            .checked_add(ceil_fraction_millis(fraction)?)?
    } else {
        return None;
    };
    if millis == 0 {
        return None;
    }
    Some(
        Duration::from_millis(u64::try_from(millis).unwrap_or(u64::MAX))
            .min(RUNTIME_RETRY_AFTER_CAP),
    )
}

fn ceil_fraction_millis(fraction: &str) -> Option<u128> {
    if fraction.is_empty() {
        return Some(0);
    }
    let digits = fraction.as_bytes();
    let mut millis = 0u128;
    for digit in digits.iter().copied().take(3) {
        millis = millis
            .checked_mul(10)?
            .checked_add(u128::from(digit - b'0'))?;
    }
    for _ in digits.len().min(3)..3 {
        millis = millis.checked_mul(10)?;
    }
    if digits.len() > 3 && digits[3..].iter().any(|digit| *digit != b'0') {
        millis = millis.checked_add(1)?;
    }
    Some(millis)
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

        return runtime_error_policy_match(
            rule.class,
            rule.precommit_action,
            rule.name,
            message,
            phase,
        );
    }

    RuntimeHttpErrorPolicy::pass_through()
}

/// Classifies an upstream error carried inside a streaming payload.
///
/// Unlike HTTP failures, streaming failures have no reliable transport status.
/// Only payload signals are considered, so a retry cannot be synthesized from
/// a transport status that the stream never supplied.
pub fn runtime_stream_error_policy(
    body: &[u8],
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return runtime_stream_error_policy_from_value(&value, phase);
    }

    RuntimeHttpErrorPolicy::pass_through()
}

pub fn runtime_stream_error_policy_from_value(
    value: &serde_json::Value,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    for &(class, action, rule) in RUNTIME_STREAM_ERROR_RULES {
        if let Some(message) = runtime_error_signal_message_from_value_mode(
            value,
            class,
            RuntimeSignalMatchMode::ExplicitCode,
        ) {
            return runtime_error_policy_match(class, action, rule, message, phase);
        }
    }

    RuntimeHttpErrorPolicy::pass_through()
}

pub fn runtime_http_error_class_label(class: RuntimeHttpErrorClass) -> &'static str {
    match class {
        RuntimeHttpErrorClass::Quota => "quota",
        RuntimeHttpErrorClass::RateLimited => "rate_limited",
        RuntimeHttpErrorClass::ProfileUnavailable => "profile_unavailable",
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
    runtime_error_signal_message_from_value_mode(
        value,
        signal,
        RuntimeSignalMatchMode::UsageMessage,
    )
}

fn runtime_error_signal_message_from_value_mode(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorClass,
    mode: RuntimeSignalMatchMode,
) -> Option<String> {
    match signal {
        RuntimeHttpErrorClass::Quota => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(candidate, RuntimeHttpErrorSignal::ExplicitQuota, mode)
        }),
        RuntimeHttpErrorClass::RateLimited => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitRateLimit,
                RuntimeSignalMatchMode::ExplicitCode,
            )
        }),
        RuntimeHttpErrorClass::ProfileUnavailable => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitProfileUnavailable,
                mode,
            )
        }),
        RuntimeHttpErrorClass::Overload => runtime_json_find(value, |candidate| {
            runtime_error_signal_candidate(
                candidate,
                RuntimeHttpErrorSignal::ExplicitOverload,
                mode,
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
        RuntimeHttpErrorClass::RateLimited => {
            runtime_text_has_payload_code(trimmed, RuntimeHttpErrorSignal::ExplicitRateLimit)
                .then(|| trimmed.to_string())
        }
        RuntimeHttpErrorClass::ProfileUnavailable => {
            runtime_profile_unavailable_text_message(trimmed).then(|| trimmed.to_string())
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

pub fn runtime_rate_limit_payload_code(code: &str) -> bool {
    runtime_payload_code_matches(code, RuntimeHttpErrorSignal::ExplicitRateLimit)
}

pub fn runtime_overload_payload_code(code: &str) -> bool {
    runtime_payload_code_matches(code, RuntimeHttpErrorSignal::ExplicitOverload)
}

pub fn runtime_usage_limit_text_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    runtime_text_has_payload_code(message, RuntimeHttpErrorSignal::ExplicitQuota)
        || runtime_workspace_credit_exhausted_text_message(message)
        || lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("the usage limit has been reached")
        || lower.contains("usage limit has been reached")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
}

pub fn runtime_authoritative_usage_limit_text_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("you hit your usage limit")
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
    match_mode: RuntimeSignalMatchMode,
) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return runtime_json_find(&value, |candidate| {
            runtime_error_signal_candidate(candidate, signal, match_mode)
        });
    }

    if let Some(message) = runtime_error_signal_message_from_sse_body(body, signal, match_mode) {
        return Some(message);
    }

    runtime_utf8_text(body).and_then(|text| match signal {
        RuntimeHttpErrorSignal::ExplicitQuota => match match_mode {
            // A generic 429 text body is not a structured provider signal,
            // even when it happens to contain a known code-shaped phrase.
            RuntimeSignalMatchMode::ExplicitCode => None,
            RuntimeSignalMatchMode::UsageMessage => {
                runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::Quota)
            }
        },
        RuntimeHttpErrorSignal::ExplicitRateLimit => {
            if match_mode == RuntimeSignalMatchMode::ExplicitCode {
                None
            } else {
                runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::RateLimited)
            }
        }
        RuntimeHttpErrorSignal::ExplicitProfileUnavailable => {
            runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::ProfileUnavailable)
        }
        RuntimeHttpErrorSignal::ExplicitOverload => {
            runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::Overload)
        }
        RuntimeHttpErrorSignal::TransientStatus => None,
    })
}

fn runtime_error_signal_message_from_sse_body(
    body: &[u8],
    signal: RuntimeHttpErrorSignal,
    match_mode: RuntimeSignalMatchMode,
) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    for line in text.lines() {
        let Some(payload) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let Some(value) = serde_json::from_str::<serde_json::Value>(payload.trim()).ok() else {
            continue;
        };
        if let Some(message) = runtime_json_find(&value, |candidate| {
            runtime_error_signal_candidate(candidate, signal, match_mode)
        }) {
            return Some(message);
        }
    }
    None
}

fn runtime_error_signal_candidate(
    value: &serde_json::Value,
    signal: RuntimeHttpErrorSignal,
    match_mode: RuntimeSignalMatchMode,
) -> Option<String> {
    match value {
        serde_json::Value::String(_) => None,
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let explicit_code = ["code", "type", "status", "reason"]
                .into_iter()
                .filter_map(|key| map.get(key).and_then(serde_json::Value::as_str))
                .any(|code| runtime_payload_code_matches(code, signal))
                || (signal == RuntimeHttpErrorSignal::ExplicitQuota
                    && match_mode == RuntimeSignalMatchMode::UsageMessage
                    && map
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|code| runtime_payload_code_matches(code, signal)));

            match signal {
                RuntimeHttpErrorSignal::ExplicitQuota if explicit_code => Some(
                    message
                        .unwrap_or("Upstream Codex account quota was exhausted.")
                        .to_string(),
                ),
                RuntimeHttpErrorSignal::ExplicitQuota
                    if match_mode == RuntimeSignalMatchMode::UsageMessage
                        && message.is_some_and(|message| {
                            runtime_workspace_credit_exhausted_text_message(message)
                                || runtime_usage_limit_text_message(message)
                        }) =>
                {
                    Some(
                        message
                            .unwrap_or("Upstream Codex account quota was exhausted.")
                            .to_string(),
                    )
                }
                RuntimeHttpErrorSignal::ExplicitRateLimit if explicit_code => Some(
                    message
                        .unwrap_or("Upstream Codex profile is temporarily rate limited.")
                        .to_string(),
                ),
                RuntimeHttpErrorSignal::ExplicitProfileUnavailable if explicit_code => Some(
                    message
                        .unwrap_or("Upstream Codex workspace is deactivated for this profile.")
                        .to_string(),
                ),
                RuntimeHttpErrorSignal::ExplicitOverload if explicit_code => Some(
                    message
                        .unwrap_or("Upstream Codex backend is currently overloaded.")
                        .to_string(),
                ),
                RuntimeHttpErrorSignal::ExplicitOverload
                    if match_mode == RuntimeSignalMatchMode::UsageMessage =>
                {
                    message
                        .filter(|message| runtime_overload_text_message(message))
                        .map(str::to_string)
                }
                RuntimeHttpErrorSignal::ExplicitOverload => None,
                RuntimeHttpErrorSignal::ExplicitQuota
                | RuntimeHttpErrorSignal::ExplicitRateLimit
                | RuntimeHttpErrorSignal::ExplicitProfileUnavailable
                | RuntimeHttpErrorSignal::TransientStatus => None,
            }
        }
        _ => None,
    }
}

fn runtime_payload_code_matches(code: &str, signal: RuntimeHttpErrorSignal) -> bool {
    RUNTIME_PAYLOAD_CODE_RULES
        .iter()
        .any(|rule| rule.signal == signal && rule.code.eq_ignore_ascii_case(code.trim()))
}

fn runtime_text_has_payload_code(text: &str, signal: RuntimeHttpErrorSignal) -> bool {
    let lower = text.to_ascii_lowercase();
    RUNTIME_PAYLOAD_CODE_RULES
        .iter()
        .any(|rule| rule.signal == signal && lower.contains(rule.code))
}

pub fn runtime_workspace_credit_exhausted_text_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("workspace_member_credits_depleted")
        || lower.contains("workspace is out of credits")
        || (lower.contains("out of credits")
            && lower.contains("workspace owner")
            && lower.contains("refill"))
}

fn runtime_profile_unavailable_text_message(message: &str) -> bool {
    runtime_text_has_payload_code(message, RuntimeHttpErrorSignal::ExplicitProfileUnavailable)
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
