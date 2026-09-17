use std::error::Error;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct TelemetryAttribute {
    key: String,
    value: String,
}

impl fmt::Debug for TelemetryAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelemetryAttribute")
            .field("key", &self.key)
            .field("value", &"<redacted>")
            .finish()
    }
}

impl TelemetryAttribute {
    pub fn metric_label(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    pub fn as_metric_label(&self) -> Result<(&str, &str), TelemetryAttributeError> {
        if invalid_label_key(&self.key) {
            return Err(TelemetryAttributeError::InvalidKey);
        }
        if invalid_label_value(&self.value) {
            return Err(TelemetryAttributeError::InvalidValue);
        }
        Ok((&self.key, &self.value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryAttributeError {
    InvalidKey,
    InvalidValue,
}

impl fmt::Display for TelemetryAttributeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("telemetry metric label is invalid")
    }
}
impl Error for TelemetryAttributeError {}

fn invalid_label_key(key: &str) -> bool {
    if key.is_empty() || key.len() > 128 || key.chars().any(|c| !c.is_ascii_graphic()) {
        return true;
    }
    let normalized = key.to_ascii_lowercase().replace(['-', '.'], "_");
    [
        "tenant_id",
        "user_id",
        "principal_id",
        "request_id",
        "call_id",
        "virtual_key",
        "api_key",
        "prompt",
    ]
    .iter()
    .any(|blocked| normalized.contains(blocked))
}

fn invalid_label_value(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 || value.chars().any(|c| !c.is_ascii_graphic()) {
        return true;
    }
    let bytes = value.as_bytes();
    let uuid = bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|i| bytes[*i] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| [8, 13, 18, 23].contains(&i) || b.is_ascii_hexdigit());
    let hex_id = bytes.len() == 32 && bytes.iter().all(u8::is_ascii_hexdigit);
    uuid || hex_id
}
