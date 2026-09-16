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

pub fn plan_shutdown_lifecycle_metric(
    event: ShutdownLifecycleEvent,
    result: ShutdownLifecycleResult,
) -> Result<ShutdownLifecycleMetricPlan, TelemetryAttributeError> {
    let event_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(136)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "shutdown_event"
            }
        },
        shutdown_lifecycle_event_label(event),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(137)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "shutdown_result"
            }
        },
        shutdown_lifecycle_result_label(result),
    );
    event_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ShutdownLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(46, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_shutdown_lifecycle_total"
            }
        },
        increment: 1,
        event_label,
        result_label,
    })
}

pub fn plan_health_probe_metric(
    probe: HealthProbeKind,
    result: HealthProbeResult,
) -> Result<HealthProbeMetricPlan, TelemetryAttributeError> {
    let probe_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(72)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "health_probe"
            }
        },
        health_probe_kind_label(probe),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(73)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "health_result"
            }
        },
        health_probe_result_label(result),
    );
    probe_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(HealthProbeMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(40, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_health_probe_results_total"
            }
        },
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
    let backend_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(127)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "secret_backend"
            }
        },
        secret_provider_backend_label(backend),
    );
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(128)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "secret_operation"
            }
        },
        secret_provider_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(129)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "secret_result"
            }
        },
        secret_provider_result_label(result),
    );
    backend_label.as_metric_label()?;
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecretProviderMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(44, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_secret_provider_operations_total"
            }
        },
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
    let scope_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(131)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "secret_scope"
            }
        },
        secret_rotation_scope_label(scope),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(130)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "secret_rotation_result"
            }
        },
        secret_rotation_result_label(result),
    );
    scope_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecretRotationMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(45, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_secret_rotation_events_total"
            }
        },
        increment: 1,
        scope_label,
        result_label,
    })
}

pub fn plan_backup_restore_metric(
    operation: BackupRestoreOperation,
    result: BackupRestoreResult,
) -> Result<BackupRestoreMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(47)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "backup_restore_operation"
            }
        },
        backup_restore_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(48)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "backup_restore_result"
            }
        },
        backup_restore_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BackupRestoreMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(37, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_backup_restore_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_deployment_rollout_metric(
    operation: DeploymentRolloutOperation,
    result: DeploymentRolloutResult,
) -> Result<DeploymentRolloutMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(66)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "deployment_rollout_operation"
            }
        },
        deployment_rollout_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(67)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "deployment_rollout_result"
            }
        },
        deployment_rollout_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(DeploymentRolloutMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(38, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_deployment_rollout_events_total"
            }
        },
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
    let scenario_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(87)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "load_soak_scenario"
            }
        },
        load_soak_scenario_label(scenario),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(86)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "load_soak_result"
            }
        },
        load_soak_result_label(result),
    );
    scenario_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(LoadSoakMetricPlan {
        event_count_metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(41, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_load_soak_events_total"
            }
        },
        duration_metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(41, 1)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_load_soak_duration_ms"
            }
        },
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
    let target_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(71)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "fault_injection_target"
            }
        },
        fault_injection_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(70)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "fault_injection_result"
            }
        },
        fault_injection_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(FaultInjectionMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(39, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_fault_injection_events_total"
            }
        },
        increment: 1,
        target_label,
        result_label,
    })
}

pub fn plan_migration_lifecycle_metric(
    operation: MigrationLifecycleOperation,
    result: MigrationLifecycleResult,
) -> Result<MigrationLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(88)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "migration_operation"
            }
        },
        migration_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(89)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "migration_result"
            }
        },
        migration_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(MigrationLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(42, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_migration_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_persistence_metric(
    operation: PersistenceOperation,
    result: PersistenceResult,
) -> Result<PersistenceMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(92)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "persistence_operation"
            }
        },
        persistence_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(93)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "persistence_result"
            }
        },
        persistence_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PersistenceMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(43, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_persistence_operations_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

fn shutdown_lifecycle_event_label(event: ShutdownLifecycleEvent) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(87, event as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn shutdown_lifecycle_result_label(result: ShutdownLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(88, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn health_probe_kind_label(probe: HealthProbeKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(74, probe as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match probe {
            HealthProbeKind::Live => "live",
            HealthProbeKind::Ready => "ready",
            HealthProbeKind::Startup => "startup",
        })
        .to_string()
    }
}

fn health_probe_result_label(result: HealthProbeResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(75, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn secret_provider_backend_label(backend: SecretProviderBackend) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(82, backend as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match backend {
            SecretProviderBackend::File => "file",
            SecretProviderBackend::Keyring => "keyring",
            SecretProviderBackend::ExternalManager => "external_manager",
        })
        .to_string()
    }
}

fn secret_provider_operation_label(operation: SecretProviderOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(83, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn secret_provider_result_label(result: SecretProviderResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(84, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn secret_rotation_scope_label(scope: SecretRotationScope) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(86, scope as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn secret_rotation_result_label(result: SecretRotationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(85, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn backup_restore_operation_label(operation: BackupRestoreOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(68, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn backup_restore_result_label(result: BackupRestoreResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(69, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn deployment_rollout_operation_label(operation: DeploymentRolloutOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(70, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn deployment_rollout_result_label(result: DeploymentRolloutResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(71, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn load_soak_scenario_label(scenario: LoadSoakScenarioKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(77, scenario as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn load_soak_result_label(result: LoadSoakResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(76, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn fault_injection_target_label(target: FaultInjectionTarget) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(73, target as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn fault_injection_result_label(result: FaultInjectionResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(72, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn migration_lifecycle_operation_label(operation: MigrationLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(78, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn migration_lifecycle_result_label(result: MigrationLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(79, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn persistence_operation_label(operation: PersistenceOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(80, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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

fn persistence_result_label(result: PersistenceResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(81, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
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
