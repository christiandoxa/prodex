use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_virtual_key_store_load,
};
use super::local_rewrite_gateway_admin_audit::{
    runtime_gateway_audit_admin_authorization_denied_event,
    runtime_gateway_audit_admin_scim_user_event,
};
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_response::{
    RuntimeGatewayAdminError, runtime_gateway_admin_json_body, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_store_mutation::runtime_gateway_mutate_admin_key_store;
use super::local_rewrite_gateway_scim::{
    runtime_gateway_apply_scim_user_patch, runtime_gateway_generate_scim_user_id,
    runtime_gateway_scim_user_json,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayVirtualKeyStoreFile,
};
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_seconds;
use super::*;
use prodex_application::{
    ApplicationUserLifecycleErrorStatus, ApplicationUserLifecycleRequest,
    plan_application_user_lifecycle, plan_application_user_lifecycle_error_response,
};
use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneOperation, ControlPlaneResourceRef,
};
use prodex_domain::{
    AuditDigest, CredentialScope, Principal, PrincipalId, PrincipalKind, Role, TenantId,
};
use prodex_storage::{DurableStoreKind, TenantStorageKey, UserLifecycleCommand, UserLifecycleKind};
use sha2::{Digest, Sha256};

const RUNTIME_GATEWAY_SCIM_LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
const RUNTIME_GATEWAY_SCIM_USER_GENERATION_FAILED_MESSAGE: &str =
    "gateway SCIM user id could not be generated";
const RUNTIME_GATEWAY_SCIM_USER_TENANT_REQUIRED_MESSAGE: &str =
    "gateway SCIM user tenant_id is required for durable user storage";
const RUNTIME_GATEWAY_SCIM_USER_TENANT_INVALID_MESSAGE: &str =
    "gateway SCIM user tenant_id must be a valid tenant identifier";
const RUNTIME_GATEWAY_SCIM_USER_ID_REQUIRED_MESSAGE: &str =
    "gateway SCIM user user_id is required for durable user storage";
const RUNTIME_GATEWAY_SCIM_USER_ID_INVALID_MESSAGE: &str =
    "gateway SCIM user user_id must be a valid principal identifier";

#[derive(Debug)]
enum RuntimeGatewayScimUpdateMutationError {
    Admin(RuntimeGatewayAdminError),
    ScopeForbidden { user_id: String },
}

impl From<RuntimeGatewayAdminError> for RuntimeGatewayScimUpdateMutationError {
    fn from(err: RuntimeGatewayAdminError) -> Self {
        Self::Admin(err)
    }
}

