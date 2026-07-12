use prodex_control_plane::ControlPlaneOperation;
use prodex_domain::{ResourceKind, VirtualKeyId};

use super::test_support::{action_plan, id};
use super::*;

const NOW: u64 = 2_000;

fn key_id(value: &str) -> VirtualKeyId {
    id(value)
}

fn fingerprint() -> ApplicationSecretFingerprint {
    ApplicationSecretFingerprint::new("sha256:test-fingerprint").unwrap()
}

fn scoped_governance() -> ApplicationControlPlaneGovernanceScope {
    ApplicationControlPlaneGovernanceScope::new(
        Some("tenant-a".to_string()),
        Some("team-a".to_string()),
        Some("project-a".to_string()),
        Some("user-a".to_string()),
        Some("budget-a".to_string()),
        vec!["team-a-".to_string()],
    )
}

fn record(id: VirtualKeyId, name: &str) -> ApplicationGatewayVirtualKeyRecord {
    ApplicationGatewayVirtualKeyRecord {
        id,
        tenant_id: Some("tenant-a".to_string()),
        name: name.to_string(),
        source: ApplicationGatewayRecordSource::Editable,
        team_id: Some("team-a".to_string()),
        project_id: Some("project-a".to_string()),
        user_id: Some("user-a".to_string()),
        budget_id: Some("budget-a".to_string()),
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(1),
        request_budget: Some(2),
        rpm_limit: Some(3),
        tpm_limit: Some(4),
        disabled: false,
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    }
}

fn plan(
    action: &prodex_control_plane::ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    records: &[ApplicationGatewayVirtualKeyRecord],
    mutation: ApplicationGatewayVirtualKeyMutation,
) -> Result<ApplicationGatewayVirtualKeyMutationPlan, ApplicationGatewayIdentityMutationError> {
    plan_application_gateway_virtual_key_mutation(ApplicationGatewayVirtualKeyMutationRequest {
        authorized_action: action,
        governance,
        current_records: records,
        mutation,
        now_unix_ms: NOW,
    })
}

#[test]
fn create_owns_defaults_and_retains_exact_authorized_action() {
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        None,
    );
    let governance = scoped_governance();
    let id = key_id("00000000-0000-7000-8000-000000000201");
    let result = plan(
        &action,
        &governance,
        &[],
        ApplicationGatewayVirtualKeyMutation::Create {
            id,
            name: "team-a-key".to_string(),
            patch: ApplicationGatewayVirtualKeyPatch::default(),
            secret_fingerprint: fingerprint(),
        },
    )
    .unwrap();

    assert_eq!(result.authorized_action, action);
    assert_eq!(
        result.secret_mutation,
        ApplicationSecretMutation::Create(fingerprint())
    );
    let ApplicationGatewayIdentityProjection::Insert { record } = result.projection else {
        panic!("create must insert");
    };
    assert_eq!(record.id, id);
    assert_eq!(record.tenant_id.as_deref(), Some("tenant-a"));
    assert_eq!(record.team_id.as_deref(), Some("team-a"));
    assert_eq!(record.project_id.as_deref(), Some("project-a"));
    assert_eq!(record.user_id.as_deref(), Some("user-a"));
    assert_eq!(record.budget_id.as_deref(), Some("budget-a"));
    assert!(record.allowed_models.is_empty());
    assert!(!record.disabled);
    assert_eq!(record.created_at_unix_ms, NOW);
    assert_eq!(record.updated_at_unix_ms, NOW);
}

#[test]
fn unscoped_create_preserves_file_and_redis_none_tenant_compatibility() {
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        Some("key-a"),
    );
    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[],
        ApplicationGatewayVirtualKeyMutation::Create {
            id: key_id("00000000-0000-7000-8000-000000000202"),
            name: "key-a".to_string(),
            patch: ApplicationGatewayVirtualKeyPatch::default(),
            secret_fingerprint: fingerprint(),
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Insert { record } = result.projection else {
        panic!("create must insert");
    };
    assert_eq!(record.tenant_id, None);
}

#[test]
fn create_patch_is_tri_state_and_cannot_escape_scope_defaults() {
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        None,
    );
    let governance = scoped_governance();
    let denied = plan(
        &action,
        &governance,
        &[],
        ApplicationGatewayVirtualKeyMutation::Create {
            id: key_id("00000000-0000-7000-8000-000000000203"),
            name: "team-a-key".to_string(),
            patch: ApplicationGatewayVirtualKeyPatch {
                tenant_id: ApplicationPatchValue::Clear,
                ..Default::default()
            },
            secret_fingerprint: fingerprint(),
        },
    );
    assert_eq!(
        denied.unwrap_err(),
        ApplicationGatewayIdentityMutationError::GovernanceDenied
    );

    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[],
        ApplicationGatewayVirtualKeyMutation::Create {
            id: key_id("00000000-0000-7000-8000-000000000204"),
            name: "key-b".to_string(),
            patch: ApplicationGatewayVirtualKeyPatch {
                tenant_id: ApplicationPatchValue::Set("tenant-α".to_string()),
                allowed_models: ApplicationPatchValue::Set(vec!["provider/model:v1".to_string()]),
                disabled: ApplicationPatchValue::Set(true),
                ..Default::default()
            },
            secret_fingerprint: fingerprint(),
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Insert { record } = result.projection else {
        panic!("create must insert");
    };
    assert_eq!(record.tenant_id.as_deref(), Some("tenant-α"));
    assert_eq!(record.allowed_models, ["provider/model:v1"]);
    assert!(record.disabled);
}

