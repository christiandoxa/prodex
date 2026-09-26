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
        use prodex_mojo_core::observability::TelemetryMetricLabelValidation as Validation;

        match prodex_mojo_core::observability::validate_telemetry_metric_label(
            &self.key,
            &self.value,
        ) {
            Ok(Validation::Valid) => Ok((&self.key, &self.value)),
            Ok(Validation::InvalidKey) => Err(TelemetryAttributeError::InvalidKey),
            // Preserve the public error type and fail closed on boundary errors.
            Ok(Validation::InvalidValue) | Err(_) => Err(TelemetryAttributeError::InvalidValue),
        }
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
