use prodex_control_plane::ControlPlaneOperation;
use prodex_domain::{PrincipalId, ResourceKind};

use super::test_support::{action_plan, id};
use super::*;

const NOW: u64 = 2_000;

fn user_id(value: &str) -> PrincipalId {
    id(value)
}

fn scoped_governance() -> ApplicationControlPlaneGovernanceScope {
    ApplicationControlPlaneGovernanceScope::new(
        Some("tenant-a".to_string()),
        Some("team-a".to_string()),
        Some("project-a".to_string()),
        Some("delegated-user".to_string()),
        Some("budget-a".to_string()),
        vec!["team-a-".to_string()],
    )
}

fn record(id: PrincipalId, user_name: &str) -> ApplicationGatewayScimUserRecord {
    ApplicationGatewayScimUserRecord {
        id,
        tenant_id: Some("tenant-a".to_string()),
        user_name: user_name.to_string(),
        external_id: Some("external-a".to_string()),
        display_name: Some("Alice Example".to_string()),
        active: false,
        role: Some("admin".to_string()),
        team_id: Some("team-a".to_string()),
        project_id: Some("project-a".to_string()),
        user_id: Some("delegated-user".to_string()),
        budget_id: Some("budget-a".to_string()),
        allowed_key_prefixes: vec!["team-a-".to_string()],
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    }
}

fn plan(
    action: &prodex_control_plane::ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    records: &[ApplicationGatewayScimUserRecord],
    mutation: ApplicationGatewayScimUserMutation,
) -> Result<ApplicationGatewayScimUserMutationPlan, ApplicationGatewayIdentityMutationError> {
    plan_application_gateway_scim_user_mutation(ApplicationGatewayScimUserMutationRequest {
        authorized_action: action,
        governance,
        current_records: records,
        mutation,
        now_unix_ms: NOW,
    })
}

fn create_patch(user_name: &str) -> ApplicationGatewayScimUserPatch {
    ApplicationGatewayScimUserPatch {
        user_name: ApplicationPatchValue::Set(user_name.to_string()),
        ..Default::default()
    }
}

#[test]
fn create_owns_defaults_inherits_delegation_and_retains_action() {
    let id = user_id("00000000-0000-7000-8000-000000000301");
    let action = action_plan(
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        None,
    );
    let result = plan(
        &action,
        &scoped_governance(),
        &[],
        ApplicationGatewayScimUserMutation::Create {
            id,
            patch: create_patch("alice@example.com"),
        },
    )
    .unwrap();

    assert_eq!(result.authorized_action, action);
    let ApplicationGatewayIdentityProjection::Insert { record } = result.projection else {
        panic!("create must insert");
    };
    assert_eq!(record.id, id);
    assert_eq!(record.tenant_id.as_deref(), Some("tenant-a"));
    assert_eq!(record.team_id.as_deref(), Some("team-a"));
    assert_eq!(record.project_id.as_deref(), Some("project-a"));
    assert_eq!(record.user_id.as_deref(), Some("delegated-user"));
    assert_eq!(record.budget_id.as_deref(), Some("budget-a"));
    assert_eq!(record.allowed_key_prefixes, ["team-a-"]);
    assert!(record.active);
    assert_eq!(record.role, None);
    assert_eq!(record.created_at_unix_ms, NOW);
    assert_eq!(record.updated_at_unix_ms, NOW);
}

