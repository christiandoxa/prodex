use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_virtual_key_store_load,
};
use super::local_rewrite_gateway_admin_audit::runtime_gateway_audit_admin_authorization_denied_event;
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_execution::{
    RuntimeGatewayAdminMutationExecution, runtime_gateway_admin_mutation_execution,
};
use super::local_rewrite_gateway_admin_identity::{
    RuntimeGatewayIdentityKind, runtime_gateway_apply_scim_user_projection,
    runtime_gateway_identity_error, runtime_gateway_scim_create_mutation,
    runtime_gateway_scim_delete_mutation, runtime_gateway_scim_update_mutation,
    runtime_gateway_scim_user_record,
};
use super::local_rewrite_gateway_admin_response::{
    RuntimeGatewayAdminError, runtime_gateway_admin_json_body, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_store_mutation::runtime_gateway_mutate_admin_key_store_atomic;
use super::local_rewrite_gateway_scim::{
    runtime_gateway_scim_user_etag, runtime_gateway_scim_user_json,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayVirtualKeyStoreFile,
};
use super::*;
use prodex_application::{
    ApplicationControlPlaneGovernanceScope, ApplicationGatewayScimUserMutation,
    ApplicationGatewayScimUserMutationRequest, plan_application_gateway_scim_user_mutation,
};
use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::PrincipalId;

const RUNTIME_GATEWAY_SCIM_LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";

pub(super) fn runtime_gateway_admin_scim_list_users_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let store = runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    );
    let resources = store
        .scim_users
        .iter()
        .filter(|user| admin_auth.can_access_scim_user(user))
        .map(|user| runtime_gateway_scim_user_json(user, shared))
        .collect::<Vec<_>>();
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "schemas": [RUNTIME_GATEWAY_SCIM_LIST_SCHEMA],
            "totalResults": resources.len(),
            "startIndex": 1,
            "itemsPerPage": resources.len(),
            "Resources": resources,
        }),
    )
}

pub(super) fn runtime_gateway_admin_scim_create_user_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    let path = path_without_query(&captured.path_and_query);
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        path,
        admin_auth,
        base_action,
        ControlPlaneOperation::ScimUserCreate,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let mutation = match runtime_gateway_scim_create_mutation(PrincipalId::new(), &body) {
        Ok(mutation) => mutation,
        Err(error) => return error.into_response(),
    };
    let mut committed = None;
    match runtime_gateway_execute_scim_mutation(
        shared,
        admin_auth,
        execution,
        mutation,
        &mut committed,
    ) {
        Ok(()) => match committed {
            Some(user) => runtime_gateway_admin_json_response(
                201,
                runtime_gateway_scim_user_json(&user, shared),
            ),
            None => runtime_gateway_scim_commit_missing_error().into_response(),
        },
        Err(response) => response,
    }
}

pub(super) fn runtime_gateway_admin_scim_get_user_response(
    id: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let store = runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    );
    let Some(user) = store.scim_users.iter().find(|user| user.id == id) else {
        return runtime_gateway_scim_not_found_error().into_response();
    };
    if !admin_auth.can_access_scim_user(user) {
        return runtime_gateway_admin_scim_scope_forbidden_response(
            shared,
            admin_auth,
            "get_scim_user",
            &user.id,
        );
    }
    runtime_gateway_admin_json_response(200, runtime_gateway_scim_user_json(user, shared))
}

pub(super) fn runtime_gateway_admin_scim_update_user_response(
    id: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    let path = path_without_query(&captured.path_and_query);
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        path,
        admin_auth,
        base_action,
        ControlPlaneOperation::ScimUserUpdate,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let id = match runtime_gateway_scim_principal_id(id) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let mode = if captured.method.eq_ignore_ascii_case("PUT") {
        prodex_application::ApplicationScimUserUpdateMode::Replace
    } else {
        prodex_application::ApplicationScimUserUpdateMode::Patch
    };
    let mutation = match runtime_gateway_scim_update_mutation(id, mode, &body) {
        Ok(mutation) => mutation,
        Err(error) => return error.into_response(),
    };
    let mut committed = None;
    match runtime_gateway_execute_scim_mutation(
        shared,
        admin_auth,
        execution,
        mutation,
        &mut committed,
    ) {
        Ok(()) => match committed {
            Some(user) => runtime_gateway_admin_json_response(
                200,
                runtime_gateway_scim_user_json(&user, shared),
            ),
            None => runtime_gateway_scim_commit_missing_error().into_response(),
        },
        Err(response) => response,
    }
}

