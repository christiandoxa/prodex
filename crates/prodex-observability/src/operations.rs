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
    let event_label =
        TelemetryAttribute::metric_label("shutdown_event", shutdown_lifecycle_event_label(event));
    let result_label = TelemetryAttribute::metric_label(
        "shutdown_result",
        shutdown_lifecycle_result_label(result),
    );
    event_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ShutdownLifecycleMetricPlan {
        metric_name: "prodex_shutdown_lifecycle_total",
        increment: 1,
        event_label,
        result_label,
    })
}

pub fn plan_health_probe_metric(
    probe: HealthProbeKind,
    result: HealthProbeResult,
) -> Result<HealthProbeMetricPlan, TelemetryAttributeError> {
    let probe_label =
        TelemetryAttribute::metric_label("health_probe", health_probe_kind_label(probe));
    let result_label =
        TelemetryAttribute::metric_label("health_result", health_probe_result_label(result));
    probe_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(HealthProbeMetricPlan {
        metric_name: "prodex_health_probe_results_total",
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
    let backend_label =
        TelemetryAttribute::metric_label("secret_backend", secret_provider_backend_label(backend));
    let operation_label = TelemetryAttribute::metric_label(
        "secret_operation",
        secret_provider_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("secret_result", secret_provider_result_label(result));
    backend_label.as_metric_label()?;
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecretProviderMetricPlan {
        metric_name: "prodex_secret_provider_operations_total",
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
    let scope_label =
        TelemetryAttribute::metric_label("secret_scope", secret_rotation_scope_label(scope));
    let result_label = TelemetryAttribute::metric_label(
        "secret_rotation_result",
        secret_rotation_result_label(result),
    );
    scope_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecretRotationMetricPlan {
        metric_name: "prodex_secret_rotation_events_total",
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
        "backup_restore_operation",
        backup_restore_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "backup_restore_result",
        backup_restore_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BackupRestoreMetricPlan {
        metric_name: "prodex_backup_restore_events_total",
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
        "deployment_rollout_operation",
        deployment_rollout_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "deployment_rollout_result",
        deployment_rollout_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(DeploymentRolloutMetricPlan {
        metric_name: "prodex_deployment_rollout_events_total",
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
    let scenario_label =
        TelemetryAttribute::metric_label("load_soak_scenario", load_soak_scenario_label(scenario));
    let result_label =
        TelemetryAttribute::metric_label("load_soak_result", load_soak_result_label(result));
    scenario_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(LoadSoakMetricPlan {
        event_count_metric_name: "prodex_load_soak_events_total",
        duration_metric_name: "prodex_load_soak_duration_ms",
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
        "fault_injection_target",
        fault_injection_target_label(target),
    );
    let result_label = TelemetryAttribute::metric_label(
        "fault_injection_result",
        fault_injection_result_label(result),
    );
    target_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(FaultInjectionMetricPlan {
        metric_name: "prodex_fault_injection_events_total",
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
        "migration_operation",
        migration_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "migration_result",
        migration_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(MigrationLifecycleMetricPlan {
        metric_name: "prodex_migration_lifecycle_events_total",
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
        "persistence_operation",
        persistence_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("persistence_result", persistence_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PersistenceMetricPlan {
        metric_name: "prodex_persistence_operations_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

fn shutdown_lifecycle_event_label(event: ShutdownLifecycleEvent) -> &'static str {
    match event {
        ShutdownLifecycleEvent::SignalReceived => "signal_received",
        ShutdownLifecycleEvent::DrainingStarted => "draining_started",
        ShutdownLifecycleEvent::ReadinessDisabled => "readiness_disabled",
        ShutdownLifecycleEvent::InflightDrained => "inflight_drained",
        ShutdownLifecycleEvent::TimeoutElapsed => "timeout_elapsed",
        ShutdownLifecycleEvent::Completed => "completed",
    }
}

fn shutdown_lifecycle_result_label(result: ShutdownLifecycleResult) -> &'static str {
    match result {
        ShutdownLifecycleResult::Success => "success",
        ShutdownLifecycleResult::Timeout => "timeout",
        ShutdownLifecycleResult::Forced => "forced",
        ShutdownLifecycleResult::Failed => "failed",
    }
}

fn health_probe_kind_label(probe: HealthProbeKind) -> &'static str {
    match probe {
        HealthProbeKind::Live => "live",
        HealthProbeKind::Ready => "ready",
        HealthProbeKind::Startup => "startup",
    }
}

fn health_probe_result_label(result: HealthProbeResult) -> &'static str {
    match result {
        HealthProbeResult::Passing => "passing",
        HealthProbeResult::Degraded => "degraded",
        HealthProbeResult::Failing => "failing",
        HealthProbeResult::Draining => "draining",
    }
}

fn secret_provider_backend_label(backend: SecretProviderBackend) -> &'static str {
    match backend {
        SecretProviderBackend::File => "file",
        SecretProviderBackend::Keyring => "keyring",
        SecretProviderBackend::ExternalManager => "external_manager",
    }
}

fn secret_provider_operation_label(operation: SecretProviderOperation) -> &'static str {
    match operation {
        SecretProviderOperation::Read => "read",
        SecretProviderOperation::Write => "write",
        SecretProviderOperation::Delete => "delete",
        SecretProviderOperation::RevisionLookup => "revision_lookup",
    }
}

fn secret_provider_result_label(result: SecretProviderResult) -> &'static str {
    match result {
        SecretProviderResult::Success => "success",
        SecretProviderResult::NotFound => "not_found",
        SecretProviderResult::Unsupported => "unsupported",
        SecretProviderResult::Failed => "failed",
    }
}

fn secret_rotation_scope_label(scope: SecretRotationScope) -> &'static str {
    match scope {
        SecretRotationScope::ProviderCredential => "provider_credential",
        SecretRotationScope::OidcClient => "oidc_client",
        SecretRotationScope::SigningKey => "signing_key",
        SecretRotationScope::StorageCredential => "storage_credential",
        SecretRotationScope::WebhookSecret => "webhook_secret",
    }
}

fn secret_rotation_result_label(result: SecretRotationResult) -> &'static str {
    match result {
        SecretRotationResult::Success => "success",
        SecretRotationResult::Failed => "failed",
        SecretRotationResult::Skipped => "skipped",
        SecretRotationResult::Rollback => "rollback",
    }
}

fn backup_restore_operation_label(operation: BackupRestoreOperation) -> &'static str {
    match operation {
        BackupRestoreOperation::Backup => "backup",
        BackupRestoreOperation::Restore => "restore",
        BackupRestoreOperation::Verify => "verify",
        BackupRestoreOperation::Drill => "drill",
    }
}

fn backup_restore_result_label(result: BackupRestoreResult) -> &'static str {
    match result {
        BackupRestoreResult::Success => "success",
        BackupRestoreResult::Failed => "failed",
        BackupRestoreResult::Partial => "partial",
        BackupRestoreResult::Skipped => "skipped",
    }
}

fn deployment_rollout_operation_label(operation: DeploymentRolloutOperation) -> &'static str {
    match operation {
        DeploymentRolloutOperation::Apply => "apply",
        DeploymentRolloutOperation::Verify => "verify",
        DeploymentRolloutOperation::Promote => "promote",
        DeploymentRolloutOperation::Rollback => "rollback",
    }
}

fn deployment_rollout_result_label(result: DeploymentRolloutResult) -> &'static str {
    match result {
        DeploymentRolloutResult::Success => "success",
        DeploymentRolloutResult::Failed => "failed",
        DeploymentRolloutResult::Degraded => "degraded",
        DeploymentRolloutResult::Skipped => "skipped",
    }
}

fn load_soak_scenario_label(scenario: LoadSoakScenarioKind) -> &'static str {
    match scenario {
        LoadSoakScenarioKind::Load => "load",
        LoadSoakScenarioKind::Soak => "soak",
        LoadSoakScenarioKind::Spike => "spike",
        LoadSoakScenarioKind::Recovery => "recovery",
    }
}

fn load_soak_result_label(result: LoadSoakResult) -> &'static str {
    match result {
        LoadSoakResult::Passed => "passed",
        LoadSoakResult::Failed => "failed",
        LoadSoakResult::Aborted => "aborted",
        LoadSoakResult::ThresholdBreached => "threshold_breached",
    }
}

fn fault_injection_target_label(target: FaultInjectionTarget) -> &'static str {
    match target {
        FaultInjectionTarget::Postgres => "postgres",
        FaultInjectionTarget::Redis => "redis",
        FaultInjectionTarget::Idp => "idp",
        FaultInjectionTarget::Provider => "provider",
    }
}