fn runtime_gateway_admin_scim_user_generation_failed_error() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        500,
        "gateway_scim_user_generation_failed",
        RUNTIME_GATEWAY_SCIM_USER_GENERATION_FAILED_MESSAGE,
    )
}

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
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let now = runtime_gateway_unix_epoch_seconds();
    let id = match runtime_gateway_generate_scim_user_id() {
        Ok(id) => id,
        Err(_err) => {
            return runtime_gateway_admin_scim_user_generation_failed_error().into_response();
        }
    };
    let mut user = RuntimeGatewayScimUser {
        id,
        user_name: String::new(),
        external_id: None,
        display_name: None,
        active: true,
        role: None,
        tenant_id: admin_auth.tenant_id.clone(),
        team_id: admin_auth.team_id.clone(),
        project_id: admin_auth.project_id.clone(),
        user_id: admin_auth.user_id.clone(),
        budget_id: admin_auth.budget_id.clone(),
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: now,
        updated_at_epoch: now,
    };
    if let Err(err) = runtime_gateway_apply_scim_user_patch(&mut user, &body, false) {
        return err.into_response();
    }
    if user.tenant_id.is_none() {
        user.tenant_id = admin_auth.tenant_id.clone();
    }
    if user.team_id.is_none() {
        user.team_id = admin_auth.team_id.clone();
    }
    if user.project_id.is_none() {
        user.project_id = admin_auth.project_id.clone();
    }
    runtime_gateway_admin_scim_default_target_user_id(&mut user, admin_auth);
    if user.budget_id.is_none() {
        user.budget_id = admin_auth.budget_id.clone();
    }
    if !admin_auth.can_access_scim_user(&user) {
        return runtime_gateway_admin_scim_scope_forbidden_response(
            shared,
            admin_auth,
            "create_scim_user",
            &user.id,
        );
    }
    if let Some(response) = runtime_gateway_admin_scim_user_lifecycle_response(
        shared,
        admin_auth,
        &user,
        UserLifecycleKind::Create,
    ) {
        return response;
    }
    let user_name = user.user_name.clone();
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        if store
            .scim_users
            .iter()
            .any(|stored| stored.user_name.eq_ignore_ascii_case(&user_name))
        {
            return Err(RuntimeGatewayAdminError::new(
                409,
                "gateway_scim_user_exists",
                "gateway SCIM userName already exists",
            ));
        }
        store.scim_users.push(user.clone());
        Ok(())
    }) {
        Ok(()) => {
            runtime_gateway_audit_admin_scim_user_event(
                shared,
                "create_scim_user",
                "success",
                &user,
            );
            runtime_gateway_admin_json_response(201, runtime_gateway_scim_user_json(&user, shared))
        }
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
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_scim_user_not_found",
            "gateway SCIM user was not found",
        );
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
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let partial = !captured.method.eq_ignore_ascii_case("PUT");
    let store = runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    );
    let Some(current_user) = store.scim_users.iter().find(|user| user.id == id).cloned() else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_scim_user_not_found",
            "gateway SCIM user was not found",
        );
    };
    if !admin_auth.can_access_scim_user(&current_user) {
        return runtime_gateway_admin_scim_scope_forbidden_response(
            shared,
            admin_auth,
            "update_scim_user",
            &current_user.id,
        );
    }
    let mut planned_user = current_user.clone();
    if let Err(err) = runtime_gateway_apply_scim_user_patch(&mut planned_user, &body, partial) {
        return err.into_response();
    }
    if !admin_auth.can_access_scim_user(&planned_user) {
        return runtime_gateway_admin_scim_scope_forbidden_response(
            shared,
            admin_auth,
            "update_scim_user",
            &planned_user.id,
        );
    }
    if let Some(response) = runtime_gateway_admin_scim_user_lifecycle_response(
        shared,
        admin_auth,
        &planned_user,
        UserLifecycleKind::Update,
    ) {
        return response;
    }
    let mut updated = None;
    let update_result = runtime_gateway_mutate_admin_key_store(shared, |store| {
        match runtime_gateway_apply_authorized_scim_user_update(
            store, id, &body, partial, admin_auth,
        ) {
            Ok(user) => {
                updated = Some(user);
                Ok(())
            }
            Err(RuntimeGatewayScimUpdateMutationError::ScopeForbidden { user_id }) => {
                Err(runtime_gateway_admin_scim_scope_forbidden_error(
                    shared,
                    admin_auth,
                    "update_scim_user",
                    &user_id,
                ))
            }
            Err(RuntimeGatewayScimUpdateMutationError::Admin(err)) => Err(err),
        }
    });
    match update_result {
        Ok(()) => {
            if let Some(user) = updated {
                runtime_gateway_audit_admin_scim_user_event(
                    shared,
                    "update_scim_user",
                    "success",
                    &user,
                );
                runtime_gateway_admin_json_response(
                    200,
                    runtime_gateway_scim_user_json(&user, shared),
                )
            } else {
                build_runtime_proxy_json_error_response(
                    404,
                    "gateway_scim_user_not_found",
                    "gateway SCIM user was not found",
                )
            }
        }
        Err(response) => response,
    }
}

