use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownLifecycleEvent {
    SignalReceived,
    DrainingStarted,
    ReadinessDisabled,
    InflightDrained,
    TimeoutElapsed,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownLifecycleResult {
    Success,
    Timeout,
    Forced,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShutdownLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub event_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthProbeKind {
    Live,
    Ready,
    Startup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthProbeResult {
    Passing,
    Degraded,
    Failing,
    Draining,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthProbeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub probe_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretProviderBackend {
    File,
    Keyring,
    ExternalManager,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretProviderOperation {
    Read,
    Write,
    Delete,
    RevisionLookup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretProviderResult {
    Success,
    NotFound,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretProviderMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub backend_label: TelemetryAttribute,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretRotationScope {
    ProviderCredential,
    OidcClient,
    SigningKey,
    StorageCredential,
    WebhookSecret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretRotationResult {
    Success,
    Failed,
    Skipped,
    Rollback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretRotationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub scope_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupRestoreOperation {
    Backup,
    Restore,
    Verify,
    Drill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupRestoreResult {
    Success,
    Failed,
    Partial,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupRestoreMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentRolloutOperation {
    Apply,
    Verify,
    Promote,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentRolloutResult {
    Success,
    Failed,
    Degraded,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentRolloutMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadSoakScenarioKind {
    Load,
    Soak,
    Spike,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadSoakResult {
    Passed,
    Failed,
    Aborted,
    ThresholdBreached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadSoakMetricPlan {
    pub event_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub scenario_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultInjectionTarget {
    Postgres,
    Redis,
    Idp,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultInjectionResult {
    Injected,
    Recovered,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultInjectionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub target_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationLifecycleOperation {
    StatusCheck,
    CompatibilityCheck,
    Apply,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationLifecycleResult {
    Compatible,
    Applied,
    Blocked,
    Failed,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceOperation {
    Read,
    Write,
    Commit,
    Rollback,
    HealthCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceResult {
    Success,
    Conflict,
    Timeout,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistenceMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::*;
    pub fn plan_shutdown_lifecycle_metric(
        event: ShutdownLifecycleEvent,
        result: ShutdownLifecycleResult,
    ) -> Result<ShutdownLifecycleMetricPlan, TelemetryAttributeError> {
        let event_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(136, "shutdown_event"),
            shutdown_lifecycle_event_label(event),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(137, "shutdown_result"),
            shutdown_lifecycle_result_label(result),
        )?;
        Ok(ShutdownLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                46,
                0,
                "prodex_shutdown_lifecycle_total",
            ),
            increment: 1,
            event_label,
            result_label,
        })
    }

    pub fn plan_health_probe_metric(
        probe: HealthProbeKind,
        result: HealthProbeResult,
    ) -> Result<HealthProbeMetricPlan, TelemetryAttributeError> {
        let probe_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(72, "health_probe"),
            health_probe_kind_label(probe),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(73, "health_result"),
            health_probe_result_label(result),
        )?;
        Ok(HealthProbeMetricPlan {
            metric_name: crate::planning_support::metric_name(
                40,
                0,
                "prodex_health_probe_results_total",
            ),
            increment: 1,
            probe_label,
            result_label,
        })
    }

    pub fn plan_secret_provider_metric(
        backend: SecretProviderBackend,
        operation: SecretProviderOperation,
        result: SecretProviderResult,
    ) -> Result<SecretProviderMetricPlan, TelemetryAttributeError> {
        let backend_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(127, "secret_backend"),
            secret_provider_backend_label(backend),
        )?;
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(128, "secret_operation"),
            secret_provider_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(129, "secret_result"),
            secret_provider_result_label(result),
        )?;
        Ok(SecretProviderMetricPlan {
            metric_name: crate::planning_support::metric_name(
                44,
                0,
                "prodex_secret_provider_operations_total",
            ),
            increment: 1,
            backend_label,
            operation_label,
            result_label,
        })
    }

    pub fn plan_secret_rotation_metric(
        scope: SecretRotationScope,
        result: SecretRotationResult,
    ) -> Result<SecretRotationMetricPlan, TelemetryAttributeError> {
        let scope_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(131, "secret_scope"),
            secret_rotation_scope_label(scope),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(130, "secret_rotation_result"),
            secret_rotation_result_label(result),
        )?;
        Ok(SecretRotationMetricPlan {
            metric_name: crate::planning_support::metric_name(
                45,
                0,
                "prodex_secret_rotation_events_total",
            ),
            increment: 1,
            scope_label,
            result_label,
        })
    }

    pub fn plan_backup_restore_metric(
        operation: BackupRestoreOperation,
        result: BackupRestoreResult,
    ) -> Result<BackupRestoreMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(47, "backup_restore_operation"),
            backup_restore_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(48, "backup_restore_result"),
            backup_restore_result_label(result),
        )?;
        Ok(BackupRestoreMetricPlan {
            metric_name: crate::planning_support::metric_name(
                37,
                0,
                "prodex_backup_restore_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_deployment_rollout_metric(
        operation: DeploymentRolloutOperation,
        result: DeploymentRolloutResult,
    ) -> Result<DeploymentRolloutMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(66, "deployment_rollout_operation"),
            deployment_rollout_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(67, "deployment_rollout_result"),
            deployment_rollout_result_label(result),
        )?;
        Ok(DeploymentRolloutMetricPlan {
            metric_name: crate::planning_support::metric_name(
                38,
                0,
                "prodex_deployment_rollout_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_load_soak_metric(
        scenario: LoadSoakScenarioKind,
        result: LoadSoakResult,
        duration_ms: u64,
    ) -> Result<LoadSoakMetricPlan, TelemetryAttributeError> {
        let scenario_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(87, "load_soak_scenario"),
            load_soak_scenario_label(scenario),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(86, "load_soak_result"),
            load_soak_result_label(result),
        )?;
        Ok(LoadSoakMetricPlan {
            event_count_metric_name: crate::planning_support::metric_name(
                41,
                0,
                "prodex_load_soak_events_total",
            ),
            duration_metric_name: crate::planning_support::metric_name(
                41,
                1,
                "prodex_load_soak_duration_ms",
            ),
            increment: 1,
            duration_ms,
            scenario_label,
            result_label,
        })
    }

    pub fn plan_fault_injection_metric(
        target: FaultInjectionTarget,
        result: FaultInjectionResult,
    ) -> Result<FaultInjectionMetricPlan, TelemetryAttributeError> {
        let target_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(71, "fault_injection_target"),
            fault_injection_target_label(target),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(70, "fault_injection_result"),
            fault_injection_result_label(result),
        )?;
        Ok(FaultInjectionMetricPlan {
            metric_name: crate::planning_support::metric_name(
                39,
                0,
                "prodex_fault_injection_events_total",
            ),
            increment: 1,
            target_label,
            result_label,
        })
    }

    pub fn plan_migration_lifecycle_metric(
        operation: MigrationLifecycleOperation,
        result: MigrationLifecycleResult,
    ) -> Result<MigrationLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(88, "migration_operation"),
            migration_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(89, "migration_result"),
            migration_lifecycle_result_label(result),
        )?;
        Ok(MigrationLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                42,
                0,
                "prodex_migration_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_persistence_metric(
        operation: PersistenceOperation,
        result: PersistenceResult,
    ) -> Result<PersistenceMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(92, "persistence_operation"),
            persistence_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(93, "persistence_result"),
            persistence_result_label(result),
        )?;
        Ok(PersistenceMetricPlan {
            metric_name: crate::planning_support::metric_name(
                43,
                0,
                "prodex_persistence_operations_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    #[cfg(not(feature = "mojo"))]
    fn shutdown_lifecycle_event_label(event: ShutdownLifecycleEvent) -> String {
        {
            (match event {
                ShutdownLifecycleEvent::SignalReceived => "signal_received",
                ShutdownLifecycleEvent::DrainingStarted => "draining_started",
                ShutdownLifecycleEvent::ReadinessDisabled => "readiness_disabled",
                ShutdownLifecycleEvent::InflightDrained => "inflight_drained",
                ShutdownLifecycleEvent::TimeoutElapsed => "timeout_elapsed",
                ShutdownLifecycleEvent::Completed => "completed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn shutdown_lifecycle_result_label(result: ShutdownLifecycleResult) -> String {
        {
            (match result {
                ShutdownLifecycleResult::Success => "success",
                ShutdownLifecycleResult::Timeout => "timeout",
                ShutdownLifecycleResult::Forced => "forced",
                ShutdownLifecycleResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn health_probe_kind_label(probe: HealthProbeKind) -> String {
        {
            (match probe {
                HealthProbeKind::Live => "live",
                HealthProbeKind::Ready => "ready",
                HealthProbeKind::Startup => "startup",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn health_probe_result_label(result: HealthProbeResult) -> String {
        {
            (match result {
                HealthProbeResult::Passing => "passing",
                HealthProbeResult::Degraded => "degraded",
                HealthProbeResult::Failing => "failing",
                HealthProbeResult::Draining => "draining",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn secret_provider_backend_label(backend: SecretProviderBackend) -> String {
        {
            (match backend {
                SecretProviderBackend::File => "file",
                SecretProviderBackend::Keyring => "keyring",
                SecretProviderBackend::ExternalManager => "external_manager",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn secret_provider_operation_label(operation: SecretProviderOperation) -> String {
        {
            (match operation {
                SecretProviderOperation::Read => "read",
                SecretProviderOperation::Write => "write",
                SecretProviderOperation::Delete => "delete",
                SecretProviderOperation::RevisionLookup => "revision_lookup",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn secret_provider_result_label(result: SecretProviderResult) -> String {
        {
            (match result {
                SecretProviderResult::Success => "success",
                SecretProviderResult::NotFound => "not_found",
                SecretProviderResult::Unsupported => "unsupported",
                SecretProviderResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn secret_rotation_scope_label(scope: SecretRotationScope) -> String {
        {
            (match scope {
                SecretRotationScope::ProviderCredential => "provider_credential",
                SecretRotationScope::OidcClient => "oidc_client",
                SecretRotationScope::SigningKey => "signing_key",
                SecretRotationScope::StorageCredential => "storage_credential",
                SecretRotationScope::WebhookSecret => "webhook_secret",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn secret_rotation_result_label(result: SecretRotationResult) -> String {
        {
            (match result {
                SecretRotationResult::Success => "success",
                SecretRotationResult::Failed => "failed",
                SecretRotationResult::Skipped => "skipped",
                SecretRotationResult::Rollback => "rollback",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn backup_restore_operation_label(operation: BackupRestoreOperation) -> String {
        {
            (match operation {
                BackupRestoreOperation::Backup => "backup",
                BackupRestoreOperation::Restore => "restore",
                BackupRestoreOperation::Verify => "verify",
                BackupRestoreOperation::Drill => "drill",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn backup_restore_result_label(result: BackupRestoreResult) -> String {
        {
            (match result {
                BackupRestoreResult::Success => "success",
                BackupRestoreResult::Failed => "failed",
                BackupRestoreResult::Partial => "partial",
                BackupRestoreResult::Skipped => "skipped",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn deployment_rollout_operation_label(operation: DeploymentRolloutOperation) -> String {
        {
            (match operation {
                DeploymentRolloutOperation::Apply => "apply",
                DeploymentRolloutOperation::Verify => "verify",
                DeploymentRolloutOperation::Promote => "promote",
                DeploymentRolloutOperation::Rollback => "rollback",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn deployment_rollout_result_label(result: DeploymentRolloutResult) -> String {
        {
            (match result {
                DeploymentRolloutResult::Success => "success",
                DeploymentRolloutResult::Failed => "failed",
                DeploymentRolloutResult::Degraded => "degraded",
                DeploymentRolloutResult::Skipped => "skipped",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn load_soak_scenario_label(scenario: LoadSoakScenarioKind) -> String {
        {
            (match scenario {
                LoadSoakScenarioKind::Load => "load",
                LoadSoakScenarioKind::Soak => "soak",
                LoadSoakScenarioKind::Spike => "spike",
                LoadSoakScenarioKind::Recovery => "recovery",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn load_soak_result_label(result: LoadSoakResult) -> String {
        {
            (match result {
                LoadSoakResult::Passed => "passed",
                LoadSoakResult::Failed => "failed",
                LoadSoakResult::Aborted => "aborted",
                LoadSoakResult::ThresholdBreached => "threshold_breached",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn fault_injection_target_label(target: FaultInjectionTarget) -> String {
        {
            (match target {
                FaultInjectionTarget::Postgres => "postgres",
                FaultInjectionTarget::Redis => "redis",
                FaultInjectionTarget::Idp => "idp",
                FaultInjectionTarget::Provider => "provider",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn fault_injection_result_label(result: FaultInjectionResult) -> String {
        {
            (match result {
                FaultInjectionResult::Injected => "injected",
                FaultInjectionResult::Recovered => "recovered",
                FaultInjectionResult::Failed => "failed",
                FaultInjectionResult::Skipped => "skipped",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn migration_lifecycle_operation_label(operation: MigrationLifecycleOperation) -> String {
        {
            (match operation {
                MigrationLifecycleOperation::StatusCheck => "status_check",
                MigrationLifecycleOperation::CompatibilityCheck => "compatibility_check",
                MigrationLifecycleOperation::Apply => "apply",
                MigrationLifecycleOperation::Rollback => "rollback",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn migration_lifecycle_result_label(result: MigrationLifecycleResult) -> String {
        {
            (match result {
                MigrationLifecycleResult::Compatible => "compatible",
                MigrationLifecycleResult::Applied => "applied",
                MigrationLifecycleResult::Blocked => "blocked",
                MigrationLifecycleResult::Failed => "failed",
                MigrationLifecycleResult::RolledBack => "rolled_back",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn persistence_operation_label(operation: PersistenceOperation) -> String {
        {
            (match operation {
                PersistenceOperation::Read => "read",
                PersistenceOperation::Write => "write",
                PersistenceOperation::Commit => "commit",
                PersistenceOperation::Rollback => "rollback",
                PersistenceOperation::HealthCheck => "health_check",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn persistence_result_label(result: PersistenceResult) -> String {
        {
            (match result {
                PersistenceResult::Success => "success",
                PersistenceResult::Conflict => "conflict",
                PersistenceResult::Timeout => "timeout",
                PersistenceResult::Unavailable => "unavailable",
                PersistenceResult::Failed => "failed",
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
    pub fn plan_secret_provider_metric(
        backend: SecretProviderBackend,
        operation: SecretProviderOperation,
        result: SecretProviderResult,
    ) -> Result<SecretProviderMetricPlan, TelemetryAttributeError> {
        Ok(SecretProviderMetricPlan {
            metric_name: crate::planning_support::metric_name(44, 0, ""),
            increment: 1,
            backend_label: crate::planning_support::planned_metric_label(44, 0, backend as i64)?,
            operation_label: crate::planning_support::planned_metric_label(
                44,
                1,
                operation as i64,
            )?,
            result_label: crate::planning_support::planned_metric_label(44, 2, result as i64)?,
        })
    }
    pub fn plan_load_soak_metric(
        scenario: LoadSoakScenarioKind,
        result: LoadSoakResult,
        duration_ms: u64,
    ) -> Result<LoadSoakMetricPlan, TelemetryAttributeError> {
        Ok(LoadSoakMetricPlan {
            event_count_metric_name: crate::planning_support::metric_name(41, 0, ""),
            duration_metric_name: crate::planning_support::metric_name(41, 1, ""),
            increment: 1,
            duration_ms,
            scenario_label: crate::planning_support::planned_metric_label(41, 0, scenario as i64)?,
            result_label: crate::planning_support::planned_metric_label(41, 1, result as i64)?,
        })
    }
    two_label_plan!(plan_shutdown_lifecycle_metric, ShutdownLifecycleMetricPlan, 46, event: ShutdownLifecycleEvent => event_label, result: ShutdownLifecycleResult => result_label);
    two_label_plan!(plan_health_probe_metric, HealthProbeMetricPlan, 40, probe: HealthProbeKind => probe_label, result: HealthProbeResult => result_label);
    two_label_plan!(plan_secret_rotation_metric, SecretRotationMetricPlan, 45, scope: SecretRotationScope => scope_label, result: SecretRotationResult => result_label);
    two_label_plan!(plan_backup_restore_metric, BackupRestoreMetricPlan, 37, operation: BackupRestoreOperation => operation_label, result: BackupRestoreResult => result_label);
    two_label_plan!(plan_deployment_rollout_metric, DeploymentRolloutMetricPlan, 38, operation: DeploymentRolloutOperation => operation_label, result: DeploymentRolloutResult => result_label);
    two_label_plan!(plan_fault_injection_metric, FaultInjectionMetricPlan, 39, target: FaultInjectionTarget => target_label, result: FaultInjectionResult => result_label);
    two_label_plan!(plan_migration_lifecycle_metric, MigrationLifecycleMetricPlan, 42, operation: MigrationLifecycleOperation => operation_label, result: MigrationLifecycleResult => result_label);
    two_label_plan!(plan_persistence_metric, PersistenceMetricPlan, 43, operation: PersistenceOperation => operation_label, result: PersistenceResult => result_label);
}
#[cfg(feature = "mojo")]
pub use mojo_impl::*;