#[test]
fn create_rejects_case_insensitive_name_and_exact_id_duplicates() {
    let id = key_id("00000000-0000-7000-8000-000000000205");
    let records = [record(id, "key-a")];
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        None,
    );
    for (candidate_id, name, expected) in [
        (
            id,
            "key-b",
            ApplicationGatewayIdentityMutationError::DuplicateIdentity,
        ),
        (
            key_id("00000000-0000-7000-8000-000000000206"),
            "KEY-A",
            ApplicationGatewayIdentityMutationError::DuplicateName,
        ),
    ] {
        let result = plan(
            &action,
            &ApplicationControlPlaneGovernanceScope::default(),
            &records,
            ApplicationGatewayVirtualKeyMutation::Create {
                id: candidate_id,
                name: name.to_string(),
                patch: ApplicationGatewayVirtualKeyPatch::default(),
                secret_fingerprint: fingerprint(),
            },
        );
        assert_eq!(result.unwrap_err(), expected);
    }
}

#[test]
fn item_mutations_require_exact_resource_name() {
    let id = key_id("00000000-0000-7000-8000-000000000207");
    let records = [record(id, "team-a-key")];
    let governance = scoped_governance();
    for (operation, mutation) in [
        (
            ControlPlaneOperation::VirtualKeyUpdate,
            ApplicationGatewayVirtualKeyMutation::Update {
                id,
                patch: ApplicationGatewayVirtualKeyPatch::default(),
            },
        ),
        (
            ControlPlaneOperation::VirtualKeyRotateSecret,
            ApplicationGatewayVirtualKeyMutation::RotateSecret {
                id,
                patch: ApplicationGatewayVirtualKeyPatch::default(),
                secret_fingerprint: fingerprint(),
            },
        ),
        (
            ControlPlaneOperation::VirtualKeyDelete,
            ApplicationGatewayVirtualKeyMutation::Delete { id },
        ),
    ] {
        for resource_id in [None, Some("wrong-key")] {
            let action = action_plan(operation, ResourceKind::VirtualKey, resource_id);
            assert_eq!(
                plan(&action, &governance, &records, mutation.clone()).unwrap_err(),
                ApplicationGatewayIdentityMutationError::ResourceIdMismatch,
            );
        }
    }
}

#[test]
fn update_checks_pre_and_post_governance_and_read_only_source() {
    let id = key_id("00000000-0000-7000-8000-000000000208");
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyUpdate,
        ResourceKind::VirtualKey,
        Some("team-a-key"),
    );
    let governance = scoped_governance();
    let mut outside = record(id, "team-a-key");
    outside.team_id = Some("team-b".to_string());
    assert_eq!(
        plan(
            &action,
            &governance,
            &[outside],
            ApplicationGatewayVirtualKeyMutation::Update {
                id,
                patch: ApplicationGatewayVirtualKeyPatch::default(),
            },
        )
        .unwrap_err(),
        ApplicationGatewayIdentityMutationError::GovernanceDenied,
    );

    let current = record(id, "team-a-key");
    assert_eq!(
        plan(
            &action,
            &governance,
            std::slice::from_ref(&current),
            ApplicationGatewayVirtualKeyMutation::Update {
                id,
                patch: ApplicationGatewayVirtualKeyPatch {
                    team_id: ApplicationPatchValue::Set("team-b".to_string()),
                    ..Default::default()
                },
            },
        )
        .unwrap_err(),
        ApplicationGatewayIdentityMutationError::GovernanceDenied,
    );

    let mut read_only = current;
    read_only.source = ApplicationGatewayRecordSource::ReadOnly;
    assert_eq!(
        plan(
            &action,
            &governance,
            &[read_only],
            ApplicationGatewayVirtualKeyMutation::Update {
                id,
                patch: ApplicationGatewayVirtualKeyPatch::default(),
            },
        )
        .unwrap_err(),
        ApplicationGatewayIdentityMutationError::ReadOnly,
    );
}