fn runtime_gateway_apply_authorized_scim_user_update(
    store: &mut RuntimeGatewayVirtualKeyStoreFile,
    id: &str,
    body: &serde_json::Value,
    partial: bool,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Result<RuntimeGatewayScimUser, RuntimeGatewayScimUpdateMutationError> {
    let Some(index) = store.scim_users.iter().position(|user| user.id == id) else {
        return Err(RuntimeGatewayScimUpdateMutationError::Admin(
            RuntimeGatewayAdminError::new(
                404,
                "gateway_scim_user_not_found",
                "gateway SCIM user was not found",
            ),
        ));
    };
    let current_user = &store.scim_users[index];
    if !admin_auth.can_access_scim_user(current_user) {
        return Err(RuntimeGatewayScimUpdateMutationError::ScopeForbidden {
            user_id: current_user.id.clone(),
        });
    }
    let previous_name = current_user.user_name.clone();
    let mut planned_user = current_user.clone();
    runtime_gateway_apply_scim_user_patch(&mut planned_user, body, partial)?;
    if !admin_auth.can_access_scim_user(&planned_user) {
        return Err(RuntimeGatewayScimUpdateMutationError::ScopeForbidden {
            user_id: planned_user.id.clone(),
        });
    }
    let next_name = planned_user.user_name.clone();
    if !previous_name.eq_ignore_ascii_case(&next_name)
        && store
            .scim_users
            .iter()
            .enumerate()
            .any(|(other, user)| other != index && user.user_name.eq_ignore_ascii_case(&next_name))
    {
        return Err(RuntimeGatewayScimUpdateMutationError::Admin(
            RuntimeGatewayAdminError::new(
                409,
                "gateway_scim_user_exists",
                "gateway SCIM userName already exists",
            ),
        ));
    }
    planned_user.updated_at_epoch = runtime_gateway_unix_epoch_seconds();
    store.scim_users[index] = planned_user.clone();
    Ok(planned_user)
}

pub(super) fn runtime_gateway_admin_scim_delete_user_response(
    id: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let store = runtime_gateway_virtual_key_store_load(
        &shared.gateway_state_store,
        &shared.runtime_shared.log_path,
    );
    let Some(existing_user) = store.scim_users.iter().find(|user| user.id == id).cloned() else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_scim_user_not_found",
            "gateway SCIM user was not found",
        );
    };
    if !admin_auth.can_access_scim_user(&existing_user) {
        return runtime_gateway_admin_scim_scope_forbidden_response(
            shared,
            admin_auth,
            "delete_scim_user",
            &existing_user.id,
        );
    }
    if let Some(response) = runtime_gateway_admin_scim_user_lifecycle_response(
        shared,
        admin_auth,
        &existing_user,
        UserLifecycleKind::Delete,
    ) {
        return response;
    }
    let mut deleted = None;
    let mut forbidden = None;
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        let before = store.scim_users.len();
        for user in &store.scim_users {
            if user.id != id {
                continue;
            }
            if !admin_auth.can_access_scim_user(user) {
                forbidden = Some(user.id.clone());
                break;
            }
            deleted = Some(user.clone());
        }
        store
            .scim_users
            .retain(|user| user.id != id || deleted.is_none());
        if let Some(user_id) = forbidden.as_deref() {
            return Err(runtime_gateway_admin_scim_scope_forbidden_error(
                shared,
                admin_auth,
                "delete_scim_user",
                user_id,
            ));
        }
        if store.scim_users.len() == before {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_scim_user_not_found",
                "gateway SCIM user was not found",
            ));
        }
        Ok(())
    }) {
        Ok(()) => {
            if let Some(user) = deleted {
                runtime_gateway_audit_admin_scim_user_event(
                    shared,
                    "delete_scim_user",
                    "success",
                    &user,
                );
            }
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.scim_user.deleted",
                    "id": id,
                    "deleted": true,
                }),
            )
        }
        Err(response) => response,
    }
}

fn runtime_gateway_admin_scim_scope_forbidden_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    action: &'static str,
    user_id: &str,
) -> tiny_http::ResponseBox {
    runtime_gateway_admin_scim_scope_forbidden_error(shared, admin_auth, action, user_id)
        .into_response()
}

fn runtime_gateway_admin_scim_scope_forbidden_error(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    action: &'static str,
    user_id: &str,
) -> RuntimeGatewayAdminError {
    runtime_gateway_audit_admin_authorization_denied_event(
        shared,
        &admin_auth.name,
        admin_auth.role.as_str(),
        "scim_user",
        action,
        user_id,
    );
    runtime_gateway_admin_scim_scope_forbidden_admin_error()
}

fn runtime_gateway_admin_scim_scope_forbidden_admin_error() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        403,
        "gateway_admin_key_scope_forbidden",
        "gateway admin token is not allowed to access this tenant",
    )
}

