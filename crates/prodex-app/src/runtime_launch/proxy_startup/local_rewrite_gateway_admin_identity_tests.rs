use std::str::FromStr;

use prodex_application::{
    ApplicationControlPlaneGovernanceScope, ApplicationGatewayIdentityProjection,
    ApplicationGatewayScimUserMutationRequest, ApplicationGatewayVirtualKeyMutationRequest,
    ApplicationPatchValue, ApplicationScimUserUpdateMode, ApplicationSecretFingerprint,
    plan_application_gateway_scim_user_mutation, plan_application_gateway_virtual_key_mutation,
};
use prodex_control_plane::{
    ControlPlaneActionPlan, ControlPlaneActionRequest, ControlPlaneDecision, ControlPlaneOperation,
    ControlPlaneResourceRef, decide_control_plane_action,
};
use prodex_domain::{
    CredentialScope, Principal, PrincipalId, PrincipalKind, ResourceKind, Role, TenantId,
    VirtualKeyId,
};

use super::*;

const NOW: u64 = 2_000;

fn id<T>(value: &str) -> T
where
    T: FromStr,
    T::Err: std::fmt::Debug,
{
    value.parse().unwrap()
}

fn tenant_id() -> TenantId {
    id("00000000-0000-7000-8000-000000000401")
}

fn key_id() -> VirtualKeyId {
    id("00000000-0000-7000-8000-000000000402")
}

fn user_id() -> PrincipalId {
    id("00000000-0000-7000-8000-000000000403")
}

