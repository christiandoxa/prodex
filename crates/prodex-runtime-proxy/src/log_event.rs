use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyLogField<'a> {
    key: Cow<'a, str>,
    value: Cow<'a, str>,
}

impl<'a> RuntimeProxyLogField<'a> {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyLogEvent<'a> {
    event: Cow<'a, str>,
    fields: Vec<RuntimeProxyLogField<'a>>,
}

impl<'a> RuntimeProxyLogEvent<'a> {
    pub fn new(
        event: impl Into<Cow<'a, str>>,
        fields: impl IntoIterator<Item = RuntimeProxyLogField<'a>>,
    ) -> Self {
        Self {
            event: event.into(),
            fields: fields.into_iter().collect(),
        }
    }

    pub fn event(&self) -> &str {
        &self.event
    }

    pub fn fields(&self) -> &[RuntimeProxyLogField<'a>] {
        &self.fields
    }

    pub fn fields_map(&self) -> BTreeMap<String, String> {
        runtime_proxy_log_fields_to_map(&self.fields)
    }

    pub fn render_message(&self) -> String {
        let mut message = runtime_proxy_sanitize_log_fragment(&self.event).into_owned();
        for field in &self.fields {
            if field.key().is_empty() || runtime_proxy_log_key_needs_skip(field.key()) {
                continue;
            }
            if !message.is_empty() {
                message.push(' ');
            }
            message.push_str(field.key());
            message.push('=');
            message.push_str(&runtime_proxy_format_log_field_value(field.value()));
        }
        message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyParsedLogMessage {
    event: Option<String>,
    fields: Vec<RuntimeProxyLogField<'static>>,
}

impl RuntimeProxyParsedLogMessage {
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }

    pub fn fields(&self) -> &[RuntimeProxyLogField<'static>] {
        &self.fields
    }

    pub fn fields_map(&self) -> BTreeMap<String, String> {
        runtime_proxy_log_fields_to_map(&self.fields)
    }

    pub fn into_event(self) -> Option<RuntimeProxyLogEvent<'static>> {
        self.event.map(|event| RuntimeProxyLogEvent {
            event: Cow::Owned(event),
            fields: self.fields,
        })
    }
}

pub fn runtime_proxy_log_field<'a>(
    key: &'a str,
    value: impl Into<Cow<'a, str>>,
) -> RuntimeProxyLogField<'a> {
    RuntimeProxyLogField {
        key: Cow::Borrowed(key),
        value: value.into(),
    }
}

pub fn runtime_proxy_structured_log_message<'a>(
    event: &str,
    fields: impl IntoIterator<Item = RuntimeProxyLogField<'a>>,
) -> String {
    RuntimeProxyLogEvent::new(Cow::Owned(event.to_string()), fields).render_message()
}

pub fn runtime_proxy_parse_log_message(message: &str) -> RuntimeProxyParsedLogMessage {
    let mut parsed = RuntimeProxyParsedLogMessage {
        event: None,
        fields: Vec::new(),
    };
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        index = runtime_proxy_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let token_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'=' {
            let key = &message[token_start..index];
            index += 1;
            let value_start = index;
            index = runtime_proxy_skip_log_field_value(message, index);
            let raw_value = &message[value_start..index];
            if !key.is_empty() && !raw_value.is_empty() {
                parsed.fields.push(RuntimeProxyLogField {
                    key: Cow::Owned(key.to_string()),
                    value: Cow::Owned(runtime_proxy_parse_log_field_value(raw_value)),
                });
            }
            continue;
        }

        if token_start < index && parsed.event.is_none() {
            parsed.event = Some(message[token_start..index].to_string());
        }
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
    }

    parsed
}

pub fn runtime_proxy_parse_log_event(message: &str) -> Option<RuntimeProxyLogEvent<'static>> {
    runtime_proxy_parse_log_message(message).into_event()
}

pub fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    runtime_proxy_parse_log_message(message).fields_map()
}

pub fn runtime_proxy_log_event(message: &str) -> Option<&str> {
    runtime_proxy_log_event_span(message).map(|(start, end)| &message[start..end])
}

fn runtime_proxy_log_event_span(message: &str) -> Option<(usize, usize)> {
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        index = runtime_proxy_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }
        let token_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'=' {
            index = runtime_proxy_skip_log_field_value(message, index + 1);
            continue;
        }
        if token_start < index {
            return Some((token_start, index));
        }
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
    }
    None
}

fn runtime_proxy_log_fields_to_map(
    fields: &[RuntimeProxyLogField<'_>],
) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|field| (field.key().to_string(), field.value().to_string()))
        .collect()
}

fn runtime_proxy_log_key_needs_skip(key: &str) -> bool {
    key.bytes()
        .any(|byte| byte == b'=' || byte.is_ascii_whitespace())
}

fn runtime_proxy_format_log_field_value(value: &str) -> String {
    let sanitized = runtime_proxy_sanitize_log_fragment(value);
    if runtime_proxy_log_field_value_needs_quotes(&sanitized) {
        serde_json::to_string(sanitized.as_ref()).unwrap_or_else(|_| "\"\"".to_string())
    } else {
        sanitized.into_owned()
    }
}

fn runtime_proxy_sanitize_log_fragment(value: &str) -> Cow<'_, str> {
    if value.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        Cow::Owned(value.replace(['\r', '\n'], " "))
    } else {
        Cow::Borrowed(value)
    }
}

fn runtime_proxy_log_field_value_needs_quotes(value: &str) -> bool {
    value.is_empty()
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
        || value.contains('"')
        || value.contains('\\')
}

fn runtime_proxy_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if index >= bytes.len() {
        return index;
    }
    if bytes[index] == b'"' {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => {
                    index += 1;
                    break;
                }
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}
