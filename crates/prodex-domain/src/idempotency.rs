use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{CallId, ReservationId, TenantId};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, IdempotencyKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdempotencyKeyError::Empty);
        }
        if value.len() > 256 {
            return Err(IdempotencyKeyError::TooLong {
                length: value.len(),
            });
        }
        if let Some((index, character)) = value
            .char_indices()
            .find(|(_, character)| !character.is_ascii_graphic())
        {
            return Err(IdempotencyKeyError::InvalidCharacter { index, character });
        }
        Ok(Self(value))
    }

    pub fn from_call_reservation(call_id: CallId, reservation_id: ReservationId) -> Self {
        Self(format!("call:{call_id}:reservation:{reservation_id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IdempotencyKey")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyKeyError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Debug for IdempotencyKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::InvalidCharacter { .. } => f
                .debug_struct("InvalidCharacter")
                .field("index", &"<redacted>")
                .field("character", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::InvalidCharacter { .. } => {
                write!(f, "idempotency key is invalid")
            }
        }
    }
}

impl Error for IdempotencyKeyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyKeyErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyKeyErrorResponsePlan {
    pub status: IdempotencyKeyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_idempotency_key_error_response(
    _error: &IdempotencyKeyError,
) -> IdempotencyKeyErrorResponsePlan {
    IdempotencyKeyErrorResponsePlan {
        status: IdempotencyKeyErrorStatus::BadRequest,
        code: "idempotency_key_invalid",
        message: "idempotency key is invalid",
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotentOperation {
    pub tenant_id: TenantId,
    pub key: IdempotencyKey,
    pub request_fingerprint: String,
}

impl IdempotentOperation {
    pub fn new(
        tenant_id: TenantId,
        key: IdempotencyKey,
        request_fingerprint: impl Into<String>,
    ) -> Result<Self, IdempotentOperationError> {
        let request_fingerprint = request_fingerprint.into();
        if request_fingerprint.is_empty() {
            return Err(IdempotentOperationError::EmptyRequestFingerprint);
        }
        if let Some((index, character)) = request_fingerprint
            .char_indices()
            .find(|(_, character)| !character.is_ascii_graphic())
        {
            return Err(
                IdempotentOperationError::InvalidRequestFingerprintCharacter { index, character },
            );
        }
        Ok(Self {
            tenant_id,
            key,
            request_fingerprint,
        })
    }
}

impl fmt::Debug for IdempotentOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotentOperation")
            .field("tenant_id", &"<redacted>")
            .field("key", &self.key)
            .field("request_fingerprint", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotentOperationError {
    EmptyRequestFingerprint,
    InvalidRequestFingerprintCharacter { index: usize, character: char },
}

impl fmt::Debug for IdempotentOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequestFingerprint => f.write_str("EmptyRequestFingerprint"),
            Self::InvalidRequestFingerprintCharacter { .. } => f
                .debug_struct("InvalidRequestFingerprintCharacter")
                .field("index", &"<redacted>")
                .field("character", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotentOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequestFingerprint | Self::InvalidRequestFingerprintCharacter { .. } => {
                write!(f, "request fingerprint is invalid")
            }
        }
    }
}

impl Error for IdempotentOperationError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyRecord<R> {
    pub operation: IdempotentOperation,
    pub response: R,
}

impl<R> fmt::Debug for IdempotencyRecord<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecord")
            .field("operation", &self.operation)
            .field("response", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdempotencyEntry<R> {
    Pending {
        operation: IdempotentOperation,
        started_at_unix_ms: u64,
    },
    Completed(IdempotencyRecord<R>),
}

impl<R> IdempotencyEntry<R> {
    pub fn pending(operation: IdempotentOperation, started_at_unix_ms: u64) -> Self {
        Self::Pending {
            operation,
            started_at_unix_ms,
        }
    }

    pub fn completed(record: IdempotencyRecord<R>) -> Self {
        Self::Completed(record)
    }

    pub fn operation(&self) -> &IdempotentOperation {
        match self {
            Self::Pending { operation, .. } => operation,
            Self::Completed(record) => &record.operation,
        }
    }
}

impl<R> fmt::Debug for IdempotencyEntry<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending {
                operation,
                started_at_unix_ms: _,
            } => f
                .debug_struct("Pending")
                .field("operation", operation)
                .field("started_at_unix_ms", &"<redacted>")
                .finish(),
            Self::Completed(record) => f.debug_tuple("Completed").field(record).finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyDecision<R> {
    Execute,
    Replay(R),
}

impl<R> fmt::Debug for IdempotencyDecision<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execute => f.write_str("Execute"),
            Self::Replay(_) => f.debug_tuple("Replay").field(&"<redacted>").finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyReplayDecision<R> {
    ExecuteAndRecordPending,
    AlreadyInProgress { started_at_unix_ms: u64 },
    Replay(R),
}

impl<R> fmt::Debug for IdempotencyReplayDecision<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecuteAndRecordPending => f.write_str("ExecuteAndRecordPending"),
            Self::AlreadyInProgress {
                started_at_unix_ms: _,
            } => f
                .debug_struct("AlreadyInProgress")
                .field("started_at_unix_ms", &"<redacted>")
                .finish(),
            Self::Replay(_) => f.debug_tuple("Replay").field(&"<redacted>").finish(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdempotencyConflict {
    TenantMismatch,
    KeyMismatch,
    RequestFingerprintMismatch,
}

impl fmt::Display for IdempotencyConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch | Self::KeyMismatch => write!(f, "idempotency replay conflict"),
            Self::RequestFingerprintMismatch => {
                write!(f, "idempotency key was reused with a different request")
            }
        }
    }
}

impl Error for IdempotencyConflict {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyConflictStatus {
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyConflictResponsePlan {
    pub status: IdempotencyConflictStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_idempotency_conflict_response(
    error: &IdempotencyConflict,
) -> IdempotencyConflictResponsePlan {
    match error {
        IdempotencyConflict::TenantMismatch | IdempotencyConflict::KeyMismatch => {
            IdempotencyConflictResponsePlan {
                status: IdempotencyConflictStatus::Conflict,
                code: "idempotency_replay_conflict",
                message: "idempotency replay conflict",
            }
        }
        IdempotencyConflict::RequestFingerprintMismatch => IdempotencyConflictResponsePlan {
            status: IdempotencyConflictStatus::Conflict,
            code: "idempotency_key_reused",
            message: "idempotency key was reused for a different request",
        },
    }
}

pub fn decide_idempotency<R: Clone>(
    operation: &IdempotentOperation,
    existing: Option<&IdempotencyRecord<R>>,
) -> Result<IdempotencyDecision<R>, IdempotencyConflict> {
    let Some(existing) = existing else {
        return Ok(IdempotencyDecision::Execute);
    };
    validate_operation_replay(operation, &existing.operation)?;
    Ok(IdempotencyDecision::Replay(existing.response.clone()))
}

pub fn decide_idempotency_replay<R: Clone>(
    operation: &IdempotentOperation,
    existing: Option<&IdempotencyEntry<R>>,
) -> Result<IdempotencyReplayDecision<R>, IdempotencyConflict> {
    let Some(existing) = existing else {
        return Ok(IdempotencyReplayDecision::ExecuteAndRecordPending);
    };
    validate_operation_replay(operation, existing.operation())?;
    match existing {
        IdempotencyEntry::Pending {
            started_at_unix_ms, ..
        } => Ok(IdempotencyReplayDecision::AlreadyInProgress {
            started_at_unix_ms: *started_at_unix_ms,
        }),
        IdempotencyEntry::Completed(record) => {
            Ok(IdempotencyReplayDecision::Replay(record.response.clone()))
        }
    }
}

fn validate_operation_replay(
    operation: &IdempotentOperation,
    existing: &IdempotentOperation,
) -> Result<(), IdempotencyConflict> {
    if existing.tenant_id != operation.tenant_id {
        return Err(IdempotencyConflict::TenantMismatch);
    }
    if existing.key != operation.key {
        return Err(IdempotencyConflict::KeyMismatch);
    }
    if existing.request_fingerprint != operation.request_fingerprint {
        return Err(IdempotencyConflict::RequestFingerprintMismatch);
    }
    Ok(())
}