#[test]
fn unscoped_create_keeps_none_tenant_and_defaults_user_id_to_generated_id() {
    let id = user_id("00000000-0000-7000-8000-000000000302");
    let action = action_plan(
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        Some(&id.to_string()),
    );
    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[],
        ApplicationGatewayScimUserMutation::Create {
            id,
            patch: create_patch("alice@example.com"),
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Insert { record } = result.projection else {
        panic!("create must insert");
    };
    assert_eq!(record.tenant_id, None);
    assert_eq!(record.user_id.as_deref(), Some(id.to_string().as_str()));
    assert!(record.allowed_key_prefixes.is_empty());
}

#[test]
fn restricted_create_cannot_clear_or_widen_delegated_prefixes() {
    let id = user_id("00000000-0000-7000-8000-000000000303");
    let action = action_plan(
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        None,
    );
    for prefixes in [
        ApplicationPatchValue::Clear,
        ApplicationPatchValue::Set(Vec::new()),
        ApplicationPatchValue::Set(vec!["team-b-".to_string()]),
    ] {
        let result = plan(
            &action,
            &scoped_governance(),
            &[],
            ApplicationGatewayScimUserMutation::Create {
                id,
                patch: ApplicationGatewayScimUserPatch {
                    allowed_key_prefixes: prefixes,
                    ..create_patch("alice@example.com")
                },
            },
        );
        assert_eq!(
            result.unwrap_err(),
            ApplicationGatewayIdentityMutationError::GovernanceDenied
        );
    }
}

#[test]
fn create_requires_valid_unique_case_insensitive_user_name() {
    let existing_id = user_id("00000000-0000-7000-8000-000000000304");
    let new_id = user_id("00000000-0000-7000-8000-000000000305");
    let records = [record(existing_id, "Alice@example.com")];
    let action = action_plan(
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        None,
    );
    for (id, patch, expected) in [
        (
            new_id,
            ApplicationGatewayScimUserPatch::default(),
            ApplicationGatewayIdentityMutationError::RequiredFieldMissing,
        ),
        (
            new_id,
            create_patch("bad user"),
            ApplicationGatewayIdentityMutationError::InvalidScimUserName,
        ),
        (
            new_id,
            create_patch("alice@EXAMPLE.com"),
            ApplicationGatewayIdentityMutationError::DuplicateName,
        ),
        (
            existing_id,
            create_patch("different@example.com"),
            ApplicationGatewayIdentityMutationError::DuplicateIdentity,
        ),
    ] {
        assert_eq!(
            plan(
                &action,
                &ApplicationControlPlaneGovernanceScope::default(),
                &records,
                ApplicationGatewayScimUserMutation::Create { id, patch },
            )
            .unwrap_err(),
            expected,
        );
    }
}

#[test]
fn item_mutations_require_exact_user_id_resource() {
    let id = user_id("00000000-0000-7000-8000-000000000306");
    let records = [record(id, "alice@example.com")];
    for (operation, mutation) in [
        (
            ControlPlaneOperation::ScimUserUpdate,
            ApplicationGatewayScimUserMutation::Update {
                id,
                mode: ApplicationScimUserUpdateMode::Patch,
                patch: ApplicationGatewayScimUserPatch::default(),
            },
        ),
        (
            ControlPlaneOperation::ScimUserDelete,
            ApplicationGatewayScimUserMutation::Delete { id },
        ),
    ] {
        for resource_id in [None, Some("00000000-0000-7000-8000-000000000399")] {
            let action = action_plan(operation, ResourceKind::User, resource_id);
            assert_eq!(
                plan(&action, &scoped_governance(), &records, mutation.clone(),).unwrap_err(),
                ApplicationGatewayIdentityMutationError::ResourceIdMismatch,
            );
        }
    }
}

#[test]
fn patch_preserves_unchanged_and_applies_set_and_clear() {
    let id = user_id("00000000-0000-7000-8000-000000000307");
    let action = action_plan(
        ControlPlaneOperation::ScimUserUpdate,
        ResourceKind::User,
        Some(&id.to_string()),
    );
    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[record(id, "alice@example.com")],
        ApplicationGatewayScimUserMutation::Update {
            id,
            mode: ApplicationScimUserUpdateMode::Patch,
            patch: ApplicationGatewayScimUserPatch {
                display_name: ApplicationPatchValue::Clear,
                active: ApplicationPatchValue::Set(true),
                role: ApplicationPatchValue::Set("VIEWER".to_string()),
                ..Default::default()
            },
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Replace { record, .. } = result.projection else {
        panic!("update must replace");
    };
    assert_eq!(record.user_name, "alice@example.com");
    assert_eq!(record.external_id.as_deref(), Some("external-a"));
    assert_eq!(record.display_name, None);
    assert!(record.active);
    assert_eq!(record.role.as_deref(), Some("viewer"));
}

#[test]
fn replace_requires_name_and_resets_unchanged_optional_fields() {
    let id = user_id("00000000-0000-7000-8000-000000000308");
    let action = action_plan(
        ControlPlaneOperation::ScimUserUpdate,
        ResourceKind::User,
        Some(&id.to_string()),
    );
    let missing_name = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[record(id, "alice@example.com")],
        ApplicationGatewayScimUserMutation::Update {
            id,
            mode: ApplicationScimUserUpdateMode::Replace,
            patch: ApplicationGatewayScimUserPatch::default(),
        },
    );
    assert_eq!(
        missing_name.unwrap_err(),
        ApplicationGatewayIdentityMutationError::RequiredFieldMissing
    );

    let result = plan(
        &action,
        &ApplicationControlPlaneGovernanceScope::default(),
        &[record(id, "alice@example.com")],
        ApplicationGatewayScimUserMutation::Update {
            id,
            mode: ApplicationScimUserUpdateMode::Replace,
            patch: create_patch("alice@example.com"),
        },
    )
    .unwrap();
    let ApplicationGatewayIdentityProjection::Replace { record, .. } = result.projection else {
        panic!("update must replace");
    };
    assert_eq!(record.tenant_id, None);
    assert_eq!(record.external_id, None);
    assert_eq!(record.display_name, None);
    assert!(record.active);
    assert_eq!(record.role, None);
    assert!(record.allowed_key_prefixes.is_empty());
}

#[test]
fn update_checks_pre_and_post_governance_and_name_uniqueness() {
    let id = user_id("00000000-0000-7000-8000-000000000309");
    let other_id = user_id("00000000-0000-7000-8000-000000000310");
    let action = action_plan(
        ControlPlaneOperation::ScimUserUpdate,
        ResourceKind::User,
        Some(&id.to_string()),
    );
    let current = record(id, "alice@example.com");
    let other = record(other_id, "bob@example.com");
    let post_denied = plan(
        &action,
        &scoped_governance(),
        std::slice::from_ref(&current),
        ApplicationGatewayScimUserMutation::Update {
            id,
            mode: ApplicationScimUserUpdateMode::Patch,
            patch: ApplicationGatewayScimUserPatch {
                team_id: ApplicationPatchValue::Set("team-b".to_string()),
                ..Default::default()
            },
        },
    );
    assert_eq!(
        post_denied.unwrap_err(),
        ApplicationGatewayIdentityMutationError::GovernanceDenied
    );

    let duplicate = plan(
        &action,
        &scoped_governance(),
        &[current, other],
        ApplicationGatewayScimUserMutation::Update {
            id,
            mode: ApplicationScimUserUpdateMode::Patch,
            patch: ApplicationGatewayScimUserPatch {
                user_name: ApplicationPatchValue::Set("BOB@example.com".to_string()),
                ..Default::default()
            },
        },
    );
    assert_eq!(
        duplicate.unwrap_err(),
        ApplicationGatewayIdentityMutationError::DuplicateName
    );

    let mut outside = record(id, "alice@example.com");
    outside.team_id = Some("team-b".to_string());
    assert_eq!(
        plan(
            &action,
            &scoped_governance(),
            &[outside],
            ApplicationGatewayScimUserMutation::Update {
                id,
                mode: ApplicationScimUserUpdateMode::Patch,
                patch: ApplicationGatewayScimUserPatch::default(),
            },
        )
        .unwrap_err(),
        ApplicationGatewayIdentityMutationError::GovernanceDenied,
    );
}

#[test]
fn roles_prefixes_and_scope_values_are_validated() {
    let id = user_id("00000000-0000-7000-8000-000000000311");
    let action = action_plan(
        ControlPlaneOperation::ScimUserUpdate,
        ResourceKind::User,
        Some(&id.to_string()),
    );
    for (patch, expected) in [
        (
            ApplicationGatewayScimUserPatch {
                role: ApplicationPatchValue::Set("owner".to_string()),
                ..Default::default()
            },
            ApplicationGatewayIdentityMutationError::InvalidScimRole,
        ),
        (
            ApplicationGatewayScimUserPatch {
                tenant_id: ApplicationPatchValue::Set(" tenant-a ".to_string()),
                ..Default::default()
            },
            ApplicationGatewayIdentityMutationError::InvalidScopeValue,
        ),
        (
            ApplicationGatewayScimUserPatch {
                allowed_key_prefixes: ApplicationPatchValue::Set(vec![
                    "team-a-".to_string(),
                    "TEAM-A-".to_string(),
                ]),
                ..Default::default()
            },
            ApplicationGatewayIdentityMutationError::DuplicateKeyPrefix,
        ),
    ] {
        assert_eq!(
            plan(
                &action,
                &ApplicationControlPlaneGovernanceScope::default(),
                &[record(id, "alice@example.com")],
                ApplicationGatewayScimUserMutation::Update {
                    id,
                    mode: ApplicationScimUserUpdateMode::Patch,
                    patch,
                },
            )
            .unwrap_err(),
            expected,
        );
    }
}

#[test]
fn debug_output_redacts_scim_identity_and_governance_fields() {
    let id = user_id("00000000-0000-7000-8000-000000000312");
    let action = action_plan(
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        None,
    );
    let governance = ApplicationControlPlaneGovernanceScope::new(
        Some("tenant-debug-secret".to_string()),
        None,
        None,
        None,
        None,
        vec!["prefix-debug-secret".to_string()],
    );
    let request = ApplicationGatewayScimUserMutationRequest {
        authorized_action: &action,
        governance: &governance,
        current_records: &[],
        mutation: ApplicationGatewayScimUserMutation::Create {
            id,
            patch: create_patch("user-debug-secret@example.com"),
        },
        now_unix_ms: NOW,
    };
    let rendered = format!("{request:?}");
    for secret in [
        "tenant-debug-secret",
        "prefix-debug-secret",
        "user-debug-secret@example.com",
    ] {
        assert!(!rendered.contains(secret), "{rendered}");
    }
}