fn action(
    operation: ControlPlaneOperation,
    kind: ResourceKind,
    resource_id: Option<&str>,
) -> ControlPlaneActionPlan {
    let tenant_id = tenant_id();
    let principal = Principal::new(
        id::<PrincipalId>("00000000-0000-7000-8000-000000000404"),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    match decide_control_plane_action(ControlPlaneActionRequest {
        principal,
        operation,
        resource: ControlPlaneResourceRef::new(tenant_id, kind, resource_id),
        occurred_at_unix_ms: NOW,
    }) {
        ControlPlaneDecision::Authorized(plan) => plan,
        ControlPlaneDecision::Denied { .. } => panic!("test action should authorize"),
    }
}

fn fingerprint(value: &str) -> ApplicationSecretFingerprint {
    ApplicationSecretFingerprint::new(value).unwrap()
}

fn stored_key() -> RuntimeGatewayStoredVirtualKey {
    RuntimeGatewayStoredVirtualKey {
        name: "key-a".to_string(),
        token_hash_base64: "sha256:old-fingerprint".to_string(),
        virtual_key_id: Some(key_id().to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: Some("team-a".to_string()),
        project_id: Some("project-a".to_string()),
        user_id: Some("user-a".to_string()),
        budget_id: Some("budget-a".to_string()),
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(10),
        request_budget: Some(20),
        rpm_limit: Some(30),
        tpm_limit: Some(40),
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    }
}

fn stored_user() -> RuntimeGatewayScimUser {
    RuntimeGatewayScimUser {
        id: user_id().to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: Some("external-a".to_string()),
        display_name: Some("Alice Example".to_string()),
        active: true,
        role: Some("viewer".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: Some("team-a".to_string()),
        project_id: None,
        user_id: Some("user-a".to_string()),
        budget_id: None,
        allowed_key_prefixes: vec!["team-a-".to_string()],
        created_at_epoch: 1,
        updated_at_epoch: 2,
    }
}

#[test]
fn stored_records_round_trip_without_secret_projection_or_epoch_wrap() {
    let key = stored_key();
    let key_record = runtime_gateway_virtual_key_record(&key).unwrap();
    assert_eq!(key_record.id, key_id());
    assert_eq!(key_record.tenant_id.as_deref(), Some("tenant-a"));
    assert_eq!(key_record.created_at_unix_ms, 1_000);
    assert_eq!(key_record.updated_at_unix_ms, 2_000);

    let user = stored_user();
    let user_record = runtime_gateway_scim_user_record(&user).unwrap();
    assert_eq!(user_record.id, user_id());
    assert_eq!(user_record.user_name, "alice@example.com");
    assert_eq!(user_record.updated_at_unix_ms, 2_000);

    let mut overflow = key;
    overflow.created_at_epoch = u64::MAX;
    assert_eq!(
        runtime_gateway_virtual_key_record(&overflow)
            .unwrap()
            .created_at_unix_ms,
        u64::MAX,
    );
}

#[test]
fn legacy_missing_and_invalid_ids_fail_with_stable_redacted_errors() {
    let mut key = stored_key();
    key.virtual_key_id = None;
    let missing = runtime_gateway_virtual_key_record(&key).unwrap_err();
    assert_eq!(missing.test_status(), 400);
    assert_eq!(missing.test_code(), "gateway_key_id_required");

    key.virtual_key_id = Some("key-id-debug-secret".to_string());
    let invalid = runtime_gateway_virtual_key_record(&key).unwrap_err();
    assert_eq!(invalid.test_code(), "gateway_key_id_invalid");
    assert!(!format!("{invalid:?}").contains("key-id-debug-secret"));

    let mut user = stored_user();
    user.id = "user-id-debug-secret".to_string();
    let invalid = runtime_gateway_scim_user_record(&user).unwrap_err();
    assert_eq!(invalid.test_code(), "gateway_scim_user_id_invalid");
    assert!(!format!("{invalid:?}").contains("user-id-debug-secret"));
}

#[test]
fn virtual_key_json_maps_absent_null_and_values_to_tri_state() {
    let mutation = runtime_gateway_virtual_key_update_mutation(
        key_id(),
        &serde_json::json!({
            "tenant_id": null,
            "team_id": "team-b",
            "allowed_models": ["gpt-5", "claude-4"],
            "budget_usd": 1.25,
            "disabled": true
        }),
        None,
    )
    .unwrap();
    let prodex_application::ApplicationGatewayVirtualKeyMutation::Update { patch, .. } = mutation
    else {
        panic!("missing fingerprint must select update");
    };

    assert_eq!(patch.tenant_id, ApplicationPatchValue::Clear);
    assert_eq!(
        patch.team_id,
        ApplicationPatchValue::Set("team-b".to_string())
    );
    assert_eq!(patch.project_id, ApplicationPatchValue::Unchanged);
    assert_eq!(
        patch.allowed_models,
        ApplicationPatchValue::Set(vec!["gpt-5".to_string(), "claude-4".to_string()])
    );
    assert_eq!(patch.budget_microusd, ApplicationPatchValue::Set(1_250_000));
    assert_eq!(patch.disabled, ApplicationPatchValue::Set(true));
}

#[test]
fn scim_direct_and_operations_json_map_to_the_same_typed_patch() {
    let direct = runtime_gateway_scim_patch(&serde_json::json!({
        "userName": "alice@example.com",
        "displayName": null,
        "active": false,
        "urn:prodex:params:scim:schemas:gateway:2.0:User": {
            "tenant_id": "tenant-a",
            "allowed_key_prefixes": ["team-a-"]
        }
    }))
    .unwrap();
    assert_eq!(
        direct.user_name,
        ApplicationPatchValue::Set("alice@example.com".to_string())
    );
    assert_eq!(direct.display_name, ApplicationPatchValue::Clear);
    assert_eq!(direct.active, ApplicationPatchValue::Set(false));
    assert_eq!(
        direct.tenant_id,
        ApplicationPatchValue::Set("tenant-a".to_string())
    );

    let operations = runtime_gateway_scim_patch(&serde_json::json!({
        "Operations": [
            {"op": "replace", "path": "userName", "value": "alice@example.com"},
            {"op": "remove", "path": "displayName"},
            {"op": "replace", "path": "active", "value": false},
            {"op": "replace", "path": "tenant_id", "value": "tenant-a"}
        ]
    }))
    .unwrap();
    assert_eq!(operations.user_name, direct.user_name);
    assert_eq!(operations.display_name, direct.display_name);
    assert_eq!(operations.active, direct.active);
    assert_eq!(operations.tenant_id, direct.tenant_id);
}

#[test]
fn key_projection_applies_create_atomic_patch_rotation_and_delete_without_raw_token() {
    let governance = ApplicationControlPlaneGovernanceScope::default();
    let create_action = action(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        None,
    );
    let raw_token = "raw-token-debug-secret";
    let create_mutation = runtime_gateway_virtual_key_create_mutation(
        key_id(),
        &serde_json::json!({"name": "key-a", "token": raw_token, "team_id": "team-a"}),
        fingerprint("sha256:create-fingerprint"),
    )
    .unwrap();
    assert!(!format!("{create_mutation:?}").contains(raw_token));
    let create_plan = plan_application_gateway_virtual_key_mutation(
        ApplicationGatewayVirtualKeyMutationRequest {
            authorized_action: &create_action,
            governance: &governance,
            current_records: &[],
            mutation: create_mutation,
            now_unix_ms: NOW,
        },
    )
    .unwrap();
    let mut store = RuntimeGatewayVirtualKeyStoreFile::default();
    let created = runtime_gateway_apply_virtual_key_projection(&mut store, &create_plan).unwrap();
    assert_eq!(created.token_hash_base64, "sha256:create-fingerprint");
    assert_eq!(created.team_id.as_deref(), Some("team-a"));

    let current = [runtime_gateway_virtual_key_record(&created).unwrap()];
    let rotate_action = action(
        ControlPlaneOperation::VirtualKeyRotateSecret,
        ResourceKind::VirtualKey,
        Some("key-a"),
    );
    let rotate_mutation = runtime_gateway_virtual_key_update_mutation(
        key_id(),
        &serde_json::json!({"team_id": null, "disabled": true, "token": raw_token}),
        Some(fingerprint("sha256:rotated-fingerprint")),
    )
    .unwrap();
    assert!(!format!("{rotate_mutation:?}").contains(raw_token));
    let rotate_plan = plan_application_gateway_virtual_key_mutation(
        ApplicationGatewayVirtualKeyMutationRequest {
            authorized_action: &rotate_action,
            governance: &governance,
            current_records: &current,
            mutation: rotate_mutation,
            now_unix_ms: NOW + 1_000,
        },
    )
    .unwrap();
    let rotated = runtime_gateway_apply_virtual_key_projection(&mut store, &rotate_plan).unwrap();
    assert_eq!(rotated.token_hash_base64, "sha256:rotated-fingerprint");
    assert_eq!(rotated.team_id, None);
    assert_eq!(rotated.disabled, Some(true));

    let current = [runtime_gateway_virtual_key_record(&rotated).unwrap()];
    let delete_action = action(
        ControlPlaneOperation::VirtualKeyDelete,
        ResourceKind::VirtualKey,
        Some("key-a"),
    );
    let delete_plan = plan_application_gateway_virtual_key_mutation(
        ApplicationGatewayVirtualKeyMutationRequest {
            authorized_action: &delete_action,
            governance: &governance,
            current_records: &current,
            mutation: runtime_gateway_virtual_key_delete_mutation(key_id()),
            now_unix_ms: NOW + 2_000,
        },
    )
    .unwrap();
    let deleted = runtime_gateway_apply_virtual_key_projection(&mut store, &delete_plan).unwrap();
    assert_eq!(deleted.name, "key-a");
    assert!(store.keys.is_empty());
}

#[test]
fn scim_projection_applies_insert_replace_and_preserves_typed_identity() {
    let governance = ApplicationControlPlaneGovernanceScope::default();
    let create_action = action(
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        None,
    );
    let create_plan =
        plan_application_gateway_scim_user_mutation(ApplicationGatewayScimUserMutationRequest {
            authorized_action: &create_action,
            governance: &governance,
            current_records: &[],
            mutation: runtime_gateway_scim_create_mutation(
                user_id(),
                &serde_json::json!({"userName": "alice@example.com"}),
            )
            .unwrap(),
            now_unix_ms: NOW,
        })
        .unwrap();
    let mut store = RuntimeGatewayVirtualKeyStoreFile::default();
    let created = runtime_gateway_apply_scim_user_projection(&mut store, &create_plan).unwrap();
    assert_eq!(created.id, user_id().to_string());

    let current = [runtime_gateway_scim_user_record(&created).unwrap()];
    let update_action = action(
        ControlPlaneOperation::ScimUserUpdate,
        ResourceKind::User,
        Some(&user_id().to_string()),
    );
    let update_plan =
        plan_application_gateway_scim_user_mutation(ApplicationGatewayScimUserMutationRequest {
            authorized_action: &update_action,
            governance: &governance,
            current_records: &current,
            mutation: runtime_gateway_scim_update_mutation(
                user_id(),
                ApplicationScimUserUpdateMode::Patch,
                &serde_json::json!({"displayName": "Alice Updated"}),
            )
            .unwrap(),
            now_unix_ms: NOW + 1_000,
        })
        .unwrap();
    let updated = runtime_gateway_apply_scim_user_projection(&mut store, &update_plan).unwrap();
    assert_eq!(updated.display_name.as_deref(), Some("Alice Updated"));
    assert_eq!(store.scim_users.len(), 1);
    assert!(matches!(
        runtime_gateway_scim_delete_mutation(user_id()),
        prodex_application::ApplicationGatewayScimUserMutation::Delete { .. }
    ));
}

#[test]
fn application_errors_map_to_existing_static_http_contracts() {
    let key = runtime_gateway_identity_error(
        RuntimeGatewayIdentityKind::VirtualKey,
        ApplicationGatewayIdentityMutationError::DuplicateName,
    );
    assert_eq!(key.test_status(), 409);
    assert_eq!(key.test_code(), "gateway_key_exists");

    let scim = runtime_gateway_identity_error(
        RuntimeGatewayIdentityKind::ScimUser,
        ApplicationGatewayIdentityMutationError::GovernanceDenied,
    );
    assert_eq!(scim.test_status(), 403);
    assert_eq!(scim.test_code(), "gateway_admin_key_scope_forbidden");
    assert!(!format!("{scim:?}").contains("tenant-debug-secret"));
}

#[test]
fn projection_rejects_stale_index_without_mutating_store() {
    let action = action(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        None,
    );
    let mut plan = plan_application_gateway_virtual_key_mutation(
        ApplicationGatewayVirtualKeyMutationRequest {
            authorized_action: &action,
            governance: &ApplicationControlPlaneGovernanceScope::default(),
            current_records: &[],
            mutation: runtime_gateway_virtual_key_create_mutation(
                key_id(),
                &serde_json::json!({"name": "key-a"}),
                fingerprint("sha256:fingerprint"),
            )
            .unwrap(),
            now_unix_ms: NOW,
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Insert { record } = &plan.projection else {
        panic!("create must insert");
    };
    plan.projection = ApplicationGatewayIdentityProjection::Replace {
        index: 1,
        record: record.clone(),
    };
    plan.secret_mutation = ApplicationSecretMutation::None;
    let mut store = RuntimeGatewayVirtualKeyStoreFile::default();
    let error = runtime_gateway_apply_virtual_key_projection(&mut store, &plan).unwrap_err();
    assert_eq!(error.test_code(), "gateway_key_store_invalid");
    assert!(store.keys.is_empty());
}
