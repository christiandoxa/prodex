use prodex_provider_core::ProviderErrorClass;
use serde_json::Value;

const NATIVE_FIRST_EVENT_MAX_BYTES: usize = 65_536;

pub(super) fn runtime_local_rewrite_native_first_event_error_class(
    event: &[u8],
) -> anyhow::Result<Option<ProviderErrorClass>> {
    if event.len() > NATIVE_FIRST_EVENT_MAX_BYTES {
        return Ok(None);
    }
    let payload = event
        .split(|byte| *byte == b'\n')
        .filter_map(|line| line.strip_prefix(b"data:"))
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(value) = serde_json::from_str::<Value>(&payload) else {
        return Ok(None);
    };
    Ok(
        match prodex_mojo_core::rich::classify_native_event_error(
            value.get("type").and_then(Value::as_str),
            value.get("error").is_some(),
            value.pointer("/error/type").and_then(Value::as_str),
            value.pointer("/error/code").and_then(Value::as_str),
            value.get("code").and_then(Value::as_str),
        )
        .map_err(|error| {
            anyhow::anyhow!("Mojo native-event provider error policy failed: {error:?}")
        })? {
            Some(0) => Some(ProviderErrorClass::Auth),
            Some(1) => Some(ProviderErrorClass::Quota),
            Some(2) => Some(ProviderErrorClass::RateLimit),
            Some(3) => Some(ProviderErrorClass::Transient),
            Some(4) => Some(ProviderErrorClass::NotFound),
            None => None,
            Some(_) => {
                anyhow::bail!("Mojo native-event provider error policy returned invalid class")
            }
        },
    )
}
