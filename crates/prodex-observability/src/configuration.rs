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
    let kind_label =
        TelemetryAttribute::metric_label("enterprise_id_kind", enterprise_id_kind_label(kind));
    let result_label = TelemetryAttribute::metric_label(
        "enterprise_id_result",
        enterprise_id_result_label(result),
    );
    kind_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(EnterpriseIdMetricPlan {
        metric_name: "prodex_enterprise_id_events_total",
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
    let state_label =
        TelemetryAttribute::metric_label("jwks_cache_state", jwks_refresh_decision_label(decision));
    state_label.as_metric_label()?;
    Ok(JwksCacheAgeMetricPlan {
        metric_name: "prodex_jwks_cache_age_ms",
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
    let state_label = TelemetryAttribute::metric_label(
        "policy_cache_state",
        policy_refresh_decision_label(decision),
    );
    state_label.as_metric_label()?;
    Ok(PolicySnapshotAgeMetricPlan {
        metric_name: "prodex_policy_snapshot_age_ms",
        age_ms: snapshot.map(|snapshot| now_unix_ms.saturating_sub(snapshot.issued_at_unix_ms)),
        state_label,
    })
}

pub fn plan_jwks_refresh_outcome_metric(
    outcome: JwksRefreshOutcome,
) -> Result<JwksRefreshOutcomeMetricPlan, TelemetryAttributeError> {
    let result_label = TelemetryAttribute::metric_label(
        "jwks_refresh_result",
        jwks_refresh_outcome_label(outcome),
    );
    result_label.as_metric_label()?;
    Ok(JwksRefreshOutcomeMetricPlan {
        metric_name: "prodex_jwks_refresh_total",
        increment: 1,
        result_label,
    })
}

pub fn plan_oidc_refresh_metric(
    operation: OidcRefreshOperation,
    result: OidcRefreshResult,
) -> Result<OidcRefreshMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "oidc_refresh_operation",
        oidc_refresh_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("oidc_refresh_result", oidc_refresh_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(OidcRefreshMetricPlan {
        metric_name: "prodex_oidc_refresh_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_policy_refresh_outcome_metric(
    outcome: PolicyRefreshOutcome,
) -> Result<PolicyRefreshOutcomeMetricPlan, TelemetryAttributeError> {
    let result_label = TelemetryAttribute::metric_label(
        "policy_refresh_result",
        policy_refresh_outcome_label(outcome),
    );
    result_label.as_metric_label()?;
    Ok(PolicyRefreshOutcomeMetricPlan {
        metric_name: "prodex_policy_refresh_total",
        increment: 1,
        result_label,
    })
}

pub fn plan_policy_rollback_metric(
    operation: PolicyRollbackOperation,
    result: PolicyRollbackResult,
) -> Result<PolicyRollbackMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "policy_rollback_operation",
        policy_rollback_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "policy_rollback_result",
        policy_rollback_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PolicyRollbackMetricPlan {
        metric_name: "prodex_policy_rollback_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_config_activation_metric(
    source: ConfigActivationSource,
    result: ConfigActivationResult,
) -> Result<ConfigActivationMetricPlan, TelemetryAttributeError> {
    let source_label = TelemetryAttribute::metric_label(
        "config_activation_source",
        config_activation_source_label(source),
    );
    let result_label = TelemetryAttribute::metric_label(
        "config_activation_result",
        config_activation_result_label(result),
    );
    source_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ConfigActivationMetricPlan {
        metric_name: "prodex_config_activation_events_total",
        increment: 1,
        source_label,
        result_label,
    })
}

pub fn plan_config_publication_delivery_metric(
    target: ConfigPublicationDeliveryTarget,
    result: ConfigPublicationDeliveryResult,
) -> Result<ConfigPublicationDeliveryMetricPlan, TelemetryAttributeError> {
    let target_label = TelemetryAttribute::metric_label(
        "config_publication_target",
        config_publication_delivery_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        "config_publication_result",
        config_publication_delivery_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ConfigPublicationDeliveryMetricPlan {
        metric_name: "prodex_config_publication_delivery_total",
        increment: 1,
        target_label,
        result_label,
    })
}

pub fn plan_config_cache_invalidation_metric(
    target: ConfigCacheInvalidationTarget,
    result: ConfigCacheInvalidationResult,
) -> Result<ConfigCacheInvalidationMetricPlan, TelemetryAttributeError> {
    let target_label = TelemetryAttribute::metric_label(
        "config_invalidation_target",
        config_cache_invalidation_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        "config_invalidation_result",
        config_cache_invalidation_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ConfigCacheInvalidationMetricPlan {
        metric_name: "prodex_config_cache_invalidation_events_total",
        increment: 1,
        target_label,
        result_label,
    })
}

fn jwks_refresh_decision_label(decision: JwksRefreshDecision) -> &'static str {
    match decision {
        JwksRefreshDecision::UseFresh => "fresh",
        JwksRefreshDecision::RefreshNow => "refresh_now",
        JwksRefreshDecision::UseStaleWhileRevalidate => "stale_while_revalidate",
        JwksRefreshDecision::UseLastKnownGoodDuringBackoff => "last_known_good_backoff",
        JwksRefreshDecision::Unavailable => "unavailable",
    }
}

fn policy_refresh_decision_label(decision: PolicyRefreshDecision) -> &'static str {
    match decision {
        PolicyRefreshDecision::UseActive => "active",
        PolicyRefreshDecision::RefreshAsync => "refresh_async",
        PolicyRefreshDecision::UseLastKnownGoodAndRefresh => "last_known_good_refresh",
        PolicyRefreshDecision::Expired => "expired",
        PolicyRefreshDecision::Invalidated => "invalidated",
    }
}

fn jwks_refresh_outcome_label(outcome: JwksRefreshOutcome) -> &'static str {
    match outcome {
        JwksRefreshOutcome::Success => "success",
        JwksRefreshOutcome::Failure => "failure",
    }
}

fn oidc_refresh_operation_label(operation: OidcRefreshOperation) -> &'static str {
    match operation {
        OidcRefreshOperation::DiscoverIssuer => "discover_issuer",
        OidcRefreshOperation::FetchJwks => "fetch_jwks",
        OidcRefreshOperation::ValidateSnapshot => "validate_snapshot",
        OidcRefreshOperation::WriteCache => "write_cache",
    }
}

