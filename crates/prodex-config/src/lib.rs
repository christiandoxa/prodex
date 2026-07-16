#![forbid(unsafe_code)]
//! Revisioned configuration boundary primitives.
//!
//! This crate models control-plane configuration publishing and gateway cache
//! decisions without depending on filesystem, HTTP, storage, or async runtimes.

use std::error::Error;
use std::fmt;

use prodex_domain::{DataClassification, PolicyRevisionId, SecretPurpose, SecretRef, TenantId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceMode {
    Personal,
    EnterpriseObserve,
    EnterpriseEnforce,
    BankEnforce,
}

impl GovernanceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::EnterpriseObserve => "enterprise_observe",
            Self::EnterpriseEnforce => "enterprise_enforce",
            Self::BankEnforce => "bank_enforce",
        }
    }

    pub const fn is_enforcing(self) -> bool {
        matches!(self, Self::EnterpriseEnforce | Self::BankEnforce)
    }

    pub const fn allows_anonymous_compatibility(self) -> bool {
        matches!(self, Self::Personal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceRolloutMode {
    Off,
    Observe,
    Enforce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceUnknownClassificationBehavior {
    UseDefault,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernancePolicyFailureMode {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GovernanceSessionConfig {
    pub absolute_timeout_seconds: Option<u32>,
    pub idle_timeout_seconds: Option<u32>,
    pub max_concurrent: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GovernanceConfig {
    pub config_version: u32,
    pub mode: GovernanceMode,
    pub inspection: GovernanceRolloutMode,
    pub classification: GovernanceRolloutMode,
    pub policy: GovernanceRolloutMode,
    pub routing: GovernanceRolloutMode,
    pub mandatory_audit: bool,
    pub anonymous_data_plane: bool,
    pub raw_secret_sources: bool,
    pub classification_default: DataClassification,
    pub classification_unknown: GovernanceUnknownClassificationBehavior,
    pub policy_failure_mode: GovernancePolicyFailureMode,
    pub active_policy_revision: Option<PolicyRevisionId>,
    pub session: GovernanceSessionConfig,
}

impl GovernanceConfig {
    pub const fn personal_compatible() -> Self {
        Self {
            config_version: 1,
            mode: GovernanceMode::Personal,
            inspection: GovernanceRolloutMode::Off,
            classification: GovernanceRolloutMode::Off,
            policy: GovernanceRolloutMode::Off,
            routing: GovernanceRolloutMode::Off,
            mandatory_audit: false,
            anonymous_data_plane: true,
            raw_secret_sources: true,
            classification_default: DataClassification::Internal,
            classification_unknown: GovernanceUnknownClassificationBehavior::UseDefault,
            policy_failure_mode: GovernancePolicyFailureMode::Open,
            active_policy_revision: None,
            session: GovernanceSessionConfig {
                absolute_timeout_seconds: None,
                idle_timeout_seconds: None,
                max_concurrent: None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceConfigError {
    UnsupportedVersion,
    EnforceModeRequiresEnforcement,
    EnforceAuditRequired,
    BankIdentityRequired,
    BankSecretReferenceRequired,
    EnforceUnknownClassificationMustDeny,
    EnforcePolicyMustFailClosed,
    EnforceActiveRevisionRequired,
    EnforceSessionBoundsRequired,
    InvalidSessionBounds,
    BankRestrictedDefaultRequired,
}

impl fmt::Display for GovernanceConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "governance configuration is invalid")
    }
}

impl Error for GovernanceConfigError {}

pub fn validate_governance_config(
    config: GovernanceConfig,
) -> Result<GovernanceConfig, GovernanceConfigError> {
    if config.config_version != 1 {
        return Err(GovernanceConfigError::UnsupportedVersion);
    }
    if config.mode.is_enforcing()
        && [
            config.inspection,
            config.classification,
            config.policy,
            config.routing,
        ]
        .into_iter()
        .any(|mode| mode != GovernanceRolloutMode::Enforce)
    {
        return Err(GovernanceConfigError::EnforceModeRequiresEnforcement);
    }
    if config.mode.is_enforcing() && !config.mandatory_audit {
        return Err(GovernanceConfigError::EnforceAuditRequired);
    }
    if config.mode == GovernanceMode::BankEnforce {
        if config.anonymous_data_plane {
            return Err(GovernanceConfigError::BankIdentityRequired);
        }
        if config.raw_secret_sources {
            return Err(GovernanceConfigError::BankSecretReferenceRequired);
        }
        if config.classification_default != DataClassification::Restricted {
            return Err(GovernanceConfigError::BankRestrictedDefaultRequired);
        }
    }
    if config.mode.is_enforcing() {
        if config.classification_unknown != GovernanceUnknownClassificationBehavior::Deny {
            return Err(GovernanceConfigError::EnforceUnknownClassificationMustDeny);
        }
        if config.policy_failure_mode != GovernancePolicyFailureMode::Closed {
            return Err(GovernanceConfigError::EnforcePolicyMustFailClosed);
        }
        if config.active_policy_revision.is_none() {
            return Err(GovernanceConfigError::EnforceActiveRevisionRequired);
        }
        if config.session.absolute_timeout_seconds.is_none()
            || config.session.idle_timeout_seconds.is_none()
            || config.session.max_concurrent.is_none()
        {
            return Err(GovernanceConfigError::EnforceSessionBoundsRequired);
        }
    }
    let session = config.session;
    if session
        .absolute_timeout_seconds
        .is_some_and(|value| !(300..=86_400).contains(&value))
        || session
            .idle_timeout_seconds
            .is_some_and(|value| !(60..=3_600).contains(&value))
        || session
            .max_concurrent
            .is_some_and(|value| value == 0 || value > 10_000)
        || matches!(
            (session.idle_timeout_seconds, session.absolute_timeout_seconds),
            (Some(idle), Some(absolute)) if idle > absolute
        )
    {
        return Err(GovernanceConfigError::InvalidSessionBounds);
    }
    Ok(config)
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigRevision<T> {
    pub tenant_id: TenantId,
    pub revision_id: PolicyRevisionId,
    pub published_at_unix_ms: u64,
    pub payload: T,
}

impl<T> fmt::Debug for ConfigRevision<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigRevision")
            .field("tenant_id", &"<redacted>")
            .field("revision_id", &"<redacted>")
            .field("published_at_unix_ms", &"<redacted>")
            .field("payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ConfigCacheWindow {
    pub refresh_after_unix_ms: u64,
    pub stale_after_unix_ms: u64,
    pub expires_after_unix_ms: u64,
}

impl fmt::Debug for ConfigCacheWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigCacheWindow")
            .field("refresh_after_unix_ms", &"<redacted>")
            .field("stale_after_unix_ms", &"<redacted>")
            .field("expires_after_unix_ms", &"<redacted>")
            .finish()
    }
}

impl ConfigCacheWindow {
    pub fn new(
        refresh_after_unix_ms: u64,
        stale_after_unix_ms: u64,
        expires_after_unix_ms: u64,
    ) -> Result<Self, ConfigCacheWindowError> {
        if refresh_after_unix_ms > stale_after_unix_ms {
            return Err(ConfigCacheWindowError::RefreshAfterStale);
        }
        if stale_after_unix_ms > expires_after_unix_ms {
            return Err(ConfigCacheWindowError::StaleAfterExpiry);
        }
        Ok(Self {
            refresh_after_unix_ms,
            stale_after_unix_ms,
            expires_after_unix_ms,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigCacheWindowError {
    RefreshAfterStale,
    StaleAfterExpiry,
}

impl fmt::Display for ConfigCacheWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration cache window is invalid")
    }
}

impl Error for ConfigCacheWindowError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigCacheWindowErrorStatus {
    InvalidConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigCacheWindowErrorResponsePlan {
    pub status: ConfigCacheWindowErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_config_cache_window_error_response(
    _error: &ConfigCacheWindowError,
) -> ConfigCacheWindowErrorResponsePlan {
    ConfigCacheWindowErrorResponsePlan {
        status: ConfigCacheWindowErrorStatus::InvalidConfiguration,
        code: "configuration_cache_window_invalid",
        message: "configuration cache window is invalid",
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ConfigSecretSource {
    Reference(SecretRef),
    RawSecretMaterial,
}

impl fmt::Debug for ConfigSecretSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(_) => f.debug_tuple("Reference").field(&"<redacted>").finish(),
            Self::RawSecretMaterial => f.write_str("RawSecretMaterial"),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigSecretReferencePlan {
    pub tenant_id: TenantId,
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
}

impl fmt::Debug for ConfigSecretReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigSecretReferencePlan")
            .field("tenant_id", &"<redacted>")
            .field("reference", &self.reference)
            .field("purpose", &self.purpose)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigSecretReferenceError {
    RawSecretMaterialRejected,
    MalformedSecretReference,
}

impl fmt::Display for ConfigSecretReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration secrets must use secret references")
    }
}

impl Error for ConfigSecretReferenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigSecretReferenceErrorStatus {
    InvalidConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSecretReferenceErrorResponsePlan {
    pub status: ConfigSecretReferenceErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_config_secret_reference_error_response(
    error: &ConfigSecretReferenceError,
) -> ConfigSecretReferenceErrorResponsePlan {
    let code = match error {
        ConfigSecretReferenceError::RawSecretMaterialRejected => {
            "configuration_secret_reference_required"
        }
        ConfigSecretReferenceError::MalformedSecretReference => {
            "configuration_secret_reference_invalid"
        }
    };
    ConfigSecretReferenceErrorResponsePlan {
        status: ConfigSecretReferenceErrorStatus::InvalidConfiguration,
        code,
        message: "configuration secrets must use secret references",
    }
}

pub fn plan_config_secret_reference(
    tenant_id: TenantId,
    purpose: SecretPurpose,
    source: ConfigSecretSource,
) -> Result<ConfigSecretReferencePlan, ConfigSecretReferenceError> {
    match source {
        ConfigSecretSource::Reference(reference) => {
            if !reference.is_well_formed() {
                return Err(ConfigSecretReferenceError::MalformedSecretReference);
            }
            Ok(ConfigSecretReferencePlan {
                tenant_id,
                reference,
                purpose,
            })
        }
        ConfigSecretSource::RawSecretMaterial => {
            Err(ConfigSecretReferenceError::RawSecretMaterialRejected)
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigCacheState {
    pub tenant_id: TenantId,
    pub active_revision_id: Option<PolicyRevisionId>,
    pub last_known_good_revision_id: Option<PolicyRevisionId>,
    pub invalidated_revision_id: Option<PolicyRevisionId>,
    pub window: ConfigCacheWindow,
}

impl fmt::Debug for ConfigCacheState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigCacheState")
            .field("tenant_id", &"<redacted>")
            .field(
                "active_revision_id",
                &self.active_revision_id.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "last_known_good_revision_id",
                &self
                    .last_known_good_revision_id
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field(
                "invalidated_revision_id",
                &self.invalidated_revision_id.as_ref().map(|_| "<redacted>"),
            )
            .field("window", &self.window)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigRefreshDecision {
    UseActive,
    RefreshAsync,
    UseLastKnownGood,
    RefreshRequired,
    RejectedInvalidated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigRefreshError {
    RefreshRequired,
    InvalidatedRevisionRejected,
}

impl fmt::Display for ConfigRefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration is not currently available")
    }
}

impl Error for ConfigRefreshError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigRefreshErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigRefreshErrorResponsePlan {
    pub status: ConfigRefreshErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn config_refresh_error_for_decision(
    decision: ConfigRefreshDecision,
) -> Option<ConfigRefreshError> {
    match decision {
        ConfigRefreshDecision::RefreshRequired => Some(ConfigRefreshError::RefreshRequired),
        ConfigRefreshDecision::RejectedInvalidated => {
            Some(ConfigRefreshError::InvalidatedRevisionRejected)
        }
        ConfigRefreshDecision::UseActive
        | ConfigRefreshDecision::RefreshAsync
        | ConfigRefreshDecision::UseLastKnownGood => None,
    }
}

pub fn plan_config_refresh_error_response(
    error: &ConfigRefreshError,
) -> ConfigRefreshErrorResponsePlan {
    match error {
        ConfigRefreshError::RefreshRequired => ConfigRefreshErrorResponsePlan {
            status: ConfigRefreshErrorStatus::ServiceUnavailable,
            code: "configuration_refresh_required",
            message: "configuration is not currently available",
        },
        ConfigRefreshError::InvalidatedRevisionRejected => ConfigRefreshErrorResponsePlan {
            status: ConfigRefreshErrorStatus::ServiceUnavailable,
            code: "configuration_revision_unavailable",
            message: "configuration is not currently available",
        },
    }
}

pub fn evaluate_config_refresh(
    state: &ConfigCacheState,
    now_unix_ms: u64,
) -> ConfigRefreshDecision {
    if state.active_revision_id.is_some()
        && state.active_revision_id == state.invalidated_revision_id
    {
        return if state.last_known_good_revision_id.is_some()
            && state.last_known_good_revision_id != state.invalidated_revision_id
        {
            ConfigRefreshDecision::UseLastKnownGood
        } else {
            ConfigRefreshDecision::RejectedInvalidated
        };
    }
    if now_unix_ms < state.window.refresh_after_unix_ms && state.active_revision_id.is_some() {
        return ConfigRefreshDecision::UseActive;
    }
    if now_unix_ms < state.window.stale_after_unix_ms && state.active_revision_id.is_some() {
        return ConfigRefreshDecision::RefreshAsync;
    }
    if now_unix_ms < state.window.expires_after_unix_ms
        && state.last_known_good_revision_id.is_some()
        && state.last_known_good_revision_id != state.invalidated_revision_id
    {
        return ConfigRefreshDecision::UseLastKnownGood;
    }
    ConfigRefreshDecision::RefreshRequired
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigActivationPlan {
    pub previous_active_revision_id: Option<PolicyRevisionId>,
    pub activated_revision_id: PolicyRevisionId,
    pub next_state: ConfigCacheState,
}

impl fmt::Debug for ConfigActivationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigActivationPlan")
            .field(
                "previous_active_revision_id",
                &self
                    .previous_active_revision_id
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field("activated_revision_id", &"<redacted>")
            .field("next_state", &self.next_state)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationEventTarget {
    GatewayCacheRefresh,
    RuntimePolicyReload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigPublicationEventDelivery {
    pub gateway_cache_refresh: bool,
    pub runtime_policy_reload: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigPublicationEventPlan {
    pub tenant_id: TenantId,
    pub activated_revision_id: PolicyRevisionId,
    pub previous_active_revision_id: Option<PolicyRevisionId>,
    pub last_known_good_revision_id: Option<PolicyRevisionId>,
    pub targets: [ConfigPublicationEventTarget; 2],
}

impl fmt::Debug for ConfigPublicationEventPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigPublicationEventPlan")
            .field("tenant_id", &"<redacted>")
            .field("activated_revision_id", &"<redacted>")
            .field(
                "previous_active_revision_id",
                &self
                    .previous_active_revision_id
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field(
                "last_known_good_revision_id",
                &self
                    .last_known_good_revision_id
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field("targets", &self.targets)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationEventError {
    MissingGatewayCacheRefreshTarget,
    MissingRuntimePolicyReloadTarget,
}

impl fmt::Display for ConfigPublicationEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration publication event is incomplete")
    }
}

impl Error for ConfigPublicationEventError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationEventErrorStatus {
    InvalidConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationEventErrorResponsePlan {
    pub status: ConfigPublicationEventErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_config_publication_event_error_response(
    error: &ConfigPublicationEventError,
) -> ConfigPublicationEventErrorResponsePlan {
    match error {
        ConfigPublicationEventError::MissingGatewayCacheRefreshTarget
        | ConfigPublicationEventError::MissingRuntimePolicyReloadTarget => {
            ConfigPublicationEventErrorResponsePlan {
                status: ConfigPublicationEventErrorStatus::InvalidConfiguration,
                code: "configuration_publication_event_incomplete",
                message: "configuration publication event is incomplete",
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigInvalidationPlan {
    pub tenant_id: TenantId,
    pub invalidated_revision_id: PolicyRevisionId,
    pub next_state: ConfigCacheState,
}

impl fmt::Debug for ConfigInvalidationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigInvalidationPlan")
            .field("tenant_id", &"<redacted>")
            .field("invalidated_revision_id", &"<redacted>")
            .field("next_state", &self.next_state)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ConfigInvalidationError {
    TenantMismatch {
        expected: TenantId,
        actual: TenantId,
    },
    UnknownRevision {
        revision_id: PolicyRevisionId,
    },
}

impl fmt::Debug for ConfigInvalidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::UnknownRevision { .. } => f
                .debug_struct("UnknownRevision")
                .field("revision_id", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ConfigInvalidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "configuration request is invalid"),
            Self::UnknownRevision { .. } => write!(f, "configuration revision is not known"),
        }
    }
}

impl Error for ConfigInvalidationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigInvalidationErrorStatus {
    BadRequest,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigInvalidationErrorResponsePlan {
    pub status: ConfigInvalidationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_config_invalidation_error_response(
    error: &ConfigInvalidationError,
) -> ConfigInvalidationErrorResponsePlan {
    match error {
        ConfigInvalidationError::TenantMismatch { .. } => ConfigInvalidationErrorResponsePlan {
            status: ConfigInvalidationErrorStatus::BadRequest,
            code: "configuration_tenant_mismatch",
            message: "configuration tenant does not match request tenant",
        },
        ConfigInvalidationError::UnknownRevision { .. } => ConfigInvalidationErrorResponsePlan {
            status: ConfigInvalidationErrorStatus::Conflict,
            code: "configuration_revision_unknown",
            message: "configuration revision is not known",
        },
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ConfigPublicationError {
    TenantMismatch {
        expected: TenantId,
        actual: TenantId,
    },
    RevisionNotNewer {
        current: PolicyRevisionId,
        candidate: PolicyRevisionId,
    },
}

impl fmt::Debug for ConfigPublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::RevisionNotNewer { .. } => f
                .debug_struct("RevisionNotNewer")
                .field("current", &"<redacted>")
                .field("candidate", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ConfigPublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "configuration request is invalid"),
            Self::RevisionNotNewer { .. } => {
                write!(
                    f,
                    "configuration revision is not newer than active revision"
                )
            }
        }
    }
}

impl Error for ConfigPublicationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationErrorStatus {
    BadRequest,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationErrorResponsePlan {
    pub status: ConfigPublicationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_config_publication_error_response(
    error: &ConfigPublicationError,
) -> ConfigPublicationErrorResponsePlan {
    match error {
        ConfigPublicationError::TenantMismatch { .. } => ConfigPublicationErrorResponsePlan {
            status: ConfigPublicationErrorStatus::BadRequest,
            code: "configuration_tenant_mismatch",
            message: "configuration tenant does not match request tenant",
        },
        ConfigPublicationError::RevisionNotNewer { .. } => ConfigPublicationErrorResponsePlan {
            status: ConfigPublicationErrorStatus::Conflict,
            code: "configuration_revision_not_newer",
            message: "configuration revision is not newer than active revision",
        },
    }
}

pub fn validate_config_publication<T>(
    expected_tenant_id: TenantId,
    current_revision_id: Option<PolicyRevisionId>,
    candidate: &ConfigRevision<T>,
) -> Result<(), ConfigPublicationError> {
    if candidate.tenant_id != expected_tenant_id {
        return Err(ConfigPublicationError::TenantMismatch {
            expected: expected_tenant_id,
            actual: candidate.tenant_id,
        });
    }
    if let Some(current) = current_revision_id
        && candidate.revision_id <= current
    {
        return Err(ConfigPublicationError::RevisionNotNewer {
            current,
            candidate: candidate.revision_id,
        });
    }
    Ok(())
}

pub fn plan_config_activation<T>(
    state: &ConfigCacheState,
    candidate: &ConfigRevision<T>,
) -> Result<ConfigActivationPlan, ConfigPublicationError> {
    validate_config_publication(state.tenant_id, state.active_revision_id, candidate)?;
    let previous_active_revision_id = state.active_revision_id;
    let previous_active_is_invalidated = previous_active_revision_id.is_some()
        && previous_active_revision_id == state.invalidated_revision_id;
    let next_state = ConfigCacheState {
        tenant_id: state.tenant_id,
        active_revision_id: Some(candidate.revision_id),
        last_known_good_revision_id: if previous_active_is_invalidated {
            state
                .last_known_good_revision_id
                .filter(|revision_id| Some(*revision_id) != state.invalidated_revision_id)
        } else {
            previous_active_revision_id.or(state.last_known_good_revision_id)
        },
        invalidated_revision_id: None,
        window: state.window,
    };

    Ok(ConfigActivationPlan {
        previous_active_revision_id,
        activated_revision_id: candidate.revision_id,
        next_state,
    })
}

pub fn plan_config_publication_event(
    activation: &ConfigActivationPlan,
    delivery: ConfigPublicationEventDelivery,
) -> Result<ConfigPublicationEventPlan, ConfigPublicationEventError> {
    if !delivery.gateway_cache_refresh {
        return Err(ConfigPublicationEventError::MissingGatewayCacheRefreshTarget);
    }
    if !delivery.runtime_policy_reload {
        return Err(ConfigPublicationEventError::MissingRuntimePolicyReloadTarget);
    }

    Ok(ConfigPublicationEventPlan {
        tenant_id: activation.next_state.tenant_id,
        activated_revision_id: activation.activated_revision_id,
        previous_active_revision_id: activation.previous_active_revision_id,
        last_known_good_revision_id: activation.next_state.last_known_good_revision_id,
        targets: [
            ConfigPublicationEventTarget::GatewayCacheRefresh,
            ConfigPublicationEventTarget::RuntimePolicyReload,
        ],
    })
}

pub fn plan_config_invalidation(
    expected_tenant_id: TenantId,
    state: &ConfigCacheState,
    revision_id: PolicyRevisionId,
) -> Result<ConfigInvalidationPlan, ConfigInvalidationError> {
    if state.tenant_id != expected_tenant_id {
        return Err(ConfigInvalidationError::TenantMismatch {
            expected: expected_tenant_id,
            actual: state.tenant_id,
        });
    }
    if Some(revision_id) != state.active_revision_id
        && Some(revision_id) != state.last_known_good_revision_id
    {
        return Err(ConfigInvalidationError::UnknownRevision { revision_id });
    }

    let next_state = ConfigCacheState {
        tenant_id: state.tenant_id,
        active_revision_id: state.active_revision_id,
        last_known_good_revision_id: state.last_known_good_revision_id,
        invalidated_revision_id: Some(revision_id),
        window: state.window,
    };

    Ok(ConfigInvalidationPlan {
        tenant_id: expected_tenant_id,
        invalidated_revision_id: revision_id,
        next_state,
    })
}