pub(super) fn runtime_gateway_admin_scim_delete_user_response(
    id: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    let path = path_without_query(&captured.path_and_query);
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        path,
        admin_auth,
        base_action,
        ControlPlaneOperation::ScimUserDelete,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let id = match runtime_gateway_scim_principal_id(id) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };
    let mut committed = None;
    match runtime_gateway_execute_scim_mutation(
        shared,
        admin_auth,
        execution,
        runtime_gateway_scim_delete_mutation(id),
        &mut committed,
    ) {
        Ok(()) => match committed {
            Some(user) => runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.scim_user.deleted",
                    "id": user.id,
                    "deleted": true,
                }),
            ),
            None => runtime_gateway_scim_commit_missing_error().into_response(),
        },
        Err(response) => response,
    }
}

fn runtime_gateway_execute_scim_mutation(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    execution: RuntimeGatewayAdminMutationExecution,
    mutation: ApplicationGatewayScimUserMutation,
    committed: &mut Option<RuntimeGatewayScimUser>,
) -> Result<(), tiny_http::ResponseBox> {
    let RuntimeGatewayAdminMutationExecution {
        authorized_action,
        governance,
        atomic_write,
        entity_tag,
    } = execution;
    runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let user = runtime_gateway_plan_and_apply_scim_mutation(
            store,
            &authorized_action,
            &governance,
            mutation,
            entity_tag.as_ref(),
            shared,
            admin_auth,
        )?;
        *committed = Some(user);
        Ok(())
    })
}

fn runtime_gateway_plan_and_apply_scim_mutation(
    store: &mut RuntimeGatewayVirtualKeyStoreFile,
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    mutation: ApplicationGatewayScimUserMutation,
    entity_tag: Option<&prodex_domain::EntityTag>,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Result<RuntimeGatewayScimUser, RuntimeGatewayAdminError> {
    let (audit_action, audit_resource_id) = match &mutation {
        ApplicationGatewayScimUserMutation::Create { id, .. } => {
            ("create_scim_user", id.to_string())
        }
        ApplicationGatewayScimUserMutation::Update { id, .. } => {
            ("update_scim_user", id.to_string())
        }
        ApplicationGatewayScimUserMutation::Delete { id } => ("delete_scim_user", id.to_string()),
    };
    enforce_scim_entity_tag(entity_tag, store, &mutation).inspect_err(|_| {
        runtime_gateway_audit_admin_authorization_denied_event(
            shared,
            admin_auth,
            "scim_user",
            audit_action,
            &audit_resource_id,
        );
    })?;
    let current_records = store
        .scim_users
        .iter()
        .map(runtime_gateway_scim_user_record)
        .collect::<Result<Vec<_>, _>>()?;
    let plan =
        plan_application_gateway_scim_user_mutation(ApplicationGatewayScimUserMutationRequest {
            authorized_action,
            governance,
            current_records: &current_records,
            mutation,
            now_unix_ms: authorized_action.audit_event.occurred_at_unix_ms,
        })
        .map_err(|error| {
            if matches!(
            error,
            prodex_application::ApplicationGatewayIdentityMutationError::GovernanceDenied
                | prodex_application::ApplicationGatewayIdentityMutationError::TenantMismatch
                | prodex_application::ApplicationGatewayIdentityMutationError::ResourceIdMismatch
        ) {
                runtime_gateway_audit_admin_authorization_denied_event(
                    shared,
                    admin_auth,
                    "scim_user",
                    audit_action,
                    &audit_resource_id,
                );
            }
            runtime_gateway_identity_error(RuntimeGatewayIdentityKind::ScimUser, error)
        })?;
    runtime_gateway_apply_scim_user_projection(store, &plan)
}

fn enforce_scim_entity_tag(
    expected: Option<&prodex_domain::EntityTag>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
    mutation: &ApplicationGatewayScimUserMutation,
) -> Result<(), RuntimeGatewayAdminError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let id = match mutation {
        ApplicationGatewayScimUserMutation::Create { .. } => return Ok(()),
        ApplicationGatewayScimUserMutation::Update { id, .. }
        | ApplicationGatewayScimUserMutation::Delete { id } => id.to_string(),
    };
    let Some(current) = store.scim_users.iter().find(|user| user.id == id) else {
        return Ok(());
    };
    if expected.as_str() == "*" || expected.as_str() == runtime_gateway_scim_user_etag(current) {
        return Ok(());
    }
    Err(RuntimeGatewayAdminError::new(
        412,
        "precondition_failed",
        "If-Match does not match the current SCIM user ETag",
    ))
}

