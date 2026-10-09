use prodex_provider_core::{ProviderErrorClass, classify_provider_error};
use serde_json::Value;

const NATIVE_FIRST_EVENT_MAX_BYTES: usize = 65_536;

pub(super) fn runtime_local_rewrite_native_first_event_error_class(
    event: &[u8],
) -> Option<ProviderErrorClass> {
    if event.len() > NATIVE_FIRST_EVENT_MAX_BYTES {
        return None;
    }
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
        .or_else(|| value.get("code").and_then(Value::as_str));
    let class = classify_provider_error(None, code, None).class;
    (class != ProviderErrorClass::Other).then_some(class)
}
