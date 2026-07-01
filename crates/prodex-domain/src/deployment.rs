use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSecurityPolicy {
    pub non_root_user: bool,
    pub read_only_root_filesystem: bool,
    pub dropped_all_capabilities: bool,
    pub immutable_image_digest: bool,
}

impl fmt::Debug for ContainerSecurityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerSecurityPolicy")
            .field("violation_count", &self.violations().len())
            .finish()
    }
}

impl ContainerSecurityPolicy {
    pub fn hardened() -> Self {
        Self {
            non_root_user: true,
            read_only_root_filesystem: true,
            dropped_all_capabilities: true,
            immutable_image_digest: true,
        }
    }

    pub fn violations(&self) -> Vec<DeploymentViolation> {
        let mut violations = Vec::new();
        if !self.non_root_user {
            violations.push(DeploymentViolation::RunsAsRoot);
        }
        if !self.read_only_root_filesystem {
            violations.push(DeploymentViolation::WritableRootFilesystem);
        }
        if !self.dropped_all_capabilities {
            violations.push(DeploymentViolation::LinuxCapabilitiesNotDropped);
        }
        if !self.immutable_image_digest {
            violations.push(DeploymentViolation::MutableImageReference);
        }
        violations
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KubernetesArtifactSet {
    pub deployment: bool,
    pub service: bool,
    pub config_map: bool,
    pub external_secret: bool,
    pub pod_disruption_budget: bool,
    pub horizontal_pod_autoscaler: bool,
    pub network_policy: bool,
    pub service_monitor_or_otel_collector: bool,
    pub migration_job: bool,
}

impl fmt::Debug for KubernetesArtifactSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KubernetesArtifactSet")
            .field("violation_count", &self.violations().len())
            .finish()
    }
}

impl KubernetesArtifactSet {
    pub fn complete() -> Self {
        Self {
            deployment: true,
            service: true,
            config_map: true,
            external_secret: true,
            pod_disruption_budget: true,
            horizontal_pod_autoscaler: true,
            network_policy: true,
            service_monitor_or_otel_collector: true,
            migration_job: true,
        }
    }

    pub fn violations(&self) -> Vec<DeploymentViolation> {
        let mut violations = Vec::new();
        if !self.deployment {
            violations.push(DeploymentViolation::MissingDeployment);
        }
        if !self.service {
            violations.push(DeploymentViolation::MissingService);
        }
        if !self.config_map {
            violations.push(DeploymentViolation::MissingConfigMap);
        }
        if !self.external_secret {
            violations.push(DeploymentViolation::MissingExternalSecret);
        }
        if !self.pod_disruption_budget {
            violations.push(DeploymentViolation::MissingPodDisruptionBudget);
        }
        if !self.horizontal_pod_autoscaler {
            violations.push(DeploymentViolation::MissingHorizontalPodAutoscaler);
        }
        if !self.network_policy {
            violations.push(DeploymentViolation::MissingNetworkPolicy);
        }
        if !self.service_monitor_or_otel_collector {
            violations.push(DeploymentViolation::MissingObservabilityArtifact);
        }
        if !self.migration_job {
            violations.push(DeploymentViolation::MissingMigrationJob);
        }
        violations
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentViolation {
    RunsAsRoot,
    WritableRootFilesystem,
    LinuxCapabilitiesNotDropped,
    MutableImageReference,
    MissingDeployment,
    MissingService,
    MissingConfigMap,
    MissingExternalSecret,
    MissingPodDisruptionBudget,
    MissingHorizontalPodAutoscaler,
    MissingNetworkPolicy,
    MissingObservabilityArtifact,
    MissingMigrationJob,
    GatewayReplicaCountTooLow,
    MultiReplicaAccountingGateDisabled,
    MissingSharedPostgres,
    MissingSharedRedis,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentSecurityReport {
    pub violations: Vec<DeploymentViolation>,
}

impl fmt::Debug for DeploymentSecurityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeploymentSecurityReport")
            .field("violation_count", &self.violations.len())
            .finish()
    }
}

impl DeploymentSecurityReport {
    pub fn is_compliant(&self) -> bool {
        self.violations.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentSecurityErrorStatus {
    InvalidDeployment,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentSecurityErrorResponsePlan {
    pub status: DeploymentSecurityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_deployment_security_error_response(
    _report: &DeploymentSecurityReport,
) -> DeploymentSecurityErrorResponsePlan {
    DeploymentSecurityErrorResponsePlan {
        status: DeploymentSecurityErrorStatus::InvalidDeployment,
        code: "deployment_security_validation_failed",
        message: "deployment security validation failed",
    }
}

pub fn evaluate_deployment_security(
    container: &ContainerSecurityPolicy,
    artifacts: &KubernetesArtifactSet,
) -> DeploymentSecurityReport {
    let mut violations = container.violations();
    violations.extend(artifacts.violations());
    DeploymentSecurityReport { violations }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionReadinessTopology {
    pub gateway_replica_count: u16,
    pub require_multi_replica_accounting_checks: bool,
    pub shared_postgres_configured: bool,
    pub shared_redis_configured: bool,
}

impl fmt::Debug for ProductionReadinessTopology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionReadinessTopology")
            .field("gateway_replica_count", &"<redacted>")
            .field(
                "multi_replica_accounting_gate_configured",
                &self.require_multi_replica_accounting_checks,
            )
            .field("shared_postgres_present", &self.shared_postgres_configured)
            .field("shared_redis_present", &self.shared_redis_configured)
            .finish()
    }
}

impl ProductionReadinessTopology {
    pub fn multi_replica_postgres_redis(gateway_replica_count: u16) -> Self {
        Self {
            gateway_replica_count,
            require_multi_replica_accounting_checks: true,
            shared_postgres_configured: true,
            shared_redis_configured: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentReadinessPlan {
    pub gateway_replica_count: u16,
    pub accounting_gate_required: bool,
}

impl fmt::Debug for DeploymentReadinessPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeploymentReadinessPlan")
            .field("gateway_replica_count", &"<redacted>")
            .field("accounting_gate_required", &self.accounting_gate_required)
            .finish()
    }
}

pub fn plan_production_deployment_readiness(
    topology: &ProductionReadinessTopology,
) -> Result<DeploymentReadinessPlan, DeploymentSecurityReport> {
    let mut violations = Vec::new();
    if topology.gateway_replica_count < 2 {
        violations.push(DeploymentViolation::GatewayReplicaCountTooLow);
    }
    if !topology.require_multi_replica_accounting_checks {
        violations.push(DeploymentViolation::MultiReplicaAccountingGateDisabled);
    }
    if !topology.shared_postgres_configured {
        violations.push(DeploymentViolation::MissingSharedPostgres);
    }
    if !topology.shared_redis_configured {
        violations.push(DeploymentViolation::MissingSharedRedis);
    }

    if violations.is_empty() {
        Ok(DeploymentReadinessPlan {
            gateway_replica_count: topology.gateway_replica_count,
            accounting_gate_required: topology.require_multi_replica_accounting_checks,
        })
    } else {
        Err(DeploymentSecurityReport { violations })
    }
}
