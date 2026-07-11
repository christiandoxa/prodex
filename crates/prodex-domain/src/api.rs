use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u16,
    pub minor: u16,
}

impl fmt::Debug for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ApiVersion").field(&"<redacted>").finish()
    }
}

impl ApiVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}", self.major, self.minor)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiVersionStatus {
    Current,
    Deprecated {
        deprecated_at_unix_ms: u64,
        sunset_at_unix_ms: Option<u64>,
    },
    Sunset {
        sunset_at_unix_ms: u64,
    },
}

impl fmt::Debug for ApiVersionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Current => f.write_str("Current"),
            Self::Deprecated { .. } => f
                .debug_struct("Deprecated")
                .field("deprecated_at_unix_ms", &"<redacted>")
                .field("sunset_at_unix_ms", &"<redacted>")
                .finish(),
            Self::Sunset { .. } => f
                .debug_struct("Sunset")
                .field("sunset_at_unix_ms", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVersionPolicy {
    pub version: ApiVersion,
    pub status: ApiVersionStatus,
}

impl fmt::Debug for ApiVersionPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiVersionPolicy")
            .field("version", &"<redacted>")
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiVersionDecision {
    Allowed,
    AllowedDeprecated {
        deprecated_at_unix_ms: u64,
        sunset_at_unix_ms: Option<u64>,
    },
}

impl fmt::Debug for ApiVersionDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allowed => f.write_str("Allowed"),
            Self::AllowedDeprecated { .. } => f
                .debug_struct("AllowedDeprecated")
                .field("deprecated_at_unix_ms", &"<redacted>")
                .field("sunset_at_unix_ms", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApiVersionError {
    Unsupported {
        requested: ApiVersion,
    },
    Sunset {
        requested: ApiVersion,
        sunset_at_unix_ms: u64,
    },
}

impl fmt::Debug for ApiVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { .. } => f
                .debug_struct("Unsupported")
                .field("requested", &"<redacted>")
                .finish(),
            Self::Sunset { .. } => f
                .debug_struct("Sunset")
                .field("requested", &"<redacted>")
                .field("sunset_at_unix_ms", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApiVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { .. } => write!(f, "API version is unsupported"),
            Self::Sunset { .. } => write!(f, "API version is no longer available"),
        }
    }
}

