use super::*;

pub(super) fn bank_artifacts(
    settings: &RuntimePolicyGovernanceSettings,
) -> Vec<(GovernanceArtifactKind, String, Vec<u8>)> {
    #[derive(serde::Serialize)]
    struct ClassificationChecksumInput {
        unsupported_coverage_floor: DataClassification,
        rules: Vec<serde_json::Value>,
    }

    let classification_checksum = Sha256::digest(
        serde_json::to_vec(&ClassificationChecksumInput {
            unsupported_coverage_floor: DataClassification::Restricted,
            rules: Vec::new(),
        })
        .unwrap(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
    let mut policy_settings = settings.clone();
    policy_settings.classification_checksum = Some(classification_checksum.clone());
    let classification = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "detector_revision": "detector-v1",
        "patterns": [],
        "classification_revision": "classification-v1",
        "classification_checksum": classification_checksum,
        "unsupported_coverage_floor": "restricted",
        "classification_rules": []
    }))
    .unwrap();
    let adapter = provider_adapter(ProviderId::OpenAi);
    let endpoints = adapter
        .supported_endpoints()
        .iter()
        .copied()
        .filter(|endpoint| {
            crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_capability_is_executable(
                adapter.capability_status(*endpoint),
            )
        })
        .collect::<Vec<_>>();
    let provider_registry = serde_json::to_vec(&serde_json::json!({
        "schema_version": 2,
        "revision": 1,
        "pricing_revision": 1,
        "descriptors": [{
            "revision": 1,
            "pricing_revision": 1,
            "provider": "openai",
            "credential_ref": SecretRef::new("runtime-provider", "openai", None::<String>),
            "enabled": true,
            "revoked": false,
            "executable": true,
            "endpoints": endpoints,
            "capabilities": crate::runtime_launch::proxy_startup::local_rewrite_application_data_plane::runtime_gateway_provider_executable_capabilities(ProviderId::OpenAi),
            "regions": ["*"],
            "local_execution": true,
            "trust_tier": "restricted_approved",
            "maximum_classification": "restricted",
            "retention_seconds": 0,
            "training_use": false,
            "model_costs": {"*": {
                "input_cost_per_million_microusd": 1_000_000,
                "output_cost_per_million_microusd": 2_000_000
            }},
            "cost": 2_000,
            "latency": 3_000,
            "risk": 1_000,
            "priority": 8_000
        }]
    }))
    .unwrap();
    let routing = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "revision": 1,
        "weights": {
            "health": 2_000,
            "load": 1_000,
            "cost": 3_000,
            "latency": 1_000,
            "risk": 1_000,
            "priority": 1_000,
            "affinity": 1_000
        }
    }))
    .unwrap();

    vec![
        (
            GovernanceArtifactKind::Policy,
            settings.policy_revision.unwrap().to_string(),
            serde_json::to_vec(&policy_settings).unwrap(),
        ),
        (
            GovernanceArtifactKind::ClassificationRules,
            "classification-v1".to_string(),
            classification,
        ),
        (
            GovernanceArtifactKind::ProviderRegistry,
            "1".to_string(),
            provider_registry,
        ),
        (
            GovernanceArtifactKind::RoutingScores,
            "1".to_string(),
            routing,
        ),
    ]
}

pub(super) fn seed_authority(
    database_path: &Path,
    tenant_id: TenantId,
    signing_key: &Ed25519KeyPair,
    artifacts: &[(GovernanceArtifactKind, String, Vec<u8>)],
) {
    let connection = Connection::open(database_path).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_tenants
             (tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'test tenant', 1, 1)",
            [tenant_id.to_string()],
        )
        .unwrap();
    for (kind, revision, artifact) in artifacts {
        seed_revision(
            &connection,
            tenant_id,
            signing_key,
            *kind,
            revision,
            artifact,
        );
        connection
            .execute(
                &format!(
                    "INSERT INTO {} (tenant_id, active_revision_id, last_known_good_revision_id,
                     etag, updated_at_unix_ms) VALUES (?1, ?2, ?2, 'etag-1', 1)",
                    pointer_table(*kind),
                ),
                params![tenant_id.to_string(), revision],
            )
            .unwrap();
    }
}

