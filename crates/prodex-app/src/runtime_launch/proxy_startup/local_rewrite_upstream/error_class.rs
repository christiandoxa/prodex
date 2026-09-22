use prodex_provider_core::ProviderErrorClass;
use serde_json::Value;

pub(super) fn runtime_local_rewrite_native_first_event_error_class(
    event: &[u8],
) -> Option<ProviderErrorClass> {
    let payload = event
        .split(|byte| *byte == b'\n')
        .filter_map(|line| line.strip_prefix(b"data:"))
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let value = serde_json::from_str::<Value>(&payload).ok()?;
    let is_error =
        value.get("type").and_then(Value::as_str) == Some("error") || value.get("error").is_some();
    if !is_error {
        return None;
    }
    let code = value
        .pointer("/error/type")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/error/code").and_then(Value::as_str))
        .or_else(|| value.get("code").and_then(Value::as_str))
        .map(str::to_ascii_lowercase);
    match code.as_deref() {
        Some(
            "rate_limit_error" | "rate_limit_exceeded" | "rate_limit_exceeded_error" | "slow_down",
        ) => Some(ProviderErrorClass::RateLimit),
        Some("overloaded_error" | "server_is_overloaded") => Some(ProviderErrorClass::Transient),
        Some("not_found_error" | "model_not_supported") => Some(ProviderErrorClass::NotFound),
        Some("authentication_error") | Some("invalid_api_key") => Some(ProviderErrorClass::Auth),
        Some(
            "insufficient_quota"
            | "credit_balance_exhausted"
            | "organization_spend_limit_exceeded"
            | "project_spend_limit_exceeded"
            | "quota_exhausted"
            | "quota_exceeded"
            | "resource_exhausted",
        ) => Some(ProviderErrorClass::Quota),
        _ => None,
    }
}