impl Error for ApiVersionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiVersionErrorStatus {
    NotFound,
    Gone,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVersionErrorResponsePlan {
    pub status: ApiVersionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for ApiVersionErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiVersionErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_api_version_error_response(error: &ApiVersionError) -> ApiVersionErrorResponsePlan {
    match error {
        ApiVersionError::Unsupported { .. } => ApiVersionErrorResponsePlan {
            status: ApiVersionErrorStatus::NotFound,
            code: "api_version_unsupported",
            message: "API version is unsupported",
        },
        ApiVersionError::Sunset { .. } => ApiVersionErrorResponsePlan {
            status: ApiVersionErrorStatus::Gone,
            code: "api_version_sunset",
            message: "API version is no longer available",
        },
    }
}

pub fn evaluate_api_version(
    requested: ApiVersion,
    policies: &[ApiVersionPolicy],
    now_unix_ms: u64,
) -> Result<ApiVersionDecision, ApiVersionError> {
    let Some(policy) = policies.iter().find(|policy| policy.version == requested) else {
        return Err(ApiVersionError::Unsupported { requested });
    };

    match policy.status {
        ApiVersionStatus::Current => Ok(ApiVersionDecision::Allowed),
        ApiVersionStatus::Deprecated {
            deprecated_at_unix_ms,
            sunset_at_unix_ms,
        } => {
            if let Some(sunset_at_unix_ms) = sunset_at_unix_ms
                && now_unix_ms >= sunset_at_unix_ms
            {
                return Err(ApiVersionError::Sunset {
                    requested,
                    sunset_at_unix_ms,
                });
            }
            Ok(ApiVersionDecision::AllowedDeprecated {
                deprecated_at_unix_ms,
                sunset_at_unix_ms,
            })
        }
        ApiVersionStatus::Sunset { sunset_at_unix_ms } => Err(ApiVersionError::Sunset {
            requested,
            sunset_at_unix_ms,
        }),
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cursor(String);

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cursor").field(&"<redacted>").finish()
    }
}

impl Cursor {
    pub fn new(value: impl Into<String>) -> Result<Self, CursorError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CursorError::Empty);
        }
        if value.len() > 512 {
            return Err(CursorError::TooLong {
                length: value.len(),
            });
        }
        if let Some((index, character)) = value
            .char_indices()
            .find(|(_, character)| !character.is_ascii_graphic())
        {
            return Err(CursorError::InvalidCharacter { index, character });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum CursorError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Debug for CursorError {
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

impl fmt::Display for CursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pagination cursor is invalid")
    }
}

impl Error for CursorError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorErrorStatus {
    BadRequest,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorErrorResponsePlan {
    pub status: CursorErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for CursorErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CursorErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_cursor_error_response(error: &CursorError) -> CursorErrorResponsePlan {
    match error {
        CursorError::Empty => CursorErrorResponsePlan {
            status: CursorErrorStatus::BadRequest,
            code: "pagination_cursor_invalid",
            message: "pagination cursor is invalid",
        },
        CursorError::TooLong { .. } | CursorError::InvalidCharacter { .. } => {
            CursorErrorResponsePlan {
                status: CursorErrorStatus::BadRequest,
                code: "pagination_cursor_invalid",
                message: "pagination cursor is invalid",
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    pub limit: u16,
    pub cursor: Option<Cursor>,
}

impl fmt::Debug for PageRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageRequest")
            .field("limit", &self.limit)
            .field("cursor", &self.cursor.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl PageRequest {
    pub const DEFAULT_LIMIT: u16 = 50;
    pub const MAX_LIMIT: u16 = 500;

    pub fn new(limit: Option<u16>, cursor: Option<Cursor>) -> Self {
        let limit = limit
            .unwrap_or(Self::DEFAULT_LIMIT)
            .clamp(1, Self::MAX_LIMIT);
        Self { limit, cursor }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
}

impl<T> fmt::Debug for Page<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Page")
            .field("item_count", &self.items.len())
            .field(
                "next_cursor",
                &self.next_cursor.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, next_cursor: Option<Cursor>) -> Self {
        Self { items, next_cursor }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityTag(String);

impl fmt::Debug for EntityTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EntityTag").field(&"<redacted>").finish()
    }
}

impl EntityTag {
    pub fn new(value: impl Into<String>) -> Result<Self, EntityTagError> {
        let value = value.into();
        if value.is_empty() {
            return Err(EntityTagError::Empty);
        }
        if value.len() > 256 {
            return Err(EntityTagError::TooLong {
                length: value.len(),
            });
        }
        if let Some((index, character)) = value
            .char_indices()
            .find(|(_, character)| !character.is_ascii_graphic())
        {
            return Err(EntityTagError::InvalidCharacter { index, character });
        }
        Ok(Self(value))
    }

    pub fn from_version(version: ResourceVersion) -> Self {
        Self(format!("W/\"{}\"", version.value()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum EntityTagError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Debug for EntityTagError {
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

impl fmt::Display for EntityTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity tag is invalid")
    }
}

impl Error for EntityTagError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityTagErrorStatus {
    BadRequest,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityTagErrorResponsePlan {
    pub status: EntityTagErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for EntityTagErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntityTagErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_entity_tag_error_response(error: &EntityTagError) -> EntityTagErrorResponsePlan {
    match error {
        EntityTagError::Empty => EntityTagErrorResponsePlan {
            status: EntityTagErrorStatus::BadRequest,
            code: "entity_tag_invalid",
            message: "entity tag is invalid",
        },
        EntityTagError::TooLong { .. } | EntityTagError::InvalidCharacter { .. } => {
            EntityTagErrorResponsePlan {
                status: EntityTagErrorStatus::BadRequest,
                code: "entity_tag_invalid",
                message: "entity tag is invalid",
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceVersion(u64);

impl fmt::Debug for ResourceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResourceVersion")
            .field(&"<redacted>")
            .finish()
    }
}

impl ResourceVersion {
    pub fn new(value: u64) -> Result<Self, ResourceVersionError> {
        if value == 0 {
            Err(ResourceVersionError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, ResourceVersionError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ResourceVersionError::Overflow)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResourceVersionError {
    Zero,
    Overflow,
}

impl fmt::Debug for ResourceVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("Zero"),
            Self::Overflow => f.write_str("Overflow"),
        }
    }
}

impl fmt::Display for ResourceVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resource version is invalid")
    }
}

impl Error for ResourceVersionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVersionErrorStatus {
    BadRequest,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceVersionErrorResponsePlan {
    pub status: ResourceVersionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for ResourceVersionErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceVersionErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_resource_version_error_response(
    _error: &ResourceVersionError,
) -> ResourceVersionErrorResponsePlan {
    ResourceVersionErrorResponsePlan {
        status: ResourceVersionErrorStatus::BadRequest,
        code: "resource_version_invalid",
        message: "resource version is invalid",
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ConcurrencyError {
    MissingPrecondition,
    VersionMismatch {
        expected: ResourceVersion,
        actual: ResourceVersion,
    },
    EntityTagMismatch {
        expected: EntityTag,
        actual: EntityTag,
    },
}

impl fmt::Debug for ConcurrencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrecondition => f.write_str("MissingPrecondition"),
            Self::VersionMismatch { .. } => f
                .debug_struct("VersionMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::EntityTagMismatch { .. } => f
                .debug_struct("EntityTagMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ConcurrencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrecondition => {
                write!(f, "mutation requires a concurrency precondition")
            }
            Self::VersionMismatch { .. } => write!(f, "resource version precondition failed"),
            Self::EntityTagMismatch { .. } => write!(f, "entity tag precondition failed"),
        }
    }
}

impl Error for ConcurrencyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyErrorStatus {
    PreconditionRequired,
    Conflict,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcurrencyErrorResponsePlan {
    pub status: ConcurrencyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for ConcurrencyErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcurrencyErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_concurrency_error_response(error: &ConcurrencyError) -> ConcurrencyErrorResponsePlan {
    match error {
        ConcurrencyError::MissingPrecondition => ConcurrencyErrorResponsePlan {
            status: ConcurrencyErrorStatus::PreconditionRequired,
            code: "mutation_precondition_required",
            message: "mutation requires a concurrency precondition",
        },
        ConcurrencyError::VersionMismatch { .. } => ConcurrencyErrorResponsePlan {
            status: ConcurrencyErrorStatus::Conflict,
            code: "resource_version_conflict",
            message: "resource version precondition failed",
        },
        ConcurrencyError::EntityTagMismatch { .. } => ConcurrencyErrorResponsePlan {
            status: ConcurrencyErrorStatus::Conflict,
            code: "entity_tag_conflict",
            message: "entity tag precondition failed",
        },
    }
}

pub fn require_matching_version(
    expected: Option<ResourceVersion>,
    actual: ResourceVersion,
) -> Result<(), ConcurrencyError> {
    let expected = expected.ok_or(ConcurrencyError::MissingPrecondition)?;
    if expected == actual {
        Ok(())
    } else {
        Err(ConcurrencyError::VersionMismatch { expected, actual })
    }
}

pub fn require_matching_etag(
    expected: Option<EntityTag>,
    actual: EntityTag,
) -> Result<(), ConcurrencyError> {
    let expected = expected.ok_or(ConcurrencyError::MissingPrecondition)?;
    if expected == actual {
        Ok(())
    } else {
        Err(ConcurrencyError::EntityTagMismatch { expected, actual })
    }
}