fn fault_injection_result_label(result: FaultInjectionResult) -> &'static str {
    match result {
        FaultInjectionResult::Injected => "injected",
        FaultInjectionResult::Recovered => "recovered",
        FaultInjectionResult::Failed => "failed",
        FaultInjectionResult::Skipped => "skipped",
    }
}

fn migration_lifecycle_operation_label(operation: MigrationLifecycleOperation) -> &'static str {
    match operation {
        MigrationLifecycleOperation::StatusCheck => "status_check",
        MigrationLifecycleOperation::CompatibilityCheck => "compatibility_check",
        MigrationLifecycleOperation::Apply => "apply",
        MigrationLifecycleOperation::Rollback => "rollback",
    }
}

fn migration_lifecycle_result_label(result: MigrationLifecycleResult) -> &'static str {
    match result {
        MigrationLifecycleResult::Compatible => "compatible",
        MigrationLifecycleResult::Applied => "applied",
        MigrationLifecycleResult::Blocked => "blocked",
        MigrationLifecycleResult::Failed => "failed",
        MigrationLifecycleResult::RolledBack => "rolled_back",
    }
}

fn persistence_operation_label(operation: PersistenceOperation) -> &'static str {
    match operation {
        PersistenceOperation::Read => "read",
        PersistenceOperation::Write => "write",
        PersistenceOperation::Commit => "commit",
        PersistenceOperation::Rollback => "rollback",
        PersistenceOperation::HealthCheck => "health_check",
    }
}

fn persistence_result_label(result: PersistenceResult) -> &'static str {
    match result {
        PersistenceResult::Success => "success",
        PersistenceResult::Conflict => "conflict",
        PersistenceResult::Timeout => "timeout",
        PersistenceResult::Unavailable => "unavailable",
        PersistenceResult::Failed => "failed",
    }
}
