use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AuditEventId, Principal, PrincipalId, TenantContext, TenantId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Denied,
    Failed,
}

impl AuditOutcome {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, AuditOutcomeError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(AuditOutcomeError::Empty);
        }
        match value {
            "success" => Ok(Self::Success),
            "denied" => Ok(Self::Denied),
            "failed" => Ok(Self::Failed),
            _ => Err(AuditOutcomeError::Unknown),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditOutcomeError {
    Empty,
    Unknown,
}

impl fmt::Debug for AuditOutcomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

impl fmt::Display for AuditOutcomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit outcome is invalid")
    }
}

impl Error for AuditOutcomeError {}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditAction(String);

impl fmt::Debug for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditAction").field(&"<redacted>").finish()
    }
}

impl AuditAction {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn try_new(value: impl Into<String>) -> Result<Self, AuditActionError> {
        let action = Self(value.into());
        action.validate()?;
        Ok(action)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn validate(&self) -> Result<(), AuditActionError> {
        validate_audit_action(&self.0)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditActionError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for AuditActionError {
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

impl fmt::Display for AuditActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit action is invalid")
    }
}

impl Error for AuditActionError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditResource {
    pub kind: String,
    pub id: Option<String>,
    pub tenant_id: Option<TenantId>,
}

impl fmt::Debug for AuditResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditResource")
            .field("kind", &self.kind)
            .field("id", &self.id.as_deref().map(|_| "<redacted>"))
            .field("tenant_id", &self.tenant_id.map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditResourceId(String);

impl AuditResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, AuditResourceIdError> {
        let value = value.into();
        validate_audit_resource_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AuditResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditResourceId")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditResourceIdError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter,
}

impl fmt::Debug for AuditResourceIdError {
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

impl fmt::Display for AuditResourceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit resource id is invalid")
    }
}

impl Error for AuditResourceIdError {}

impl AuditResource {
    pub fn new(
        kind: impl Into<String>,
        id: Option<impl Into<String>>,
        tenant_id: Option<TenantId>,
    ) -> Self {
        let kind = kind.into();
        Self {
            kind: if validate_audit_resource_kind(&kind).is_ok() {
                kind
            } else {
                "audit.invalid_resource".to_string()
            },
            id: id
                .map(Into::into)
                .filter(|id| validate_audit_resource_id(id).is_ok()),
            tenant_id,
        }
    }

    pub fn try_new(
        kind: impl Into<String>,
        id: Option<impl Into<String>>,
        tenant_id: Option<TenantId>,
    ) -> Result<Self, AuditResourceKindError> {
        let kind = kind.into();
        validate_audit_resource_kind(&kind)?;
        Ok(Self::new(kind, id, tenant_id))
    }

    pub fn new_with_resource_id(
        kind: impl Into<String>,
        id: Option<AuditResourceId>,
        tenant_id: Option<TenantId>,
    ) -> Result<Self, AuditResourceKindError> {
        let kind = kind.into();
        validate_audit_resource_kind(&kind)?;
        Ok(Self::new(kind, id.map(|id| id.0), tenant_id))
    }

    pub fn validate_kind(&self) -> Result<(), AuditResourceKindError> {
        validate_audit_resource_kind(&self.kind)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditResourceKindError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for AuditResourceKindError {
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

impl fmt::Display for AuditResourceKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit resource kind is invalid")
    }
}

impl Error for AuditResourceKindError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub occurred_at_unix_ms: u64,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub action: AuditAction,
    pub resource: AuditResource,
    pub outcome: AuditOutcome,
    pub reason_code: Option<String>,
}

impl fmt::Debug for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditEvent")
            .field("id", &"<redacted>")
            .field("occurred_at_unix_ms", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("action", &self.action)
            .field("resource", &self.resource)
            .field("outcome", &self.outcome)
            .field(
                "reason_code",
                &self.reason_code.as_deref().map(|_| "<redacted>"),
            )
            .finish()
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuditTimestamp {
    pub(super) unix_ms: u64,
}

impl fmt::Debug for AuditTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditTimestamp")
            .field("unix_ms", &"<redacted>")
            .finish()
    }
}

impl AuditTimestamp {
    pub fn new(unix_ms: u64) -> Result<Self, AuditTimestampError> {
        validate_audit_timestamp(unix_ms)?;
        Ok(Self { unix_ms })
    }

    pub fn unix_ms(self) -> u64 {
        self.unix_ms
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditTimestampError {
    Zero,
    BeforeUnixMilliseconds,
    TooFarFuture { unix_ms: u64 },
}

impl fmt::Debug for AuditTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("Zero"),
            Self::BeforeUnixMilliseconds => f.write_str("BeforeUnixMilliseconds"),
            Self::TooFarFuture { .. } => f
                .debug_struct("TooFarFuture")
                .field("unix_ms", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit timestamp is invalid")
    }
}

impl Error for AuditTimestampError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReasonCode(String);

impl fmt::Debug for AuditReasonCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditReasonCode")
            .field(&"<redacted>")
            .finish()
    }
}

impl AuditReasonCode {
    pub fn new(value: impl Into<String>) -> Result<Self, AuditReasonCodeError> {
        let value = value.into();
        validate_audit_reason_code(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditReasonCodeError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for AuditReasonCodeError {
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

impl fmt::Display for AuditReasonCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::EmptySegment | Self::InvalidCharacter => {
                write!(f, "audit reason code is invalid")
            }
        }
    }
}

impl Error for AuditReasonCodeError {}

impl AuditEvent {
    pub const MIN_UNIX_MS: u64 = 1_000;
    pub const MAX_UNIX_MS: u64 = 4_102_444_800_000;