#[test]
fn update_patch_preserves_sets_and_clears_fields() {
    let id = key_id("00000000-0000-7000-8000-000000000209");
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyUpdate,
        ResourceKind::VirtualKey,
        Some("key-a"),
    );
    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[record(id, "key-a")],
        ApplicationGatewayVirtualKeyMutation::Update {
            id,
            patch: ApplicationGatewayVirtualKeyPatch {
                tenant_id: ApplicationPatchValue::Unchanged,
                team_id: ApplicationPatchValue::Clear,
                allowed_models: ApplicationPatchValue::Set(vec!["claude-4".to_string()]),
                disabled: ApplicationPatchValue::Set(true),
                ..Default::default()
            },
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Replace { index, record } = result.projection else {
        panic!("update must replace");
    };
    assert_eq!(index, 0);
    assert_eq!(record.tenant_id.as_deref(), Some("tenant-a"));
    assert_eq!(record.team_id, None);
    assert_eq!(record.allowed_models, ["claude-4"]);
    assert!(record.disabled);
    assert_eq!(record.updated_at_unix_ms, NOW);
}

#[test]
fn secret_rotation_applies_patch_atomically_under_rotate_authorization() {
    let id = key_id("00000000-0000-7000-8000-000000000212");
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyRotateSecret,
        ResourceKind::VirtualKey,
        Some("key-a"),
    );
    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[record(id, "key-a")],
        ApplicationGatewayVirtualKeyMutation::RotateSecret {
            id,
            patch: ApplicationGatewayVirtualKeyPatch {
                team_id: ApplicationPatchValue::Clear,
                disabled: ApplicationPatchValue::Set(true),
                ..Default::default()
            },
            secret_fingerprint: fingerprint(),
        },
    )
    .unwrap();

    assert_eq!(result.authorized_action, action);
    assert_eq!(
        result.secret_mutation,
        ApplicationSecretMutation::Rotate(fingerprint())
    );
    let ApplicationGatewayIdentityProjection::Replace { record, .. } = result.projection else {
        panic!("rotation must replace");
    };
    assert_eq!(record.team_id, None);
    assert!(record.disabled);
    assert_eq!(record.updated_at_unix_ms, NOW);
}

#[test]
fn model_and_scope_grammar_is_bounded_and_case_insensitively_unique() {
    let id = key_id("00000000-0000-7000-8000-000000000210");
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyUpdate,
        ResourceKind::VirtualKey,
        Some("key-a"),
    );
    for models in [
        vec![String::new()],
        vec!["x".repeat(257)],
        vec!["gpt 5".to_string()],
        vec!["gpt\n5".to_string()],
    ] {
        let result = plan(
            &action,
            &ApplicationControlPlaneGovernanceScope::default(),
            &[record(id, "key-a")],
            ApplicationGatewayVirtualKeyMutation::Update {
                id,
                patch: ApplicationGatewayVirtualKeyPatch {
                    allowed_models: ApplicationPatchValue::Set(models),
                    ..Default::default()
                },
            },
        );
        assert_eq!(
            result.unwrap_err(),
            ApplicationGatewayIdentityMutationError::InvalidModel
        );
    }
    let duplicate = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[record(id, "key-a")],
        ApplicationGatewayVirtualKeyMutation::Update {
            id,
            patch: ApplicationGatewayVirtualKeyPatch {
                allowed_models: ApplicationPatchValue::Set(vec![
                    "gpt-5".to_string(),
                    "GPT-5".to_string(),
                ]),
                ..Default::default()
            },
        },
    );
    assert_eq!(
        duplicate.unwrap_err(),
        ApplicationGatewayIdentityMutationError::DuplicateModel
    );
}

#[test]
fn debug_output_redacts_identity_policy_and_secret_material() {
    let id = key_id("00000000-0000-7000-8000-000000000211");
    let action = action_plan(
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        None,
    );
    let governance = ApplicationControlPlaneGovernanceScope::new(
        Some("tenant-debug-secret".to_string()),
        None,
        None,
        None,
        None,
        Vec::new(),
    );
    let request = ApplicationGatewayVirtualKeyMutationRequest {
        authorized_action: &action,
        governance: &governance,
        current_records: &[],
        mutation: ApplicationGatewayVirtualKeyMutation::Create {
            id,
            name: "key-debug-secret".to_string(),
            patch: ApplicationGatewayVirtualKeyPatch {
                allowed_models: ApplicationPatchValue::Set(vec!["model-debug-secret".to_string()]),
                ..Default::default()
            },
            secret_fingerprint: ApplicationSecretFingerprint::new("fingerprint-debug-secret")
                .unwrap(),
        },
        now_unix_ms: NOW,
    };
    let rendered = format!("{request:?}");
    for secret in [
        "tenant-debug-secret",
        "key-debug-secret",
        "model-debug-secret",
        "fingerprint-debug-secret",
    ] {
        assert!(!rendered.contains(secret), "{rendered}");
    }
}
