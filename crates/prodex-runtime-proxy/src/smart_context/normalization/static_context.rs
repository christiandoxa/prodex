use super::*;

pub(in crate::smart_context) fn smart_context_static_context_prompt_cache_payload(
    items: &[SmartContextStableStaticContextItem],
) -> String {
    let mut payload = String::from("prodex-smart-context-static-prompt-cache-v1\n");
    for item in items {
        payload.push_str("id-bytes:");
        payload.push_str(&item.id.len().to_string());
        payload.push('\n');
        payload.push_str(&item.id);
        payload.push('\n');
        payload.push_str("text-bytes:");
        payload.push_str(&item.byte_len.to_string());
        payload.push('\n');
        payload.push_str(&item.canonical_text);
        payload.push('\n');
    }
    payload
}

pub(in crate::smart_context) fn smart_context_static_context_noise_line(line: &str) -> bool {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::rich::smart_context_static_context_noise_line(line)
            .expect("Mojo Smart Context static-context classifier returned invalid output")
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_static_context_noise_line_rust(line)
}

#[cfg(not(feature = "mojo"))]
fn smart_context_static_context_noise_line_rust(line: &str) -> bool {
    let mut value = line.trim();
    if let Some(inner) = value
        .strip_prefix("<!--")
        .and_then(|value| value.strip_suffix("-->"))
    {
        value = inner.trim();
    }

    for prefix in ["//", "#", ";"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.trim_start();
            break;
        }
    }

    let Some((key, noise_value)) = value.split_once(':').or_else(|| value.split_once('=')) else {
        return false;
    };
    let key = smart_context_static_context_noise_key(key);
    if !smart_context_static_context_noise_key_is_volatile(&key) {
        return false;
    }

    matches!(
        key.as_str(),
        "run id" | "request id" | "trace id" | "session id"
    ) || smart_context_static_context_noise_value_looks_volatile(noise_value)
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_static_context_noise_key(key: &str) -> String {
    let lower = key.trim().to_ascii_lowercase();
    let mut normalized = String::new();
    let mut previous_space = false;
    for value in lower.chars() {
        let value = match value {
            '-' | '_' => ' ',
            value => value,
        };
        if value.is_whitespace() {
            if !previous_space {
                normalized.push(' ');
                previous_space = true;
            }
        } else {
            normalized.push(value);
            previous_space = false;
        }
    }

    normalized
        .trim()
        .strip_prefix("prodex ")
        .unwrap_or(normalized.trim())
        .to_string()
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_static_context_noise_key_is_volatile(
    key: &str,
) -> bool {
    matches!(
        key,
        "generated"
            | "generated at"
            | "generated on"
            | "last generated"
            | "last generated at"
            | "timestamp"
            | "current date"
            | "current time"
            | "current datetime"
            | "as of"
            | "last updated"
            | "updated at"
            | "run id"
            | "request id"
            | "trace id"
            | "session id"
    )
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_static_context_noise_value_looks_volatile(
    value: &str,
) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(|value| value.is_ascii_digit()) {
        return true;
    }

    matches!(
        value.to_ascii_lowercase().as_str(),
        "now" | "today" | "yesterday" | "tomorrow"
    )
}
