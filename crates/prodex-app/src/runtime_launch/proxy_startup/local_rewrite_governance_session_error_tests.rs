use super::{
    RuntimeGatewayGovernanceSessionPersistError, RuntimeGatewayGovernanceSessionRecord,
    RuntimeGatewayGovernanceSessionStore, RuntimeGovernanceAuthority,
    runtime_gateway_governance_session_refresh, stored_record,
};
use prodex_domain::{
    Channel, CredentialScope, DataClassification, PolicyRevisionId, Principal, PrincipalId,
    PrincipalKind, Role, TenantId,
};
use prodex_provider_core::ProviderId;
use prodex_storage::{GovernanceRepositoryError, GovernanceSessionRecord};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::thread;

fn principal(tenant_id: TenantId) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        Role::Operator,
        CredentialScope::DataPlane,
    )
}

#[test]
fn memory_refresh_reports_poisoned_session_state() {
    let tenant_id = TenantId::new();
    let principal = principal(tenant_id);
    let store = RuntimeGatewayGovernanceSessionStore::default();
    let poisoned = Arc::clone(&store.0);
    assert!(
        thread::spawn(move || {
            let _guard = poisoned.state.lock().unwrap();
            panic!("poison session lock");
        })
        .join()
        .is_err()
    );
    let record = RuntimeGatewayGovernanceSessionRecord {
        session_id_hash: [1; 32],
        tenant_id,
        principal_id: principal.id,
        credential_scope: principal.credential_scope,
        channel: Channel::Api,
        created_at_seconds: 1,
        last_seen_seconds: 1,
        revoked: false,
        classification: DataClassification::Internal,
        policy_revision: PolicyRevisionId::new(),
        registry_revision: 1,
        provider_descriptor_revision: 1,
        provider: ProviderId::OpenAi,
    };

    assert_eq!(
        store.remember_memory(record),
        Err(RuntimeGatewayGovernanceSessionPersistError::Unavailable)
    );
}

#[test]
fn durable_refresh_propagates_tenant_discovery_failure() {
    let tenant_ids = Arc::new(Mutex::new(BTreeSet::new()));
    let poisoned = Arc::clone(&tenant_ids);
    assert!(
        thread::spawn(move || {
            let _guard = poisoned.lock().unwrap();
            panic!("poison tenant lock");
        })
        .join()
        .is_err()
    );
    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: "unused.sqlite".into(),
        tenant_ids,
    };

    assert_eq!(
        runtime_gateway_governance_session_refresh(
            &RuntimeGatewayGovernanceSessionStore::default(),
            &authority,
            None,
        ),
        Err(GovernanceRepositoryError::Database)
    );
}

#[test]
fn durable_session_round_trips_separate_provider_revisions() {
    let tenant_id = TenantId::new();
    let principal = principal(tenant_id);
    let policy_revision = PolicyRevisionId::new();
    let record = GovernanceSessionRecord {
        tenant_id,
        session_id_hash: "a".repeat(64),
        principal_id: principal.id,
        channel: Channel::Api,
        credential_scope: CredentialScope::DataPlane,
        classification: DataClassification::Restricted,
        policy_revision_id: policy_revision,
        provider_registry_revision: "7".to_string(),
        provider_descriptor_revision: 9,
        provider_affinity: Some("gemini".to_string()),
        created_at_unix_ms: 100_000,
        last_seen_at_unix_ms: 112_000,
        absolute_expires_at_unix_ms: 200_000,
        idle_expires_at_unix_ms: 150_000,
        revoked_at_unix_ms: None,
        revocation_reason_code: None,
    };

    let stored = stored_record(record).unwrap();
    assert_eq!(stored.registry_revision, 7);
    assert_eq!(stored.provider_descriptor_revision, 9);
    assert_eq!(stored.provider, ProviderId::Gemini);
    assert_eq!(stored.classification, DataClassification::Restricted);
    assert_eq!(stored.policy_revision, policy_revision);
}