pub(super) fn seed_mismatched_active_revision(
    database_path: &Path,
    tenant_id: TenantId,
    signing_key: &Ed25519KeyPair,
    artifacts: &[(GovernanceArtifactKind, String, Vec<u8>)],
    kind: GovernanceArtifactKind,
) {
    let (_, lkg_revision, artifact) = artifacts
        .iter()
        .find(|(candidate, _, _)| *candidate == kind)
        .unwrap();
    let active_revision = match kind {
        GovernanceArtifactKind::Policy => prodex_domain::PolicyRevisionId::new().to_string(),
        GovernanceArtifactKind::ClassificationRules => "classification-v2".to_string(),
        GovernanceArtifactKind::ProviderRegistry | GovernanceArtifactKind::RoutingScores => {
            "2".to_string()
        }
    };
    let mismatched_artifact = if kind == GovernanceArtifactKind::Policy {
        let mut settings =
            serde_json::from_slice::<RuntimePolicyGovernanceSettings>(artifact).unwrap();
        let revision = active_revision.parse().unwrap();
        settings.policy_revision = Some(revision);
        settings.active_policy_revision = Some(revision);
        settings.classification_revision = Some("classification-v2".to_string());
        settings.classification_checksum = Some("classification-v2".to_string());
        settings.provider_registry_revision = Some(2);
        settings.routing_score_revision = Some(2);
        serde_json::to_vec(&settings).unwrap()
    } else {
        let mut artifact = artifact.clone();
        artifact.push(b' ');
        artifact
    };
    let connection = Connection::open(database_path).unwrap();
    connection
        .execute(
            &format!(
                "UPDATE {} SET lifecycle_state = 'superseded' \
                 WHERE tenant_id = ?1 AND revision_id = ?2",
                revision_table(kind),
            ),
            params![tenant_id.to_string(), lkg_revision],
        )
        .unwrap();
    seed_revision(
        &connection,
        tenant_id,
        signing_key,
        kind,
        &active_revision,
        &mismatched_artifact,
    );
    connection
        .execute(
            &format!(
                "UPDATE {} SET active_revision_id = ?2, last_known_good_revision_id = ?3, \
                 etag = 'etag-mismatch', updated_at_unix_ms = 2 WHERE tenant_id = ?1",
                pointer_table(kind),
            ),
            params![tenant_id.to_string(), active_revision, lkg_revision],
        )
        .unwrap();
}

fn seed_revision(
    connection: &Connection,
    tenant_id: TenantId,
    signing_key: &Ed25519KeyPair,
    kind: GovernanceArtifactKind,
    revision: &str,
    artifact: &[u8],
) {
    let checksum = governance_support::artifact_checksum(artifact);
    let created_by = PrincipalId::new().to_string();
    let signature = signing_key.sign(&governance_support::artifact_signature_message(
        tenant_id, kind, revision, artifact,
    ));
    connection
        .execute(
            "INSERT INTO prodex_governance_revision_artifacts
             (tenant_id, artifact_kind, revision_id, artifact_checksum, compiled_artifact,
              created_by, created_at_unix_ms, signature_key_id, artifact_signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 'test-authority', ?7)",
            params![
                tenant_id.to_string(),
                artifact_kind_label(kind),
                revision,
                checksum,
                artifact,
                created_by,
                STANDARD.encode(signature.as_ref()),
            ],
        )
        .unwrap();
    match kind {
        GovernanceArtifactKind::Policy => connection.execute(
            "INSERT INTO prodex_policy_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_by, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'active', ?4, 1)",
            params![tenant_id.to_string(), revision, checksum, created_by],
        ),
        GovernanceArtifactKind::ClassificationRules => connection.execute(
            "INSERT INTO prodex_classification_rule_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'active', 1)",
            params![tenant_id.to_string(), revision, checksum],
        ),
        GovernanceArtifactKind::ProviderRegistry => connection.execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, 'active', 1)",
            params![tenant_id.to_string(), revision, checksum],
        ),
        GovernanceArtifactKind::RoutingScores => connection.execute(
            "INSERT INTO prodex_routing_score_revisions (
                tenant_id, revision_id, artifact_checksum, fixed_point_weights,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'active', 1)",
            params![tenant_id.to_string(), revision, checksum],
        ),
    }
    .unwrap();
}

fn artifact_kind_label(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "policy",
        GovernanceArtifactKind::ClassificationRules => "classification_rules",
        GovernanceArtifactKind::ProviderRegistry => "provider_registry",
        GovernanceArtifactKind::RoutingScores => "routing_scores",
    }
}

fn pointer_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_pointers",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_pointers",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_pointers",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_pointers",
    }
}

fn revision_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_revisions",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_revisions",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_revisions",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_revisions",
    }
}