    pub fn new(
        occurred_at_unix_ms: u64,
        tenant: TenantContext,
        principal: &Principal,
        action: AuditAction,
        resource: AuditResource,
        outcome: AuditOutcome,
        reason_code: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id: AuditEventId::new(),
            occurred_at_unix_ms,
            tenant_id: tenant.tenant_id,
            principal_id: principal.id,
            action: if action.validate().is_ok() {
                action
            } else {
                AuditAction("audit.invalid_action".to_string())
            },
            resource,
            outcome,
            reason_code: reason_code
                .map(Into::into)
                .filter(|reason_code| validate_audit_reason_code(reason_code).is_ok()),
        }
    }

    pub fn new_at(
        occurred_at: AuditTimestamp,
        tenant: TenantContext,
        principal: &Principal,
        action: AuditAction,
        resource: AuditResource,
        outcome: AuditOutcome,
        reason_code: Option<impl Into<String>>,
    ) -> Self {
        Self::new(
            occurred_at.unix_ms(),
            tenant,
            principal,
            action,
            resource,
            outcome,
            reason_code,
        )
    }

    pub fn new_with_reason_code(
        occurred_at_unix_ms: u64,
        tenant: TenantContext,
        principal: &Principal,
        action: AuditAction,
        resource: AuditResource,
        outcome: AuditOutcome,
        reason_code: Option<AuditReasonCode>,
    ) -> Self {
        Self::new(
            occurred_at_unix_ms,
            tenant,
            principal,
            action,
            resource,
            outcome,
            reason_code.map(|reason_code| reason_code.0),
        )
    }

    pub fn immutable_key(&self) -> AuditEventId {
        self.id
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditDigest(String);

impl AuditDigest {
    pub fn new(value: impl Into<String>) -> Result<Self, AuditDigestError> {
        let value = value.into();
        validate_audit_digest(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AuditDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditDigest").field(&"<redacted>").finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditDigestError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter,
}

impl fmt::Debug for AuditDigestError {
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

impl fmt::Display for AuditDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::InvalidCharacter => {
                write!(f, "audit digest is invalid")
            }
        }
    }
}

impl Error for AuditDigestError {}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEnvelope {
    pub event: AuditEvent,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for AuditEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditEnvelope")
            .field("event", &self.event)
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

impl AuditEnvelope {
    pub fn new(
        event: AuditEvent,
        previous_digest: Option<AuditDigest>,
        event_digest: AuditDigest,
    ) -> Self {
        Self {
            event,
            previous_digest,
            event_digest,
        }
    }

    pub fn verify_chain_link(
        &self,
        expected_previous_digest: Option<&AuditDigest>,
    ) -> Result<(), AuditChainError> {
        if self.previous_digest.as_ref() != expected_previous_digest {
            return Err(AuditChainError::PreviousDigestMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuditChainError {
    PreviousDigestMismatch,
}

impl fmt::Debug for AuditChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreviousDigestMismatch => f.write_str("PreviousDigestMismatch"),
        }
    }
}

fn validate_audit_action(value: &str) -> Result<(), AuditActionError> {
    if value.is_empty() {
        return Err(AuditActionError::Empty);
    }
    if value.len() > 128 {
        return Err(AuditActionError::TooLong {
            length: value.len(),
        });
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err(AuditActionError::EmptySegment);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(AuditActionError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_resource_kind(value: &str) -> Result<(), AuditResourceKindError> {
    if value.is_empty() {
        return Err(AuditResourceKindError::Empty);
    }
    if value.len() > 96 {
        return Err(AuditResourceKindError::TooLong {
            length: value.len(),
        });
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err(AuditResourceKindError::EmptySegment);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(AuditResourceKindError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_resource_id(value: &str) -> Result<(), AuditResourceIdError> {
    if value.is_empty() {
        return Err(AuditResourceIdError::Empty);
    }
    if value.len() > 256 {
        return Err(AuditResourceIdError::TooLong {
            length: value.len(),
        });
    }
    if !value.chars().all(|ch| {
        ch.is_ascii_lowercase()
            || ch.is_ascii_digit()
            || ch == '.'
            || ch == '_'
            || ch == '-'
            || ch == ':'
    }) {
        return Err(AuditResourceIdError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_reason_code(value: &str) -> Result<(), AuditReasonCodeError> {
    if value.is_empty() {
        return Err(AuditReasonCodeError::Empty);
    }
    if value.len() > 128 {
        return Err(AuditReasonCodeError::TooLong {
            length: value.len(),
        });
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err(AuditReasonCodeError::EmptySegment);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(AuditReasonCodeError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_digest(value: &str) -> Result<(), AuditDigestError> {
    if value.is_empty() {
        return Err(AuditDigestError::Empty);
    }
    if value.len() > 128 {
        return Err(AuditDigestError::TooLong {
            length: value.len(),
        });
    }
    if !value.chars().all(|ch| {
        ch.is_ascii_lowercase()
            || ch.is_ascii_digit()
            || ch == ':'
            || ch == '.'
            || ch == '_'
            || ch == '-'
    }) {
        return Err(AuditDigestError::InvalidCharacter);
    }
    Ok(())
}
fn validate_audit_timestamp(unix_ms: u64) -> Result<(), AuditTimestampError> {
    if unix_ms == 0 {
        return Err(AuditTimestampError::Zero);
    }
    if unix_ms < AuditEvent::MIN_UNIX_MS {
        return Err(AuditTimestampError::BeforeUnixMilliseconds);
    }
    if unix_ms > AuditEvent::MAX_UNIX_MS {
        return Err(AuditTimestampError::TooFarFuture { unix_ms });
    }
    Ok(())
}