fn runtime_gateway_scim_principal_id(id: &str) -> Result<PrincipalId, RuntimeGatewayAdminError> {
    id.parse::<PrincipalId>().map_err(|_| {
        RuntimeGatewayAdminError::new(
            400,
            "gateway_scim_user_id_invalid",
            "gateway SCIM user id must be a valid principal identifier",
        )
    })
}

fn runtime_gateway_scim_not_found_error() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        404,
        "gateway_scim_user_not_found",
        "gateway SCIM user was not found",
    )
}

fn runtime_gateway_scim_commit_missing_error() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        500,
        "gateway_scim_mutation_commit_missing",
        "gateway SCIM mutation did not produce a committed projection",
    )
}

fn runtime_gateway_admin_scim_scope_forbidden_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    action: &'static str,
    user_id: &str,
) -> tiny_http::ResponseBox {
    runtime_gateway_audit_admin_authorization_denied_event(
        shared,
        admin_auth,
        "scim_user",
        action,
        user_id,
    );
    RuntimeGatewayAdminError::new(
        403,
        "gateway_admin_key_scope_forbidden",
        "gateway admin token is not allowed to access this tenant",
    )
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scim_entity_tag_rejects_stale_mutation() {
        let id = PrincipalId::new();
        let store = RuntimeGatewayVirtualKeyStoreFile {
            scim_users: vec![RuntimeGatewayScimUser {
                id: id.to_string(),
                user_name: "user@example.com".to_string(),
                external_id: None,
                display_name: None,
                active: true,
                role: None,
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                allowed_key_prefixes: Vec::new(),
                created_at_epoch: 10,
                updated_at_epoch: 20,
            }],
            ..RuntimeGatewayVirtualKeyStoreFile::default()
        };
        let mutation = ApplicationGatewayScimUserMutation::Delete { id };
        let current = prodex_domain::EntityTag::new("\"gateway-scim-user-20\"").unwrap();
        assert!(enforce_scim_entity_tag(Some(&current), &store, &mutation).is_ok());

        let stale = prodex_domain::EntityTag::new("\"gateway-scim-user-19\"").unwrap();
        let error = enforce_scim_entity_tag(Some(&stale), &store, &mutation).unwrap_err();
        assert_eq!(error.test_status(), 412);
        assert_eq!(error.test_code(), "precondition_failed");
    }

    #[test]
    fn scim_mutations_have_one_application_and_atomic_write_path() {
        let source = include_str!("local_rewrite_gateway_admin_scim.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(
            production
                .matches("plan_application_gateway_scim_user_mutation(")
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("runtime_gateway_mutate_admin_key_store_atomic(")
                .count(),
            1
        );
        for forbidden in [
            "runtime_gateway_mutate_admin_key_store(",
            "plan_application_user_lifecycle(",
            "runtime_gateway_audit_admin_scim_user_event(",
            "sha256:scim-user-",
            "runtime_gateway_apply_scim_user_patch(",
        ] {
            assert!(
                !production.contains(forbidden),
                "legacy SCIM path remains: {forbidden}"
            );
        }
    }
}
