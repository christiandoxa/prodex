use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AuditEventId, CallId, RequestId, TenantId};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl fmt::Debug for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TraceId").field(&"<redacted>").finish()
    }
}

impl TraceId {
    pub fn new(value: impl Into<String>) -> Result<Self, TraceIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TraceIdError::Empty);
        }
        if value.len() > 128 {
            return Err(TraceIdError::TooLong {
                length: value.len(),
            });
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() || ch == '-' || ch == '_')
        {
            return Err(TraceIdError::InvalidCharacter);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn new_w3c(value: impl Into<String>) -> Result<Self, TraceIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TraceIdError::Empty);
        }
        if value.len() != 32
            || !value.chars().all(|ch| ch.is_ascii_hexdigit())
            || value.chars().all(|ch| ch == '0')
        {
            return Err(TraceIdError::InvalidCharacter);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum TraceIdError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter,
}

impl fmt::Debug for TraceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for TraceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::InvalidCharacter => {
                write!(f, "trace id is invalid")
            }
        }
    }
}

impl Error for TraceIdError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceIdErrorStatus {
    InvalidRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceIdErrorResponsePlan {
    pub status: TraceIdErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_trace_id_error_response(error: &TraceIdError) -> TraceIdErrorResponsePlan {
    let code = match error {
        TraceIdError::Empty => "trace_id_required",
        TraceIdError::TooLong { .. } => "trace_id_invalid",
        TraceIdError::InvalidCharacter => "trace_id_invalid",
    };

    TraceIdErrorResponsePlan {
        status: TraceIdErrorStatus::InvalidRequest,
        code,
        message: "trace id is invalid",
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrelationContext {
    pub request_id: RequestId,
    pub call_id: Option<CallId>,
    pub trace_id: Option<TraceId>,
    pub tenant_id: Option<TenantId>,
    pub audit_event_id: Option<AuditEventId>,
}

impl fmt::Debug for CorrelationContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorrelationContext")
            .field("request_id", &"<redacted>")
            .field("call_id", &self.call_id.as_ref().map(|_| "<redacted>"))
            .field("trace_id", &self.trace_id.as_ref().map(|_| "<redacted>"))
            .field("tenant_id", &self.tenant_id.as_ref().map(|_| "<redacted>"))
            .field(
                "audit_event_id",
                &self.audit_event_id.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl CorrelationContext {
    pub fn new(request_id: RequestId) -> Self {
        Self {
            request_id,
            call_id: None,
            trace_id: None,
            tenant_id: None,
            audit_event_id: None,
        }
    }

    pub fn with_call_id(mut self, call_id: CallId) -> Self {
        self.call_id = Some(call_id);
        self
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn with_tenant_id(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn with_audit_event_id(mut self, audit_event_id: AuditEventId) -> Self {
        self.audit_event_id = Some(audit_event_id);
        self
    }
}
