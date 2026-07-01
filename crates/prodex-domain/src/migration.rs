use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MigrationVersion(String);

impl fmt::Debug for MigrationVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MigrationVersion")
            .field(&"<redacted>")
            .finish()
    }
}

impl MigrationVersion {
    pub fn new(value: impl Into<String>) -> Result<Self, MigrationVersionError> {
        let value = value.into();
        if value.is_empty() {
            return Err(MigrationVersionError::Empty);
        }
        if let Some((index, character)) = value
            .char_indices()
            .find(|(_, character)| !character.is_ascii_graphic())
        {
            return Err(MigrationVersionError::InvalidCharacter { index, character });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum MigrationVersionError {
    Empty,
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Debug for MigrationVersionError {
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

impl fmt::Display for MigrationVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "migration version is invalid")
    }
}

impl Error for MigrationVersionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationVersionErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationVersionErrorResponsePlan {
    pub status: MigrationVersionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_migration_version_error_response(
    _error: &MigrationVersionError,
) -> MigrationVersionErrorResponsePlan {
    MigrationVersionErrorResponsePlan {
        status: MigrationVersionErrorStatus::BadRequest,
        code: "migration_version_invalid",
        message: "migration version is invalid",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationExecutionMode {
    DedicatedMigrator,
    RequestServingRuntime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStepState {
    Pending,
    Applied,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStepKind {
    Expand,
    Backfill,
    Verify,
    Contract,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationStep {
    pub version: MigrationVersion,
    pub description: String,
    pub state: MigrationStepState,
    pub kind: MigrationStepKind,
}

impl fmt::Debug for MigrationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MigrationStep")
            .field("version", &self.version)
            .field("description", &"<redacted>")
            .field("state", &self.state)
            .field("kind", &self.kind)
            .finish()
    }
}

impl MigrationStep {
    pub fn new(
        version: MigrationVersion,
        description: impl Into<String>,
        state: MigrationStepState,
    ) -> Self {
        Self {
            version,
            description: description.into(),
            state,
            kind: MigrationStepKind::Expand,
        }
    }

    pub fn with_kind(mut self, kind: MigrationStepKind) -> Self {
        self.kind = kind;
        self
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub mode: MigrationExecutionMode,
    pub lock_owner: Option<String>,
    pub steps: Vec<MigrationStep>,
}

impl fmt::Debug for MigrationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MigrationPlan")
            .field("mode", &self.mode)
            .field(
                "lock_owner",
                &self.lock_owner.as_deref().map(|_| "<redacted>"),
            )
            .field("step_count", &self.steps.len())
            .finish()
    }
}

impl MigrationPlan {
    pub fn new(
        mode: MigrationExecutionMode,
        lock_owner: Option<impl Into<String>>,
        steps: Vec<MigrationStep>,
    ) -> Self {
        Self {
            mode,
            lock_owner: lock_owner.map(Into::into),
            steps,
        }
    }

    pub fn pending_versions(&self) -> Vec<&MigrationVersion> {
        self.steps
            .iter()
            .filter(|step| step.state == MigrationStepState::Pending)
            .map(|step| &step.version)
            .collect()
    }

    pub fn can_execute(&self) -> Result<(), MigrationPlanError> {
        if self.mode == MigrationExecutionMode::RequestServingRuntime {
            return Err(MigrationPlanError::RequestServingExecutionForbidden);
        }
        if self.lock_owner.is_none() {
            return Err(MigrationPlanError::MigratorLockRequired);
        }
        if self
            .steps
            .iter()
            .any(|step| step.state == MigrationStepState::Failed)
        {
            return Err(MigrationPlanError::FailedStepPresent);
        }
        validate_expand_contract_order(&self.steps)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationPlanError {
    RequestServingExecutionForbidden,
    MigratorLockRequired,
    FailedStepPresent,
    ContractBeforeExpand,
    BackfillBeforeExpand,
    VerifyBeforeBackfill,
}

impl fmt::Display for MigrationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestServingExecutionForbidden => {
                write!(
                    f,
                    "migration execution is not allowed from request-serving runtime"
                )
            }
            Self::MigratorLockRequired => write!(f, "migration lock is required"),
            Self::FailedStepPresent => write!(f, "migration plan has unresolved failed steps"),
            Self::ContractBeforeExpand
            | Self::BackfillBeforeExpand
            | Self::VerifyBeforeBackfill => {
                write!(f, "migration step order is invalid")
            }
        }
    }
}

impl Error for MigrationPlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPlanErrorStatus {
    InvalidConfiguration,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationPlanErrorResponsePlan {
    pub status: MigrationPlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_migration_plan_error_response(
    error: &MigrationPlanError,
) -> MigrationPlanErrorResponsePlan {
    match error {
        MigrationPlanError::RequestServingExecutionForbidden => MigrationPlanErrorResponsePlan {
            status: MigrationPlanErrorStatus::InvalidConfiguration,
            code: "migration_request_path_forbidden",
            message: "migration execution is not allowed from request-serving runtime",
        },
        MigrationPlanError::MigratorLockRequired => MigrationPlanErrorResponsePlan {
            status: MigrationPlanErrorStatus::Conflict,
            code: "migration_lock_required",
            message: "migration lock is required",
        },
        MigrationPlanError::FailedStepPresent => MigrationPlanErrorResponsePlan {
            status: MigrationPlanErrorStatus::Conflict,
            code: "migration_failed_step_present",
            message: "migration plan has unresolved failed steps",
        },
        MigrationPlanError::ContractBeforeExpand
        | MigrationPlanError::BackfillBeforeExpand
        | MigrationPlanError::VerifyBeforeBackfill => MigrationPlanErrorResponsePlan {
            status: MigrationPlanErrorStatus::InvalidConfiguration,
            code: "migration_order_invalid",
            message: "migration step order is invalid",
        },
    }
}

pub fn validate_expand_contract_order(steps: &[MigrationStep]) -> Result<(), MigrationPlanError> {
    let mut saw_expand = false;
    let mut saw_backfill = false;
    for step in steps {
        match step.kind {
            MigrationStepKind::Expand => saw_expand = true,
            MigrationStepKind::Backfill => {
                if !saw_expand {
                    return Err(MigrationPlanError::BackfillBeforeExpand);
                }
                saw_backfill = true;
            }
            MigrationStepKind::Verify => {
                if !saw_backfill {
                    return Err(MigrationPlanError::VerifyBeforeBackfill);
                }
            }
            MigrationStepKind::Contract => {
                if !saw_expand {
                    return Err(MigrationPlanError::ContractBeforeExpand);
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationCompatibilityWindow {
    pub target_version: MigrationVersion,
    pub compatible_from_versions: Vec<MigrationVersion>,
}

impl fmt::Debug for MigrationCompatibilityWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MigrationCompatibilityWindow")
            .field("target_version", &self.target_version)
            .field(
                "compatible_from_count",
                &self.compatible_from_versions.len(),
            )
            .finish()
    }
}

impl MigrationCompatibilityWindow {
    pub fn new(
        target_version: MigrationVersion,
        mut compatible_from_versions: Vec<MigrationVersion>,
    ) -> Result<Self, MigrationCompatibilityError> {
        compatible_from_versions.sort();
        compatible_from_versions.dedup();
        if compatible_from_versions.len() < 2 {
            return Err(MigrationCompatibilityError::RequiresAtLeastTwoPreviousVersions);
        }
        Ok(Self {
            target_version,
            compatible_from_versions,
        })
    }

    pub fn supports_upgrade_from(&self, current_version: &MigrationVersion) -> bool {
        self.compatible_from_versions
            .binary_search(current_version)
            .is_ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationCompatibilityError {
    RequiresAtLeastTwoPreviousVersions,
    UnsupportedSourceVersion,
}

impl fmt::Display for MigrationCompatibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresAtLeastTwoPreviousVersions => {
                write!(f, "migration compatibility window is invalid")
            }
            Self::UnsupportedSourceVersion => {
                write!(f, "migration source version is unsupported")
            }
        }
    }
}

impl Error for MigrationCompatibilityError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationCompatibilityErrorStatus {
    InvalidConfiguration,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationCompatibilityErrorResponsePlan {
    pub status: MigrationCompatibilityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_migration_compatibility_error_response(
    error: &MigrationCompatibilityError,
) -> MigrationCompatibilityErrorResponsePlan {
    match error {
        MigrationCompatibilityError::RequiresAtLeastTwoPreviousVersions => {
            MigrationCompatibilityErrorResponsePlan {
                status: MigrationCompatibilityErrorStatus::InvalidConfiguration,
                code: "migration_compatibility_window_invalid",
                message: "migration compatibility window is invalid",
            }
        }
        MigrationCompatibilityError::UnsupportedSourceVersion => {
            MigrationCompatibilityErrorResponsePlan {
                status: MigrationCompatibilityErrorStatus::Conflict,
                code: "migration_source_version_unsupported",
                message: "migration source version is unsupported",
            }
        }
    }
}

pub fn validate_migration_compatibility(
    current_version: &MigrationVersion,
    window: &MigrationCompatibilityWindow,
) -> Result<(), MigrationCompatibilityError> {
    if window.supports_upgrade_from(current_version) {
        Ok(())
    } else {
        Err(MigrationCompatibilityError::UnsupportedSourceVersion)
    }
}