fn runtime_gateway_admin_scim_user_lifecycle_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    user: &RuntimeGatewayScimUser,
    kind: UserLifecycleKind,
) -> Option<tiny_http::ResponseBox> {
    let durable_store = match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Postgres { .. } => DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::Sqlite { .. } => DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return None;
        }
    };
    let operation = match kind {
        UserLifecycleKind::Create => ControlPlaneOperation::ScimUserCreate,
        UserLifecycleKind::Update => ControlPlaneOperation::ScimUserUpdate,
        UserLifecycleKind::Delete => ControlPlaneOperation::ScimUserDelete,
    };
    let tenant_id = match runtime_gateway_admin_scim_durable_tenant_id(user) {
        Ok(tenant_id) => tenant_id,
        Err(err) => return Some(err.into_response()),
    };
    let principal_id = match runtime_gateway_admin_scim_durable_principal_id(user) {
        Ok(principal_id) => principal_id,
        Err(err) => return Some(err.into_response()),
    };
    let request = ApplicationUserLifecycleRequest {
        durable_store,
        action: ControlPlaneActionRequest {
            principal: Principal::new(
                runtime_gateway_admin_scim_actor_principal_id(admin_auth, tenant_id),
                Some(tenant_id),
                PrincipalKind::User,
                runtime_gateway_admin_scim_domain_role(admin_auth.role),
                CredentialScope::ControlPlane,
            ),
            operation,
            resource: ControlPlaneResourceRef::new(
                tenant_id,
                prodex_domain::ResourceKind::User,
                None::<String>,
            ),
            occurred_at_unix_ms: user.updated_at_epoch.saturating_mul(1000),
        },
        user: UserLifecycleCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            principal_id,
            external_id: user.external_id.clone().unwrap_or_else(|| user.id.clone()),
            display_name: user
                .display_name
                .clone()
                .unwrap_or_else(|| user.user_name.clone()),
            kind,
            occurred_at_unix_ms: user.updated_at_epoch.saturating_mul(1000),
        },
        previous_digest: None,
        event_digest: runtime_gateway_admin_scim_event_digest(kind),
    };
    match plan_application_user_lifecycle(request) {
        Ok(_) => None,
        Err(error) => {
            let response = plan_application_user_lifecycle_error_response(&error);
            Some(build_runtime_proxy_json_error_response(
                runtime_gateway_application_user_lifecycle_status_code(response.status),
                response.code,
                response.message,
            ))
        }
    }
}

fn runtime_gateway_admin_scim_durable_tenant_id(
    user: &RuntimeGatewayScimUser,
) -> Result<TenantId, RuntimeGatewayAdminError> {
    let Some(tenant_id) = user.tenant_id.as_deref() else {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "gateway_scim_user_tenant_required",
            RUNTIME_GATEWAY_SCIM_USER_TENANT_REQUIRED_MESSAGE,
        ));
    };
    tenant_id.parse::<TenantId>().map_err(|_| {
        RuntimeGatewayAdminError::new(
            400,
            "gateway_scim_user_tenant_invalid",
            RUNTIME_GATEWAY_SCIM_USER_TENANT_INVALID_MESSAGE,
        )
    })
}

fn runtime_gateway_admin_scim_actor_principal_id(
    admin_auth: &RuntimeGatewayAdminAuth,
    tenant_id: TenantId,
) -> PrincipalId {
    let mut hasher = Sha256::new();
    hasher.update(b"prodex:gateway-admin-scim-actor:v1");
    hasher.update(tenant_id.to_string().as_bytes());
    hasher.update([0]);
    hasher.update(admin_auth.name.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    PrincipalId::from_uuid(uuid::Uuid::from_bytes(bytes))
}

fn runtime_gateway_admin_scim_durable_principal_id(
    user: &RuntimeGatewayScimUser,
) -> Result<PrincipalId, RuntimeGatewayAdminError> {
    let Some(user_id) = user.user_id.as_deref() else {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "gateway_scim_user_id_required",
            RUNTIME_GATEWAY_SCIM_USER_ID_REQUIRED_MESSAGE,
        ));
    };
    user_id.parse::<PrincipalId>().map_err(|_| {
        RuntimeGatewayAdminError::new(
            400,
            "gateway_scim_user_id_invalid",
            RUNTIME_GATEWAY_SCIM_USER_ID_INVALID_MESSAGE,
        )
    })
}

fn runtime_gateway_admin_scim_default_target_user_id(
    user: &mut RuntimeGatewayScimUser,
    admin_auth: &RuntimeGatewayAdminAuth,
) {
    if user.user_id.is_none() {
        user.user_id = admin_auth.user_id.clone().or_else(|| {
            user.id
                .parse::<PrincipalId>()
                .ok()
                .map(|principal_id| principal_id.to_string())
        });
    }
}

