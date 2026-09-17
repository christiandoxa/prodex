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
    #[cfg(feature = "mojo")]
    let event_label = crate::planning_support::planned_metric_label(46, 0, (event) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let event_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(136, "shutdown_event"),
        shutdown_lifecycle_event_label(event),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(46, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(137, "shutdown_result"),
        shutdown_lifecycle_result_label(result),
    )?;
    Ok(ShutdownLifecycleMetricPlan {
        metric_name: crate::planning_support::metric_name(46, 0, "prodex_shutdown_lifecycle_total"),
        increment: 1,
        event_label,
        result_label,
    })
}

pub fn plan_health_probe_metric(
    probe: HealthProbeKind,
    result: HealthProbeResult,
) -> Result<HealthProbeMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let probe_label = crate::planning_support::planned_metric_label(40, 0, (probe) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let probe_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(72, "health_probe"),
        health_probe_kind_label(probe),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(40, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let backend_label = crate::planning_support::planned_metric_label(44, 0, (backend) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let backend_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(127, "secret_backend"),
        secret_provider_backend_label(backend),
    )?;
    #[cfg(feature = "mojo")]
    let operation_label = crate::planning_support::planned_metric_label(44, 1, (operation) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(128, "secret_operation"),
        secret_provider_operation_label(operation),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(44, 2, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let scope_label = crate::planning_support::planned_metric_label(45, 0, (scope) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let scope_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(131, "secret_scope"),
        secret_rotation_scope_label(scope),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(45, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let operation_label = crate::planning_support::planned_metric_label(37, 0, (operation) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(47, "backup_restore_operation"),
        backup_restore_operation_label(operation),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(37, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let operation_label = crate::planning_support::planned_metric_label(38, 0, (operation) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(66, "deployment_rollout_operation"),
        deployment_rollout_operation_label(operation),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(38, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let scenario_label = crate::planning_support::planned_metric_label(41, 0, (scenario) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let scenario_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(87, "load_soak_scenario"),
        load_soak_scenario_label(scenario),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(41, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let target_label = crate::planning_support::planned_metric_label(39, 0, (target) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let target_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(71, "fault_injection_target"),
        fault_injection_target_label(target),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(39, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let operation_label = crate::planning_support::planned_metric_label(42, 0, (operation) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(88, "migration_operation"),
        migration_lifecycle_operation_label(operation),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(42, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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
    #[cfg(feature = "mojo")]
    let operation_label = crate::planning_support::planned_metric_label(43, 0, (operation) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(92, "persistence_operation"),
        persistence_operation_label(operation),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(43, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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
