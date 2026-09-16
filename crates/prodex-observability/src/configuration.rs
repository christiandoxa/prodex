use prodex_domain::{
    JwksCacheSnapshot, JwksRefreshDecision, PolicyCacheStatus, PolicyRefreshDecision,
    PolicySnapshot, TelemetryAttribute, TelemetryAttributeError, evaluate_jwks_refresh,
    evaluate_policy_refresh,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterpriseIdKind {
    Tenant,
    Principal,
    Request,
    Call,
    Reservation,
    VirtualKey,
    PolicyRevision,
    AuditEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterpriseIdResult {
    Generated,
    Parsed,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnterpriseIdMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub kind_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JwksCacheAgeMetricPlan {
    pub metric_name: &'static str,
    pub age_ms: Option<u64>,
    pub state_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicySnapshotAgeMetricPlan {
    pub metric_name: &'static str,
    pub age_ms: Option<u64>,
    pub state_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JwksRefreshOutcome {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcRefreshOperation {
    DiscoverIssuer,
    FetchJwks,
    ValidateSnapshot,
    WriteCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcRefreshResult {
    Success,
    SkippedFresh,
    Backoff,
    InvalidSnapshot,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRefreshOutcome {
    Success,
    Failure,
    LastKnownGoodFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JwksRefreshOutcomeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcRefreshMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRefreshOutcomeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRollbackOperation {
    ActivateLastKnownGood,
    RejectCandidate,
    Rollback,
    Verify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyRollbackResult {
    Success,
    Failed,
    Blocked,
    Noop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRollbackMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigActivationSource {
    PublishedRevision,
    LastKnownGood,
    Rollback,
    InvalidationFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigActivationResult {
    Activated,
    Rejected,
    MissingLastKnownGood,
    InvalidRevision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigActivationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub source_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationDeliveryTarget {
    GatewayCacheRefresh,
    RuntimePolicyReload,
    AuditProjection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPublicationDeliveryResult {
    Delivered,
    Failed,
    Skipped,
    RetryScheduled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationDeliveryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub target_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigCacheInvalidationTarget {
    GatewayPolicyCache,
    RuntimePolicyCache,
    RedisPolicyCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigCacheInvalidationResult {
    Invalidated,
    ReloadScheduled,
    NotFound,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigCacheInvalidationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub target_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_enterprise_id_metric(
    kind: EnterpriseIdKind,
    result: EnterpriseIdResult,
) -> Result<EnterpriseIdMetricPlan, TelemetryAttributeError> {
    let kind_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(68, "enterprise_id_kind"),
        enterprise_id_kind_label(kind),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(69, "enterprise_id_result"),
        enterprise_id_result_label(result),
    )?;
    Ok(EnterpriseIdMetricPlan {
        metric_name: crate::planning_support::metric_name(
            31,
            0,
            "prodex_enterprise_id_events_total",
        ),
        increment: 1,
        kind_label,
        result_label,
    })
}

pub fn plan_jwks_cache_age_metric(
    snapshot: Option<&JwksCacheSnapshot>,
    now_unix_ms: u64,
) -> Result<JwksCacheAgeMetricPlan, TelemetryAttributeError> {
    let decision = evaluate_jwks_refresh(snapshot, now_unix_ms);
    let state_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(84, "jwks_cache_state"),
        jwks_refresh_decision_label(decision),
    )?;
    Ok(JwksCacheAgeMetricPlan {
        metric_name: crate::planning_support::metric_name(32, 0, "prodex_jwks_cache_age_ms"),
        age_ms: snapshot.map(|snapshot| now_unix_ms.saturating_sub(snapshot.fetched_at_unix_ms)),
        state_label,
    })
}

pub fn plan_policy_snapshot_age_metric<T>(
    snapshot: Option<&PolicySnapshot<T>>,
    status: &PolicyCacheStatus,
    now_unix_ms: u64,
) -> Result<PolicySnapshotAgeMetricPlan, TelemetryAttributeError> {
    let decision = evaluate_policy_refresh(status, now_unix_ms);
    let state_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(94, "policy_cache_state"),
        policy_refresh_decision_label(decision),
    )?;
    Ok(PolicySnapshotAgeMetricPlan {
        metric_name: "prodex_policy_snapshot_age_ms",
        age_ms: snapshot.map(|snapshot| now_unix_ms.saturating_sub(snapshot.issued_at_unix_ms)),
        state_label,
    })
}

pub fn plan_jwks_refresh_outcome_metric(
    outcome: JwksRefreshOutcome,
) -> Result<JwksRefreshOutcomeMetricPlan, TelemetryAttributeError> {
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(85, "jwks_refresh_result"),
        jwks_refresh_outcome_label(outcome),
    )?;
    Ok(JwksRefreshOutcomeMetricPlan {
        metric_name: crate::planning_support::metric_name(33, 0, "prodex_jwks_refresh_total"),
        increment: 1,
        result_label,
    })
}

pub fn plan_oidc_refresh_metric(
    operation: OidcRefreshOperation,
    result: OidcRefreshResult,
) -> Result<OidcRefreshMetricPlan, TelemetryAttributeError> {
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(90, "oidc_refresh_operation"),
        oidc_refresh_operation_label(operation),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(91, "oidc_refresh_result"),
        oidc_refresh_result_label(result),
    )?;
    Ok(OidcRefreshMetricPlan {
        metric_name: crate::planning_support::metric_name(
            34,
            0,
            "prodex_oidc_refresh_events_total",
        ),
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_policy_refresh_outcome_metric(
    outcome: PolicyRefreshOutcome,
) -> Result<PolicyRefreshOutcomeMetricPlan, TelemetryAttributeError> {
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(97, "policy_refresh_result"),
        policy_refresh_outcome_label(outcome),
    )?;
    Ok(PolicyRefreshOutcomeMetricPlan {
        metric_name: crate::planning_support::metric_name(35, 0, "prodex_policy_refresh_total"),
        increment: 1,
        result_label,
    })
}

pub fn plan_policy_rollback_metric(
    operation: PolicyRollbackOperation,
    result: PolicyRollbackResult,
) -> Result<PolicyRollbackMetricPlan, TelemetryAttributeError> {
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(98, "policy_rollback_operation"),
        policy_rollback_operation_label(operation),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(99, "policy_rollback_result"),
        policy_rollback_result_label(result),
    )?;
    Ok(PolicyRollbackMetricPlan {
        metric_name: crate::planning_support::metric_name(
            36,
            0,
            "prodex_policy_rollback_events_total",
        ),
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_config_activation_metric(
    source: ConfigActivationSource,
    result: ConfigActivationResult,
) -> Result<ConfigActivationMetricPlan, TelemetryAttributeError> {
    let source_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(57, "config_activation_source"),
        config_activation_source_label(source),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(56, "config_activation_result"),
        config_activation_result_label(result),
    )?;
    Ok(ConfigActivationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            28,
            0,
            "prodex_config_activation_events_total",
        ),
        increment: 1,
        source_label,
        result_label,
    })
}

pub fn plan_config_publication_delivery_metric(
    target: ConfigPublicationDeliveryTarget,
    result: ConfigPublicationDeliveryResult,
) -> Result<ConfigPublicationDeliveryMetricPlan, TelemetryAttributeError> {
    let target_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(61, "config_publication_target"),
        config_publication_delivery_target_label(target),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(60, "config_publication_result"),
        config_publication_delivery_result_label(result),
    )?;
    Ok(ConfigPublicationDeliveryMetricPlan {
        metric_name: crate::planning_support::metric_name(
            30,
            0,
            "prodex_config_publication_delivery_total",
        ),
        increment: 1,
        target_label,
        result_label,
    })
}

pub fn plan_config_cache_invalidation_metric(
    target: ConfigCacheInvalidationTarget,
    result: ConfigCacheInvalidationResult,
) -> Result<ConfigCacheInvalidationMetricPlan, TelemetryAttributeError> {
    let target_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(59, "config_invalidation_target"),
        config_cache_invalidation_target_label(target),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(58, "config_invalidation_result"),
        config_cache_invalidation_result_label(result),
    )?;
    Ok(ConfigCacheInvalidationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            29,
            0,
            "prodex_config_cache_invalidation_events_total",
        ),
        increment: 1,
        target_label,
        result_label,
    })
}

fn jwks_refresh_decision_label(decision: JwksRefreshDecision) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(60, decision as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match decision {
            JwksRefreshDecision::UseFresh => "fresh",
            JwksRefreshDecision::RefreshNow => "refresh_now",
            JwksRefreshDecision::UseStaleWhileRevalidate => "stale_while_revalidate",
            JwksRefreshDecision::UseLastKnownGoodDuringBackoff => "last_known_good_backoff",
            JwksRefreshDecision::Unavailable => "unavailable",
        })
        .to_string()
    }
}

fn policy_refresh_decision_label(decision: PolicyRefreshDecision) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(64, decision as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match decision {
            PolicyRefreshDecision::UseActive => "active",
            PolicyRefreshDecision::RefreshAsync => "refresh_async",
            PolicyRefreshDecision::UseLastKnownGoodAndRefresh => "last_known_good_refresh",
            PolicyRefreshDecision::Expired => "expired",
            PolicyRefreshDecision::Invalidated => "invalidated",
        })
        .to_string()
    }
}

fn jwks_refresh_outcome_label(outcome: JwksRefreshOutcome) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(61, outcome as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match outcome {
            JwksRefreshOutcome::Success => "success",
            JwksRefreshOutcome::Failure => "failure",
        })
        .to_string()
    }
}

fn oidc_refresh_operation_label(operation: OidcRefreshOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(62, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            OidcRefreshOperation::DiscoverIssuer => "discover_issuer",
            OidcRefreshOperation::FetchJwks => "fetch_jwks",
            OidcRefreshOperation::ValidateSnapshot => "validate_snapshot",
            OidcRefreshOperation::WriteCache => "write_cache",
        })
        .to_string()
    }
}

fn oidc_refresh_result_label(result: OidcRefreshResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(63, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            OidcRefreshResult::Success => "success",
            OidcRefreshResult::SkippedFresh => "skipped_fresh",
            OidcRefreshResult::Backoff => "backoff",
            OidcRefreshResult::InvalidSnapshot => "invalid_snapshot",
            OidcRefreshResult::Failed => "failed",
        })
        .to_string()
    }
}

