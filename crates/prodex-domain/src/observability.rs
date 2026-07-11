use std::fmt;

use serde::{Deserialize, Serialize};

use crate::TenantId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewaySpanKind {
    Authentication,
    TenantResolution,
    Authorization,
    BudgetReservation,
    RoutingDecision,
    ProviderRequest,
    StreamingLifecycle,
    Persistence,
    Reconciliation,
    AuditEmission,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryAttributeScope {
    MetricLabel,
    TraceOnly,
    RedactedTraceOnly,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryAttribute {
    pub key: String,
    pub value: String,
    pub scope: TelemetryAttributeScope,
}

impl fmt::Debug for TelemetryAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelemetryAttribute")
            .field("key", &self.key)
            .field("value", &"<redacted>")
            .field("scope", &self.scope)
            .finish()
    }
}

impl TelemetryAttribute {
    pub fn metric_label(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            scope: TelemetryAttributeScope::MetricLabel,
        }
    }

    pub fn trace_only(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            scope: TelemetryAttributeScope::TraceOnly,
        }
    }

    pub fn redacted_trace_only(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: "<redacted>".to_string(),
            scope: TelemetryAttributeScope::RedactedTraceOnly,
        }
    }

    pub fn as_metric_label(&self) -> Result<(&str, &str), TelemetryAttributeError> {
        if self.scope != TelemetryAttributeScope::MetricLabel {
            return Err(TelemetryAttributeError::TraceOnlyAttribute);
        }
        if metric_label_key_is_forbidden(&self.key) {
            return Err(TelemetryAttributeError::ForbiddenMetricLabelKey);
        }
        if self.key.len() > 128 {
            return Err(TelemetryAttributeError::MetricLabelKeyTooLong {
                length: self.key.len(),
            });
        }
        if metric_label_value_is_forbidden(&self.value) {
            return Err(TelemetryAttributeError::ForbiddenMetricLabelValue);
        }
        if self.value.len() > 128 {
            return Err(TelemetryAttributeError::MetricLabelValueTooLong {
                length: self.value.len(),
            });
        }
        Ok((self.key.as_str(), self.value.as_str()))
    }
}

pub fn tenant_trace_attribute(tenant_id: TenantId) -> TelemetryAttribute {
    TelemetryAttribute::trace_only("tenant_id", tenant_id.to_string())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewaySpanDescriptor {
    pub kind: GatewaySpanKind,
    pub name: String,
    pub attributes: Vec<TelemetryAttribute>,
}

impl fmt::Debug for GatewaySpanDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewaySpanDescriptor")
            .field("kind", &self.kind)
            .field("name", &"<redacted>")
            .field("attributes", &"<redacted>")
            .finish()
    }
}

impl GatewaySpanDescriptor {
    pub fn new(kind: GatewaySpanKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            attributes: Vec::new(),
        }
    }

    pub fn with_attribute(mut self, attribute: TelemetryAttribute) -> Self {
        self.attributes.push(attribute);
        self
    }

    pub fn metric_labels(&self) -> Result<Vec<(&str, &str)>, TelemetryAttributeError> {
        self.attributes
            .iter()
            .filter(|attribute| attribute.scope == TelemetryAttributeScope::MetricLabel)
            .map(TelemetryAttribute::as_metric_label)
            .collect()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryAttributeError {
    TraceOnlyAttribute,
    ForbiddenMetricLabelKey,
    ForbiddenMetricLabelValue,
    MetricLabelKeyTooLong { length: usize },
    MetricLabelValueTooLong { length: usize },
}

impl fmt::Debug for TelemetryAttributeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TraceOnlyAttribute => f.write_str("TraceOnlyAttribute"),
            Self::ForbiddenMetricLabelKey => f.write_str("ForbiddenMetricLabelKey"),
            Self::ForbiddenMetricLabelValue => f.write_str("ForbiddenMetricLabelValue"),
            Self::MetricLabelKeyTooLong { .. } => f
                .debug_struct("MetricLabelKeyTooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::MetricLabelValueTooLong { .. } => f
                .debug_struct("MetricLabelValueTooLong")
                .field("length", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryAttributeErrorStatus {
    InvalidRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryAttributeErrorResponsePlan {
    pub status: TelemetryAttributeErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_telemetry_attribute_error_response(
    error: &TelemetryAttributeError,
) -> TelemetryAttributeErrorResponsePlan {
    let code = match error {
        TelemetryAttributeError::TraceOnlyAttribute => "telemetry_attribute_scope_invalid",
        TelemetryAttributeError::ForbiddenMetricLabelKey
        | TelemetryAttributeError::ForbiddenMetricLabelValue => "telemetry_metric_label_forbidden",
        TelemetryAttributeError::MetricLabelKeyTooLong { .. }
        | TelemetryAttributeError::MetricLabelValueTooLong { .. } => {
            "telemetry_metric_label_invalid"
        }
    };

    TelemetryAttributeErrorResponsePlan {
        status: TelemetryAttributeErrorStatus::InvalidRequest,
        code,
        message: "telemetry attribute is invalid",
    }
}

fn metric_label_key_is_forbidden(key: &str) -> bool {
    if key.is_empty() || key.chars().any(|character| !character.is_ascii_graphic()) {
        return true;
    }
    let normalized = key.to_ascii_lowercase().replace(['-', '.'], "_");
    let compact = normalized.replace('_', "");
    let exact_forbidden = [
        "tenant",
        "user",
        "principal",
        "request",
        "call",
        "prompt",
        "key",
        "token",
        "secret",
        "credential",
        "password",
    ];
    if exact_forbidden
        .iter()
        .any(|forbidden| normalized == *forbidden || compact == *forbidden)
    {
        return true;
    }
    let forbidden = [
        "tenant_id",
        "user_id",
        "principal_id",
        "virtual_key",
        "api_key",
        "request_id",
        "call_id",
        "prompt",
    ];
    forbidden.iter().any(|forbidden| {
        normalized.contains(forbidden) || compact.contains(&forbidden.replace('_', ""))
    })
}

fn metric_label_value_is_forbidden(value: &str) -> bool {
    if value.is_empty() || value.chars().any(|character| !character.is_ascii_graphic()) {
        return true;
    }
    let normalized = value.to_ascii_lowercase();
    let secret_prefix = ["bearer ", "sk-", "ghp_", "gho_", "github_pat_"]
        .iter()
        .any(|prefix| normalized.starts_with(prefix));
    let jwt_like = normalized.starts_with("eyj")
        && normalized.split('.').count() == 3
        && normalized.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        });
    let bytes = value.as_bytes();
    let canonical_uuid = bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit());
    let undashed_uuid_or_trace_id =
        bytes.len() == 32 && bytes.iter().all(|byte| byte.is_ascii_hexdigit());
    secret_prefix || jwt_like || canonical_uuid || undashed_uuid_or_trace_id
}
