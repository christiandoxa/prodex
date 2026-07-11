use prodex_domain::{
    ContainerSecurityPolicy, DeploymentSecurityErrorStatus, DeploymentViolation,
    KubernetesArtifactSet, ProductionReadinessTopology, evaluate_deployment_security,
    plan_deployment_security_error_response, plan_production_deployment_readiness,
};

#[test]
fn hardened_container_and_complete_artifacts_are_compliant() {
    let report = evaluate_deployment_security(
        &ContainerSecurityPolicy::hardened(),
        &KubernetesArtifactSet::complete(),
    );

    assert!(report.is_compliant());
}

#[test]
fn container_policy_reports_hardening_violations() {
    let policy = ContainerSecurityPolicy {
        non_root_user: false,
        read_only_root_filesystem: false,
        dropped_all_capabilities: false,
        immutable_image_digest: false,
    };

    assert_eq!(
        policy.violations(),
        vec![
            DeploymentViolation::RunsAsRoot,
            DeploymentViolation::WritableRootFilesystem,
            DeploymentViolation::LinuxCapabilitiesNotDropped,
            DeploymentViolation::MutableImageReference,
        ]
    );
}

#[test]
fn artifact_set_reports_missing_required_kubernetes_objects() {
    let artifacts = KubernetesArtifactSet {
        deployment: false,
        service: true,
        config_map: true,
        external_secret: false,
        pod_disruption_budget: false,
        horizontal_pod_autoscaler: true,
        network_policy: false,
        service_monitor_or_otel_collector: false,
        migration_job: false,
    };

    assert_eq!(
        artifacts.violations(),
        vec![
            DeploymentViolation::MissingDeployment,
            DeploymentViolation::MissingExternalSecret,
            DeploymentViolation::MissingPodDisruptionBudget,
            DeploymentViolation::MissingNetworkPolicy,
            DeploymentViolation::MissingObservabilityArtifact,
            DeploymentViolation::MissingMigrationJob,
        ]
    );
}

#[test]
fn deployment_posture_debug_output_is_stable_and_redacted() {
    let container = ContainerSecurityPolicy {
        non_root_user: false,
        read_only_root_filesystem: false,
        dropped_all_capabilities: true,
        immutable_image_digest: true,
    };
    let artifacts = KubernetesArtifactSet {
        deployment: false,
        service: true,
        config_map: true,
        external_secret: false,
        pod_disruption_budget: true,
        horizontal_pod_autoscaler: true,
        network_policy: true,
        service_monitor_or_otel_collector: true,
        migration_job: true,
    };

    let rendered = format!("{container:?} {artifacts:?}");
    for sensitive in [
        "non_root_user",
        "read_only_root_filesystem",
        "deployment: false",
        "external_secret",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "deployment posture debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("ContainerSecurityPolicy { violation_count: 2 }"));
    assert!(rendered.contains("KubernetesArtifactSet { violation_count: 2 }"));
}

#[test]
fn deployment_security_report_combines_container_and_artifact_violations() {
    let container = ContainerSecurityPolicy {
        non_root_user: false,
        ..ContainerSecurityPolicy::hardened()
    };
    let artifacts = KubernetesArtifactSet {
        network_policy: false,
        ..KubernetesArtifactSet::complete()
    };

    let report = evaluate_deployment_security(&container, &artifacts);

    assert_eq!(
        report.violations,
        vec![
            DeploymentViolation::RunsAsRoot,
            DeploymentViolation::MissingNetworkPolicy
        ]
    );
    assert!(!report.is_compliant());
}

#[test]
fn deployment_security_report_debug_output_is_stable_and_redacted() {
    let report = evaluate_deployment_security(
        &ContainerSecurityPolicy {
            non_root_user: false,
            ..ContainerSecurityPolicy::hardened()
        },
        &KubernetesArtifactSet {
            external_secret: false,
            ..KubernetesArtifactSet::complete()
        },
    );

    let rendered = format!("{report:?}");
    assert!(!rendered.contains("RunsAsRoot"));
    assert!(!rendered.contains("MissingExternalSecret"));
    assert_eq!(rendered, "DeploymentSecurityReport { violation_count: 2 }");
}

#[test]
fn deployment_security_error_response_is_stable_and_redacted() {
    let container = ContainerSecurityPolicy {
        non_root_user: false,
        immutable_image_digest: false,
        ..ContainerSecurityPolicy::hardened()
    };
    let artifacts = KubernetesArtifactSet {
        external_secret: false,
        migration_job: false,
        ..KubernetesArtifactSet::complete()
    };
    let report = evaluate_deployment_security(&container, &artifacts);

    let response = plan_deployment_security_error_response(&report);

    assert_eq!(
        response.status,
        DeploymentSecurityErrorStatus::InvalidDeployment
    );
    assert_eq!(response.code, "deployment_security_validation_failed");
    assert_eq!(response.message, "deployment security validation failed");
    assert!(!response.message.contains("root"));
    assert!(!response.message.contains("image"));
    assert!(!response.message.contains("ExternalSecret"));
    assert!(!response.message.contains("migration"));
}

#[test]
fn production_readiness_requires_multi_replica_accounting_gate_and_shared_stores() {
    let topology = ProductionReadinessTopology::multi_replica_postgres_redis(3);

    let plan = plan_production_deployment_readiness(&topology).unwrap();

    assert_eq!(plan.gateway_replica_count, 3);
    assert!(plan.accounting_gate_required);
}

#[test]
fn production_readiness_topology_debug_output_is_stable_and_redacted() {
    let topology = ProductionReadinessTopology::multi_replica_postgres_redis(3);

    let rendered = format!("{topology:?}");
    assert!(!rendered.contains("gateway_replica_count: 3"));
    assert!(rendered.contains("gateway_replica_count: \"<redacted>\""));
    assert!(rendered.contains("multi_replica_accounting_gate_configured: true"));
    assert!(rendered.contains("shared_postgres_present: true"));
    assert!(rendered.contains("shared_redis_present: true"));
}

#[test]
fn deployment_readiness_plan_debug_output_is_stable_and_redacted() {
    let topology = ProductionReadinessTopology::multi_replica_postgres_redis(3);
    let plan = plan_production_deployment_readiness(&topology).unwrap();

    let rendered = format!("{plan:?}");
    assert!(!rendered.contains("gateway_replica_count: 3"));
    assert!(rendered.contains("gateway_replica_count: \"<redacted>\""));
    assert!(rendered.contains("accounting_gate_required: true"));
}

#[test]
fn production_readiness_rejects_single_replica_or_missing_shared_accounting() {
    let topology = ProductionReadinessTopology {
        gateway_replica_count: 1,
        require_multi_replica_accounting_checks: false,
        shared_postgres_configured: false,
        shared_redis_configured: false,
    };

    let report = plan_production_deployment_readiness(&topology).unwrap_err();

    assert_eq!(
        report.violations,
        vec![
            DeploymentViolation::GatewayReplicaCountTooLow,
            DeploymentViolation::MultiReplicaAccountingGateDisabled,
            DeploymentViolation::MissingSharedPostgres,
            DeploymentViolation::MissingSharedRedis,
        ]
    );
    let response = plan_deployment_security_error_response(&report);
    assert_eq!(response.code, "deployment_security_validation_failed");
    assert!(!response.message.contains("replica"));
    assert!(!response.message.contains("postgres"));
    assert!(!response.message.contains("redis"));
}
