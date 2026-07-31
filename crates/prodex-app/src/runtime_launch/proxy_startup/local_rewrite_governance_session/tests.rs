use prodex_domain::{PrincipalKind, Role};

use super::{
    Arc, AuditEventId, BTreeMap, BTreeSet, Channel, CredentialScope, DataClassification, Mutex,
    PolicyRevisionId, Principal, PrincipalId, ProviderId, RuntimeGatewayGovernanceSessionStore,
    RuntimeGovernanceAuthority, RuntimeProxyRequest, TenantContext, TenantId,
    runtime_gateway_governance_session_revocation_changed,
    runtime_gateway_governance_sessions_mark_unavailable,
};

fn principal(tenant_id: TenantId) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        Role::Operator,
        CredentialScope::DataPlane,
    )
}

fn request() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: vec![("session_id".to_string(), "opaque-session".to_string())],
        body: Vec::new(),
    }
}

#[test]
fn session_context_propagates_age_idle_classification_and_affinity() {
    let store = RuntimeGatewayGovernanceSessionStore::default();
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let principal = principal(tenant.tenant_id);
    let initial = store.snapshot(&request(), tenant, &principal, Channel::Api, 100);
    let policy_revision = PolicyRevisionId::new();
    store
        .remember(
            initial,
            tenant,
            &principal,
            Channel::Api,
            100,
            DataClassification::Confidential,
            policy_revision,
            7,
            9,
            ProviderId::Gemini,
            prodex_config::GovernanceSessionConfig::default(),
        )
        .unwrap();

    let resumed = store.snapshot(&request(), tenant, &principal, Channel::Api, 112);
    assert_eq!(resumed.policy.age_seconds, 12);
    assert_eq!(resumed.policy.idle_seconds, 12);
    assert_eq!(
        resumed.policy.retained_classification,
        DataClassification::Confidential
    );
    assert_eq!(resumed.affinity_provider, Some(ProviderId::Gemini));
    assert_eq!(resumed.pinned_registry_revision, Some(7));
    assert_eq!(resumed.pinned_provider_descriptor_revision, Some(9));
    assert_eq!(resumed.pinned_policy_revision, Some(policy_revision));
    assert!(!resumed.policy_revision_mismatch(policy_revision));
    assert!(resumed.policy_revision_mismatch(PolicyRevisionId::new()));
    assert!(!resumed.provider_revision_mismatch(7, 9));
    assert!(resumed.provider_revision_mismatch(8, 9));
    assert!(!resumed.policy.revoked);
}

#[test]
fn session_reuse_with_another_principal_is_revoked() {
    let store = RuntimeGatewayGovernanceSessionStore::default();
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let owner = principal(tenant.tenant_id);
    let initial = store.snapshot(&request(), tenant, &owner, Channel::Api, 100);
    store
        .remember(
            initial,
            tenant,
            &owner,
            Channel::Api,
            100,
            DataClassification::Internal,
            PolicyRevisionId::new(),
            1,
            1,
            ProviderId::OpenAi,
            prodex_config::GovernanceSessionConfig::default(),
        )
        .unwrap();

    let other = principal(tenant.tenant_id);
    let resumed = store.snapshot(&request(), tenant, &other, Channel::Api, 101);
    assert!(resumed.policy.revoked);
    assert_eq!(resumed.affinity_provider, None);
}

#[test]
fn configured_timeouts_and_concurrency_fail_closed() {
    let store = RuntimeGatewayGovernanceSessionStore::default();
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let principal = principal(tenant.tenant_id);
    let first_request = request();
    let initial = store.snapshot(&first_request, tenant, &principal, Channel::Api, 100);
    store
        .remember(
            initial,
            tenant,
            &principal,
            Channel::Api,
            100,
            DataClassification::Internal,
            PolicyRevisionId::new(),
            1,
            1,
            ProviderId::OpenAi,
            prodex_config::GovernanceSessionConfig::default(),
        )
        .unwrap();
    let resumed = store.snapshot(&first_request, tenant, &principal, Channel::Api, 112);
    assert_eq!(
        store.configured_violation(
            resumed,
            tenant,
            &principal,
            112,
            prodex_config::GovernanceSessionConfig {
                absolute_timeout_seconds: Some(10),
                idle_timeout_seconds: None,
                max_concurrent: None,
            },
        ),
        Some("session_absolute_timeout")
    );

    let mut second_request = request();
    second_request.headers[0].1 = "second-session".to_string();
    let second = store.snapshot(&second_request, tenant, &principal, Channel::Api, 101);
    assert_eq!(
        store.configured_violation(
            second,
            tenant,
            &principal,
            101,
            prodex_config::GovernanceSessionConfig {
                absolute_timeout_seconds: None,
                idle_timeout_seconds: None,
                max_concurrent: Some(1),
            },
        ),
        Some("session_concurrency_limit")
    );
}

#[test]
fn cross_replica_revocation_epoch_invalidates_cached_sessions_promptly() {
    let tenant_id = TenantId::new();
    let root = std::env::temp_dir().join(format!(
        "prodex-session-revocation-epoch-{}",
        AuditEventId::new()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    for migration in prodex_storage_sqlite::SQLITE_MIGRATIONS {
        connection.execute_batch(migration.sql).unwrap();
    }
    connection
        .execute(
            "INSERT INTO prodex_tenants (
                    tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES (?1, 'tenant', 1, 1)",
            rusqlite::params![tenant_id.to_string()],
        )
        .unwrap();
    drop(connection);

    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: path.clone(),
        tenant_ids: Arc::new(Mutex::new(BTreeSet::from([tenant_id]))),
    };
    let repository =
        prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(&path).unwrap();
    let mut epochs = BTreeMap::new();
    assert!(
        runtime_gateway_governance_session_revocation_changed(
            &authority,
            Some(&repository),
            &mut epochs,
        )
        .unwrap()
    );
    assert!(
        !runtime_gateway_governance_session_revocation_changed(
            &authority,
            Some(&repository),
            &mut epochs,
        )
        .unwrap()
    );

    let second_replica = rusqlite::Connection::open(&path).unwrap();
    second_replica
        .execute(
            "UPDATE prodex_tenants SET session_revocation_epoch = 1 WHERE tenant_id = ?1",
            rusqlite::params![tenant_id.to_string()],
        )
        .unwrap();
    drop(second_replica);
    assert!(
        runtime_gateway_governance_session_revocation_changed(
            &authority,
            Some(&repository),
            &mut epochs,
        )
        .unwrap()
    );
    assert_eq!(epochs.get(&tenant_id), Some(&1));

    assert!(
        runtime_gateway_governance_session_revocation_changed(&authority, None, &mut epochs)
            .is_err()
    );

    let store = RuntimeGatewayGovernanceSessionStore::default();
    {
        let mut state = store.0.state.lock().unwrap();
        state.durable = true;
        state.hydrated_tenants.insert(tenant_id);
    }
    runtime_gateway_governance_sessions_mark_unavailable(&store);
    let principal = principal(tenant_id);
    let snapshot = store.snapshot(
        &request(),
        TenantContext { tenant_id },
        &principal,
        Channel::Api,
        1,
    );
    assert_eq!(
        store.configured_violation(
            snapshot,
            TenantContext { tenant_id },
            &principal,
            1,
            prodex_config::GovernanceSessionConfig::default(),
        ),
        Some("session_store_unavailable")
    );

    drop(repository);
    std::fs::remove_dir_all(root).unwrap();
}