fn enterprise_id_kind_label(kind: EnterpriseIdKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(58, kind as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match kind {
            EnterpriseIdKind::Tenant => "tenant",
            EnterpriseIdKind::Principal => "principal",
            EnterpriseIdKind::Request => "request",
            EnterpriseIdKind::Call => "call",
            EnterpriseIdKind::Reservation => "reservation",
            EnterpriseIdKind::VirtualKey => "virtual_key",
            EnterpriseIdKind::PolicyRevision => "policy_revision",
            EnterpriseIdKind::AuditEvent => "audit_event",
        })
        .to_string()
    }
}

fn enterprise_id_result_label(result: EnterpriseIdResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(59, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            EnterpriseIdResult::Generated => "generated",
            EnterpriseIdResult::Parsed => "parsed",
            EnterpriseIdResult::Rejected => "rejected",
        })
        .to_string()
    }
}

fn policy_refresh_outcome_label(outcome: PolicyRefreshOutcome) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(65, outcome as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match outcome {
            PolicyRefreshOutcome::Success => "success",
            PolicyRefreshOutcome::Failure => "failure",
            PolicyRefreshOutcome::LastKnownGoodFallback => "last_known_good_fallback",
        })
        .to_string()
    }
}

fn policy_rollback_operation_label(operation: PolicyRollbackOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(66, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            PolicyRollbackOperation::ActivateLastKnownGood => "activate_last_known_good",
            PolicyRollbackOperation::RejectCandidate => "reject_candidate",
            PolicyRollbackOperation::Rollback => "rollback",
            PolicyRollbackOperation::Verify => "verify",
        })
        .to_string()
    }
}

