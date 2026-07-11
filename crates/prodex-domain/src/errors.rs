use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AuditEventId, RequestId, TenantId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Authentication,
    Authorization,
    TenantIsolation,
    Budget,
    RateLimit,
    Idempotency,
    Policy,
    Validation,
    Conflict,
    Unavailable,
    Internal,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorCode(String);

impl fmt::Debug for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ErrorCode").field(&"<redacted>").finish()
    }
}

impl ErrorCode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn try_new(value: impl Into<String>) -> Result<Self, ErrorCodeError> {
        let code = Self(value.into());
        code.validate()?;
        Ok(code)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn validate(&self) -> Result<(), ErrorCodeError> {
        validate_error_code(&self.0)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ErrorCodeError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for ErrorCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::EmptySegment => f.write_str("EmptySegment"),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for ErrorCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::EmptySegment | Self::InvalidCharacter => {
                write!(f, "error code is invalid")
            }
        }
    }
}

impl Error for ErrorCodeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCodeErrorStatus {
    InvalidRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorCodeErrorResponsePlan {
    pub status: ErrorCodeErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_error_code_error_response(error: &ErrorCodeError) -> ErrorCodeErrorResponsePlan {
    let code = match error {
        ErrorCodeError::Empty => "error_code_required",
        ErrorCodeError::TooLong { .. }
        | ErrorCodeError::EmptySegment
        | ErrorCodeError::InvalidCharacter => "error_code_invalid",
    };

    ErrorCodeErrorResponsePlan {
        status: ErrorCodeErrorStatus::InvalidRequest,
        code,
        message: "error code is invalid",
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorMetadata {
    pub request_id: Option<RequestId>,
    pub tenant_id: Option<TenantId>,
    pub audit_event_id: Option<AuditEventId>,
    pub retryable: bool,
}

impl fmt::Debug for ErrorMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorMetadata")
            .field(
                "request_id",
                &self.request_id.as_ref().map(|_| "<redacted>"),
            )
            .field("tenant_id", &self.tenant_id.as_ref().map(|_| "<redacted>"))
            .field(
                "audit_event_id",
                &self.audit_event_id.as_ref().map(|_| "<redacted>"),
            )
            .field("retryable", &self.retryable)
            .finish()
    }
}

impl ErrorMetadata {
    pub fn new(
        request_id: Option<RequestId>,
        tenant_id: Option<TenantId>,
        audit_event_id: Option<AuditEventId>,
        retryable: bool,
    ) -> Self {
        Self {
            request_id,
            tenant_id,
            audit_event_id,
            retryable,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub version: u16,
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub message: String,
    pub metadata: ErrorMetadata,
}

impl fmt::Debug for ErrorEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorEnvelope")
            .field("version", &self.version)
            .field("code", &self.code)
            .field("category", &self.category)
            .field("message", &"<redacted>")
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl ErrorEnvelope {
    pub const CURRENT_VERSION: u16 = 1;

    pub fn new(
        code: ErrorCode,
        category: ErrorCategory,
        message: impl Into<String>,
        metadata: ErrorMetadata,
    ) -> Self {
        let code = if code.validate().is_ok() {
            code
        } else {
            ErrorCode("internal.error".to_string())
        };
        Self {
            version: Self::CURRENT_VERSION,
            code,
            category,
            message: sanitize_error_message(message.into()),
            metadata,
        }
    }
}

fn sanitize_error_message(message: String) -> String {
    let lowered = message.to_ascii_lowercase();
    if message.trim().is_empty()
        || message.len() > 512
        || message
            .chars()
            .any(|ch| ch != ' ' && !ch.is_ascii_graphic())
        || contains_secret_like_text(&lowered)
    {
        "request failed".to_string()
    } else {
        message
    }
}

fn validate_error_code(value: &str) -> Result<(), ErrorCodeError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ErrorCodeError::Empty);
    }
    if trimmed.len() > 128 {
        return Err(ErrorCodeError::TooLong {
            length: trimmed.len(),
        });
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') || trimmed.contains("..") {
        return Err(ErrorCodeError::EmptySegment);
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(ErrorCodeError::InvalidCharacter);
    }
    Ok(())
}

fn contains_secret_like_text(lowered: &str) -> bool {
    lowered.contains("bearer ")
        || lowered.contains("authorization")
        || lowered.contains("api_key")
        || lowered.contains("api key")
        || lowered.contains("x-api-key")
        || lowered.contains("chatgpt-account-id")
        || lowered.contains("password")
        || lowered.contains("secret")
        || lowered.contains("token=")
}
