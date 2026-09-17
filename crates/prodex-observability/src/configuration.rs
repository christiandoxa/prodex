#[cfg(not(feature = "mojo"))]
use prodex_domain::{JwksRefreshDecision, PolicyRefreshDecision};

use prodex_domain::{
    JwksCacheSnapshot, PolicyCacheStatus, PolicySnapshot, TelemetryAttribute,
    TelemetryAttributeError, evaluate_jwks_refresh, evaluate_policy_refresh,
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

#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::*;
    pub fn plan_enterprise_id_metric(
        kind: EnterpriseIdKind,
        result: EnterpriseIdResult,
    ) -> Result<EnterpriseIdMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let kind_label = crate::planning_support::planned_metric_label(31, 0, (kind) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let kind_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(68, "enterprise_id_kind"),
            enterprise_id_kind_label(kind),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(31, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let state_label = crate::planning_support::planned_metric_label(32, 0, (decision) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let state_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(84, "jwks_cache_state"),
            jwks_refresh_decision_label(decision),
        )?;
        Ok(JwksCacheAgeMetricPlan {
            metric_name: crate::planning_support::metric_name(32, 0, "prodex_jwks_cache_age_ms"),
            age_ms: snapshot
                .map(|snapshot| now_unix_ms.saturating_sub(snapshot.fetched_at_unix_ms)),
            state_label,
        })
    }

    pub fn plan_policy_snapshot_age_metric<T>(
        snapshot: Option<&PolicySnapshot<T>>,
        status: &PolicyCacheStatus,
        now_unix_ms: u64,
    ) -> Result<PolicySnapshotAgeMetricPlan, TelemetryAttributeError> {
        let decision = evaluate_policy_refresh(status, now_unix_ms);
        #[cfg(feature = "mojo")]
        let state_label = crate::planning_support::planned_metric_label(76, 0, (decision) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(33, 0, (outcome) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let operation_label =
            crate::planning_support::planned_metric_label(34, 0, (operation) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(90, "oidc_refresh_operation"),
            oidc_refresh_operation_label(operation),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(34, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(35, 0, (outcome) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let operation_label =
            crate::planning_support::planned_metric_label(36, 0, (operation) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(98, "policy_rollback_operation"),
            policy_rollback_operation_label(operation),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(36, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let source_label = crate::planning_support::planned_metric_label(28, 0, (source) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let source_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(57, "config_activation_source"),
            config_activation_source_label(source),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(28, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let target_label = crate::planning_support::planned_metric_label(30, 0, (target) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let target_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(61, "config_publication_target"),
            config_publication_delivery_target_label(target),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(30, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
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
        #[cfg(feature = "mojo")]
        let target_label = crate::planning_support::planned_metric_label(29, 0, (target) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let target_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(59, "config_invalidation_target"),
            config_cache_invalidation_target_label(target),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(29, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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

    #[cfg(not(feature = "mojo"))]
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
}

#[cfg(not(feature = "mojo"))]
pub use rust_compat::*;

#[cfg(feature = "mojo")]
mod mojo_impl {
    use super::*;

    macro_rules! one_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $value:ident : $value_type:ty => $label:ident) => {
            pub fn $name($value: $value_type) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $label: crate::planning_support::planned_metric_label($plan, 0, $value as i64)?,
                })
            }
        };
    }

    macro_rules! two_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $left:ident : $left_type:ty => $left_label:ident, $right:ident : $right_type:ty => $right_label:ident) => {
            pub fn $name(
                $left: $left_type,
                $right: $right_type,
            ) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $left_label: crate::planning_support::planned_metric_label(
                        $plan,
                        0,
                        $left as i64,
                    )?,
                    $right_label: crate::planning_support::planned_metric_label(
                        $plan,
                        1,
                        $right as i64,
                    )?,
                })
            }
        };
    }

    two_label_plan!(plan_enterprise_id_metric, EnterpriseIdMetricPlan, 31, kind: EnterpriseIdKind => kind_label, result: EnterpriseIdResult => result_label);
    one_label_plan!(plan_jwks_refresh_outcome_metric, JwksRefreshOutcomeMetricPlan, 33, outcome: JwksRefreshOutcome => result_label);
    two_label_plan!(plan_oidc_refresh_metric, OidcRefreshMetricPlan, 34, operation: OidcRefreshOperation => operation_label, result: OidcRefreshResult => result_label);
    one_label_plan!(plan_policy_refresh_outcome_metric, PolicyRefreshOutcomeMetricPlan, 35, outcome: PolicyRefreshOutcome => result_label);
    two_label_plan!(plan_policy_rollback_metric, PolicyRollbackMetricPlan, 36, operation: PolicyRollbackOperation => operation_label, result: PolicyRollbackResult => result_label);
    two_label_plan!(plan_config_activation_metric, ConfigActivationMetricPlan, 28, source: ConfigActivationSource => source_label, result: ConfigActivationResult => result_label);
    two_label_plan!(plan_config_publication_delivery_metric, ConfigPublicationDeliveryMetricPlan, 30, target: ConfigPublicationDeliveryTarget => target_label, result: ConfigPublicationDeliveryResult => result_label);
    two_label_plan!(plan_config_cache_invalidation_metric, ConfigCacheInvalidationMetricPlan, 29, target: ConfigCacheInvalidationTarget => target_label, result: ConfigCacheInvalidationResult => result_label);

    pub fn plan_jwks_cache_age_metric(
        snapshot: Option<&JwksCacheSnapshot>,
        now_unix_ms: u64,
    ) -> Result<JwksCacheAgeMetricPlan, TelemetryAttributeError> {
        let decision = evaluate_jwks_refresh(snapshot, now_unix_ms);
        Ok(JwksCacheAgeMetricPlan {
            metric_name: crate::planning_support::metric_name(32, 0, ""),
            age_ms: snapshot
                .map(|snapshot| now_unix_ms.saturating_sub(snapshot.fetched_at_unix_ms)),
            state_label: crate::planning_support::planned_metric_label(32, 0, decision as i64)?,
        })
    }

    pub fn plan_policy_snapshot_age_metric<T>(
        snapshot: Option<&PolicySnapshot<T>>,
        status: &PolicyCacheStatus,
        now_unix_ms: u64,
    ) -> Result<PolicySnapshotAgeMetricPlan, TelemetryAttributeError> {
        let decision = evaluate_policy_refresh(status, now_unix_ms);
        Ok(PolicySnapshotAgeMetricPlan {
            metric_name: "prodex_policy_snapshot_age_ms",
            age_ms: snapshot.map(|snapshot| now_unix_ms.saturating_sub(snapshot.issued_at_unix_ms)),
            state_label: crate::planning_support::planned_metric_label(76, 0, decision as i64)?,
        })
    }
}

#[cfg(feature = "mojo")]
pub use mojo_impl::*;