fn runtime_gateway_admin_scim_event_digest(kind: UserLifecycleKind) -> AuditDigest {
    let value = match kind {
        UserLifecycleKind::Create => "sha256:scim-user-create",
        UserLifecycleKind::Update => "sha256:scim-user-update",
        UserLifecycleKind::Delete => "sha256:scim-user-delete",
    };
    AuditDigest::new(value).expect("static SCIM lifecycle audit digest should be valid")
}

fn runtime_gateway_admin_scim_domain_role(role: RuntimeGatewayAdminRole) -> Role {
    match role {
        RuntimeGatewayAdminRole::Admin => Role::Admin,
        RuntimeGatewayAdminRole::Viewer => Role::Viewer,
    }
}

fn runtime_gateway_application_user_lifecycle_status_code(
    status: ApplicationUserLifecycleErrorStatus,
) -> u16 {
    match status {
        ApplicationUserLifecycleErrorStatus::BadRequest => 400,
        ApplicationUserLifecycleErrorStatus::ServiceUnavailable => 503,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_scim_user_generation_error_uses_stable_redacted_message() {
        let err = runtime_gateway_admin_scim_user_generation_failed_error();
        assert_eq!(err.test_status(), 500);
        assert_eq!(err.test_code(), "gateway_scim_user_generation_failed");
        assert_eq!(
            err.test_message(),
            "gateway SCIM user id could not be generated"
        );
        assert!(!err.test_message().contains("getrandom"));
        assert!(
            !err.test_message()
                .contains("failed to generate gateway virtual key")
        );
    }

    fn scim_user_with_tenant(tenant_id: Option<String>) -> RuntimeGatewayScimUser {
        scim_user_with_ids(tenant_id, None)
    }

    fn scim_user_with_ids(
        tenant_id: Option<String>,
        user_id: Option<String>,
    ) -> RuntimeGatewayScimUser {
        RuntimeGatewayScimUser {
            id: "user_1".to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: None,
            tenant_id,
            team_id: None,
            project_id: None,
            user_id,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        }
    }

    fn admin_auth_named(name: &str) -> RuntimeGatewayAdminAuth {
        RuntimeGatewayAdminAuth {
            name: name.to_string(),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }
    }

    #[test]
    fn scim_in_transaction_update_rejects_foreign_authoritative_preimage_without_write() {
        let tenant_a = TenantId::new().to_string();
        let tenant_b = TenantId::new().to_string();
        let mut admin = admin_auth_named("tenant-a-admin");
        admin.tenant_id = Some(tenant_a.clone());
        let mut foreign_user = scim_user_with_tenant(Some(tenant_b));
        foreign_user.display_name = Some("Tenant B User".to_string());
        let mut store = RuntimeGatewayVirtualKeyStoreFile {
            scim_users: vec![foreign_user],
            ..RuntimeGatewayVirtualKeyStoreFile::default()
        };
        let before = serde_json::to_value(&store).expect("SCIM store should serialize");

        let err = runtime_gateway_apply_authorized_scim_user_update(
            &mut store,
            "user_1",
            &serde_json::json!({
                "tenant_id": tenant_a,
                "displayName": "Unauthorized Tenant Takeover"
            }),
            true,
            &admin,
        )
        .expect_err("foreign authoritative pre-image must be rejected");

        assert!(matches!(
            err,
            RuntimeGatewayScimUpdateMutationError::ScopeForbidden { ref user_id }
                if user_id == "user_1"
        ));
        assert_eq!(
            serde_json::to_value(&store).expect("SCIM store should serialize"),
            before,
            "scope denial must not mutate the authoritative store"
        );
        let stable = runtime_gateway_admin_scim_scope_forbidden_admin_error();
        assert_eq!(stable.test_status(), 403);
        assert_eq!(stable.test_code(), "gateway_admin_key_scope_forbidden");
    }

    #[test]
    fn durable_scim_lifecycle_rejects_missing_or_invalid_tenant_id() {
        let missing = runtime_gateway_admin_scim_durable_tenant_id(&scim_user_with_tenant(None))
            .expect_err("missing tenant should fail closed");
        assert_eq!(missing.test_status(), 400);
        assert_eq!(missing.test_code(), "gateway_scim_user_tenant_required");
        assert_eq!(
            missing.test_message(),
            RUNTIME_GATEWAY_SCIM_USER_TENANT_REQUIRED_MESSAGE
        );

        let invalid = runtime_gateway_admin_scim_durable_tenant_id(&scim_user_with_tenant(Some(
            "tenant-a".to_string(),
        )))
        .expect_err("non-UUID tenant should fail closed");
        assert_eq!(invalid.test_status(), 400);
        assert_eq!(invalid.test_code(), "gateway_scim_user_tenant_invalid");
        assert_eq!(
            invalid.test_message(),
            RUNTIME_GATEWAY_SCIM_USER_TENANT_INVALID_MESSAGE
        );

        let valid_tenant_id = TenantId::new();
        let parsed = runtime_gateway_admin_scim_durable_tenant_id(&scim_user_with_tenant(Some(
            valid_tenant_id.to_string(),
        )));
        assert!(matches!(parsed, Ok(parsed) if parsed == valid_tenant_id));
    }

    #[test]
    fn durable_scim_lifecycle_rejects_missing_or_invalid_user_id() {
        let tenant_id = Some(TenantId::new().to_string());
        let missing = runtime_gateway_admin_scim_durable_principal_id(&scim_user_with_ids(
            tenant_id.clone(),
            None,
        ))
        .expect_err("missing user id should fail closed");
        assert_eq!(missing.test_status(), 400);
        assert_eq!(missing.test_code(), "gateway_scim_user_id_required");
        assert_eq!(
            missing.test_message(),
            RUNTIME_GATEWAY_SCIM_USER_ID_REQUIRED_MESSAGE
        );

        let invalid = runtime_gateway_admin_scim_durable_principal_id(&scim_user_with_ids(
            tenant_id,
            Some("user-a".to_string()),
        ))
        .expect_err("non-UUID user id should fail closed");
        assert_eq!(invalid.test_status(), 400);
        assert_eq!(invalid.test_code(), "gateway_scim_user_id_invalid");
        assert_eq!(
            invalid.test_message(),
            RUNTIME_GATEWAY_SCIM_USER_ID_INVALID_MESSAGE
        );

        let principal_id = PrincipalId::new();
        let parsed = runtime_gateway_admin_scim_durable_principal_id(&scim_user_with_ids(
            Some(TenantId::new().to_string()),
            Some(principal_id.to_string()),
        ));
        assert!(matches!(parsed, Ok(parsed) if parsed == principal_id));
    }

    #[test]
    fn scim_create_defaults_missing_user_id_to_generated_resource_principal_id() {
        let principal_id = PrincipalId::new();
        let expected = principal_id.to_string();
        let mut user = scim_user_with_ids(Some(TenantId::new().to_string()), None);
        user.id = expected.clone();

        runtime_gateway_admin_scim_default_target_user_id(&mut user, &admin_auth_named("admin"));

        assert_eq!(user.user_id, Some(expected));
    }

    #[test]
    fn durable_scim_lifecycle_actor_principal_id_is_stable_per_admin_and_tenant() {
        let actor_tenant_id = TenantId::new();
        let admin = admin_auth_named("admin-token");

        assert_eq!(
            runtime_gateway_admin_scim_actor_principal_id(&admin, actor_tenant_id),
            runtime_gateway_admin_scim_actor_principal_id(&admin, actor_tenant_id)
        );
        assert_ne!(
            runtime_gateway_admin_scim_actor_principal_id(&admin, actor_tenant_id),
            runtime_gateway_admin_scim_actor_principal_id(
                &admin_auth_named("other-admin"),
                actor_tenant_id
            )
        );
        assert_ne!(
            runtime_gateway_admin_scim_actor_principal_id(&admin, actor_tenant_id),
            runtime_gateway_admin_scim_actor_principal_id(&admin, TenantId::new())
        );
    }

    #[test]
    fn durable_scim_lifecycle_does_not_synthesize_tenant_or_user_ids() {
        let source = include_str!("local_rewrite_gateway_admin_scim.rs");
        for fallback in [
            ["let tenant_id = ", "TenantId::new();"].join(""),
            ["principal_id: ", "PrincipalId::new(),"].join(""),
        ] {
            assert!(!source.contains(&fallback));
        }
    }
}