fn oidc_refresh_result_label(result: OidcRefreshResult) -> &'static str {
    match result {
        OidcRefreshResult::Success => "success",
        OidcRefreshResult::SkippedFresh => "skipped_fresh",
        OidcRefreshResult::Backoff => "backoff",
        OidcRefreshResult::InvalidSnapshot => "invalid_snapshot",
        OidcRefreshResult::Failed => "failed",
    }
}

fn enterprise_id_kind_label(kind: EnterpriseIdKind) -> &'static str {
    match kind {
        EnterpriseIdKind::Tenant => "tenant",
        EnterpriseIdKind::Principal => "principal",
        EnterpriseIdKind::Request => "request",
        EnterpriseIdKind::Call => "call",
        EnterpriseIdKind::Reservation => "reservation",
        EnterpriseIdKind::VirtualKey => "virtual_key",
        EnterpriseIdKind::PolicyRevision => "policy_revision",
        EnterpriseIdKind::AuditEvent => "audit_event",
    }
}

fn enterprise_id_result_label(result: EnterpriseIdResult) -> &'static str {
    match result {
        EnterpriseIdResult::Generated => "generated",
        EnterpriseIdResult::Parsed => "parsed",
        EnterpriseIdResult::Rejected => "rejected",
    }
}

fn policy_refresh_outcome_label(outcome: PolicyRefreshOutcome) -> &'static str {
    match outcome {
        PolicyRefreshOutcome::Success => "success",
        PolicyRefreshOutcome::Failure => "failure",
        PolicyRefreshOutcome::LastKnownGoodFallback => "last_known_good_fallback",
    }
}

fn policy_rollback_operation_label(operation: PolicyRollbackOperation) -> &'static str {
    match operation {
        PolicyRollbackOperation::ActivateLastKnownGood => "activate_last_known_good",
        PolicyRollbackOperation::RejectCandidate => "reject_candidate",
        PolicyRollbackOperation::Rollback => "rollback",
        PolicyRollbackOperation::Verify => "verify",
    }
}

fn policy_rollback_result_label(result: PolicyRollbackResult) -> &'static str {
    match result {
        PolicyRollbackResult::Success => "success",
        PolicyRollbackResult::Failed => "failed",
        PolicyRollbackResult::Blocked => "blocked",
        PolicyRollbackResult::Noop => "noop",
    }
}

fn config_activation_source_label(source: ConfigActivationSource) -> &'static str {
    match source {
        ConfigActivationSource::PublishedRevision => "published_revision",
        ConfigActivationSource::LastKnownGood => "last_known_good",
        ConfigActivationSource::Rollback => "rollback",
        ConfigActivationSource::InvalidationFallback => "invalidation_fallback",
    }
}

fn config_activation_result_label(result: ConfigActivationResult) -> &'static str {
    match result {
        ConfigActivationResult::Activated => "activated",
        ConfigActivationResult::Rejected => "rejected",
        ConfigActivationResult::MissingLastKnownGood => "missing_last_known_good",
        ConfigActivationResult::InvalidRevision => "invalid_revision",
    }
}

fn config_publication_delivery_target_label(
    target: ConfigPublicationDeliveryTarget,
) -> &'static str {
    match target {
        ConfigPublicationDeliveryTarget::GatewayCacheRefresh => "gateway_cache_refresh",
        ConfigPublicationDeliveryTarget::RuntimePolicyReload => "runtime_policy_reload",
        ConfigPublicationDeliveryTarget::AuditProjection => "audit_projection",
    }
}

fn config_publication_delivery_result_label(
    result: ConfigPublicationDeliveryResult,
) -> &'static str {
    match result {
        ConfigPublicationDeliveryResult::Delivered => "delivered",
        ConfigPublicationDeliveryResult::Failed => "failed",
        ConfigPublicationDeliveryResult::Skipped => "skipped",
        ConfigPublicationDeliveryResult::RetryScheduled => "retry_scheduled",
    }
}

fn config_cache_invalidation_target_label(target: ConfigCacheInvalidationTarget) -> &'static str {
    match target {
        ConfigCacheInvalidationTarget::GatewayPolicyCache => "gateway_policy_cache",
        ConfigCacheInvalidationTarget::RuntimePolicyCache => "runtime_policy_cache",
        ConfigCacheInvalidationTarget::RedisPolicyCache => "redis_policy_cache",
    }
}

fn config_cache_invalidation_result_label(result: ConfigCacheInvalidationResult) -> &'static str {
    match result {
        ConfigCacheInvalidationResult::Invalidated => "invalidated",
        ConfigCacheInvalidationResult::ReloadScheduled => "reload_scheduled",
        ConfigCacheInvalidationResult::NotFound => "not_found",
        ConfigCacheInvalidationResult::Failed => "failed",
    }
}
