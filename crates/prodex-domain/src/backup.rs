use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BackupId(String);

impl fmt::Debug for BackupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BackupId").field(&"<redacted>").finish()
    }
}

impl BackupId {
    pub fn new(value: impl Into<String>) -> Result<Self, BackupIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(BackupIdError::Empty);
        }
        if let Some((index, character)) = value
            .char_indices()
            .find(|(_, character)| !character.is_ascii_graphic())
        {
            return Err(BackupIdError::InvalidCharacter { index, character });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BackupIdError {
    Empty,
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Debug for BackupIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::InvalidCharacter { .. } => f
                .debug_struct("InvalidCharacter")
                .field("index", &"<redacted>")
                .field("character", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for BackupIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::InvalidCharacter { .. } => write!(f, "backup id is invalid"),
        }
    }
}

impl Error for BackupIdError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupIdErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupIdErrorResponsePlan {
    pub status: BackupIdErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_backup_id_error_response(_error: &BackupIdError) -> BackupIdErrorResponsePlan {
    BackupIdErrorResponsePlan {
        status: BackupIdErrorStatus::BadRequest,
        code: "backup_id_invalid",
        message: "backup id is invalid",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupStatus {
    Completed,
    Failed,
    Expired,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub id: BackupId,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub status: BackupStatus,
    pub checksum: Option<String>,
}

impl fmt::Debug for BackupSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackupSnapshot")
            .field("id", &self.id)
            .field("created_at_unix_ms", &"<redacted>")
            .field("expires_at_unix_ms", &"<redacted>")
            .field("status", &self.status)
            .field("checksum", &self.checksum.as_deref().map(|_| "<redacted>"))
            .finish()
    }
}

impl BackupSnapshot {
    pub fn is_expired_at(&self, now_unix_ms: u64) -> bool {
        self.status == BackupStatus::Expired || now_unix_ms >= self.expires_at_unix_ms
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorePlan {
    pub backup: BackupSnapshot,
    pub expected_checksum: Option<String>,
    pub now_unix_ms: u64,
}

impl fmt::Debug for RestorePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RestorePlan")
            .field("backup", &self.backup)
            .field(
                "expected_checksum",
                &self.expected_checksum.as_deref().map(|_| "<redacted>"),
            )
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestorePlanError {
    BackupNotCompleted,
    BackupExpired,
    InvalidExpiry,
    MissingChecksum,
    MalformedChecksum,
    ChecksumMismatch,
}

impl fmt::Display for RestorePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackupNotCompleted | Self::BackupExpired | Self::InvalidExpiry => {
                write!(f, "backup is not restorable")
            }
            Self::MissingChecksum | Self::MalformedChecksum => {
                write!(f, "backup integrity cannot be verified")
            }
            Self::ChecksumMismatch => write!(f, "backup integrity verification failed"),
        }
    }
}

impl Error for RestorePlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestorePlanErrorStatus {
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorePlanErrorResponsePlan {
    pub status: RestorePlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_restore_error_response(error: &RestorePlanError) -> RestorePlanErrorResponsePlan {
    match error {
        RestorePlanError::BackupNotCompleted => RestorePlanErrorResponsePlan {
            status: RestorePlanErrorStatus::Conflict,
            code: "restore_backup_not_completed",
            message: "backup is not restorable",
        },
        RestorePlanError::BackupExpired => RestorePlanErrorResponsePlan {
            status: RestorePlanErrorStatus::Conflict,
            code: "restore_backup_expired",
            message: "backup is not restorable",
        },
        RestorePlanError::InvalidExpiry => RestorePlanErrorResponsePlan {
            status: RestorePlanErrorStatus::Conflict,
            code: "restore_backup_expiry_invalid",
            message: "backup is not restorable",
        },
        RestorePlanError::MissingChecksum => RestorePlanErrorResponsePlan {
            status: RestorePlanErrorStatus::Conflict,
            code: "restore_checksum_unavailable",
            message: "backup integrity cannot be verified",
        },
        RestorePlanError::MalformedChecksum => RestorePlanErrorResponsePlan {
            status: RestorePlanErrorStatus::Conflict,
            code: "restore_checksum_invalid",
            message: "backup integrity cannot be verified",
        },
        RestorePlanError::ChecksumMismatch => RestorePlanErrorResponsePlan {
            status: RestorePlanErrorStatus::Conflict,
            code: "restore_checksum_mismatch",
            message: "backup integrity verification failed",
        },
    }
}

impl RestorePlan {
    pub fn validate(&self) -> Result<(), RestorePlanError> {
        if self.backup.status != BackupStatus::Completed {
            return Err(RestorePlanError::BackupNotCompleted);
        }
        if self.backup.expires_at_unix_ms <= self.backup.created_at_unix_ms {
            return Err(RestorePlanError::InvalidExpiry);
        }
        if self.backup.is_expired_at(self.now_unix_ms) {
            return Err(RestorePlanError::BackupExpired);
        }
        let Some(actual_checksum) = self.backup.checksum.as_deref() else {
            return Err(RestorePlanError::MissingChecksum);
        };
        if !checksum_is_well_formed(actual_checksum) {
            return Err(RestorePlanError::MalformedChecksum);
        }
        if let Some(expected_checksum) = self.expected_checksum.as_deref() {
            if !checksum_is_well_formed(expected_checksum) {
                return Err(RestorePlanError::MalformedChecksum);
            }
            if expected_checksum != actual_checksum {
                return Err(RestorePlanError::ChecksumMismatch);
            }
        }
        Ok(())
    }
}

fn checksum_is_well_formed(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && value.chars().all(|ch| ch.is_ascii_graphic())
}