fn policy_rollback_result_label(result: PolicyRollbackResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(67, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            PolicyRollbackResult::Success => "success",
            PolicyRollbackResult::Failed => "failed",
            PolicyRollbackResult::Blocked => "blocked",
            PolicyRollbackResult::Noop => "noop",
        })
        .to_string()
    }
}

fn config_activation_source_label(source: ConfigActivationSource) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(55, source as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match source {
            ConfigActivationSource::PublishedRevision => "published_revision",
            ConfigActivationSource::LastKnownGood => "last_known_good",
            ConfigActivationSource::Rollback => "rollback",
            ConfigActivationSource::InvalidationFallback => "invalidation_fallback",
        })
        .to_string()
    }
}

fn config_activation_result_label(result: ConfigActivationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(54, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ConfigActivationResult::Activated => "activated",
            ConfigActivationResult::Rejected => "rejected",
            ConfigActivationResult::MissingLastKnownGood => "missing_last_known_good",
            ConfigActivationResult::InvalidRevision => "invalid_revision",
        })
        .to_string()
    }
}

fn config_publication_delivery_target_label(target: ConfigPublicationDeliveryTarget) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(138, target as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match target {
            ConfigPublicationDeliveryTarget::GatewayCacheRefresh => "gateway_cache_refresh",
            ConfigPublicationDeliveryTarget::RuntimePolicyReload => "runtime_policy_reload",
            ConfigPublicationDeliveryTarget::AuditProjection => "audit_projection",
        })
        .to_string()
    }
}

fn config_publication_delivery_result_label(result: ConfigPublicationDeliveryResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(139, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ConfigPublicationDeliveryResult::Delivered => "delivered",
            ConfigPublicationDeliveryResult::Failed => "failed",
            ConfigPublicationDeliveryResult::Skipped => "skipped",
            ConfigPublicationDeliveryResult::RetryScheduled => "retry_scheduled",
        })
        .to_string()
    }
}

fn config_cache_invalidation_target_label(target: ConfigCacheInvalidationTarget) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(57, target as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match target {
            ConfigCacheInvalidationTarget::GatewayPolicyCache => "gateway_policy_cache",
            ConfigCacheInvalidationTarget::RuntimePolicyCache => "runtime_policy_cache",
            ConfigCacheInvalidationTarget::RedisPolicyCache => "redis_policy_cache",
        })
        .to_string()
    }
}

fn config_cache_invalidation_result_label(result: ConfigCacheInvalidationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(56, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ConfigCacheInvalidationResult::Invalidated => "invalidated",
            ConfigCacheInvalidationResult::ReloadScheduled => "reload_scheduled",
            ConfigCacheInvalidationResult::NotFound => "not_found",
            ConfigCacheInvalidationResult::Failed => "failed",
        })
        .to_string()
    }
}
