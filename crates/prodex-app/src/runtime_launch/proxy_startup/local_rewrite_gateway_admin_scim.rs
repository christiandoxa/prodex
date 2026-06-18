use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_virtual_key_store_load,
};
use super::local_rewrite_gateway_admin_audit::runtime_gateway_audit_admin_scim_user_event;
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_response::{
    RuntimeGatewayAdminError, runtime_gateway_admin_json_body, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_store_mutation::runtime_gateway_mutate_admin_key_store;
use super::local_rewrite_gateway_scim::{
    runtime_gateway_apply_scim_user_patch, runtime_gateway_generate_scim_user_id,
    runtime_gateway_scim_user_json,
};
use super::local_rewrite_gateway_store_types::RuntimeGatewayScimUser;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_seconds;
use super::*;

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
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let now = runtime_gateway_unix_epoch_seconds();
    let id = match runtime_gateway_generate_scim_user_id() {
        Ok(id) => id,
        Err(err) => {
            return build_runtime_proxy_json_error_response(
                500,
                "gateway_scim_user_generation_failed",
                &err.to_string(),
            );
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
    if user.user_id.is_none() {
        user.user_id = admin_auth.user_id.clone();
    }
    if user.budget_id.is_none() {
        user.budget_id = admin_auth.budget_id.clone();
    }
    if !admin_auth.can_access_scim_user(&user) {
        return runtime_gateway_admin_scim_scope_forbidden_response();
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
        return runtime_gateway_admin_scim_scope_forbidden_response();
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
    let mut updated = None;
    let update_result = runtime_gateway_mutate_admin_key_store(shared, |store| {
        let Some(index) = store.scim_users.iter().position(|user| user.id == id) else {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_scim_user_not_found",
                "gateway SCIM user was not found",
            ));
        };
        if !admin_auth.can_access_scim_user(&store.scim_users[index]) {
            return Err(RuntimeGatewayAdminError::new(
                403,
                "gateway_admin_key_scope_forbidden",
                "gateway admin token is not allowed to access this tenant",
            ));
        }
        let previous_name = store.scim_users[index].user_name.clone();
        runtime_gateway_apply_scim_user_patch(&mut store.scim_users[index], &body, partial)?;
        if !admin_auth.can_access_scim_user(&store.scim_users[index]) {
            return Err(RuntimeGatewayAdminError::new(
                403,
                "gateway_admin_key_scope_forbidden",
                "gateway admin token is not allowed to access this tenant",
            ));
        }
        let next_name = store.scim_users[index].user_name.clone();
        if !previous_name.eq_ignore_ascii_case(&next_name)
            && store.scim_users.iter().enumerate().any(|(other, user)| {
                other != index && user.user_name.eq_ignore_ascii_case(&next_name)
            })
        {
            return Err(RuntimeGatewayAdminError::new(
                409,
                "gateway_scim_user_exists",
                "gateway SCIM userName already exists",
            ));
        }
        store.scim_users[index].updated_at_epoch = runtime_gateway_unix_epoch_seconds();
        updated = Some(store.scim_users[index].clone());
        Ok(())
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

pub(super) fn runtime_gateway_admin_scim_delete_user_response(
    id: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let mut deleted = None;
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        let before = store.scim_users.len();
        store.scim_users.retain(|user| {
            if user.id == id {
                if !admin_auth.can_access_scim_user(user) {
                    return true;
                }
                deleted = Some(user.clone());
                false
            } else {
                true
            }
        });
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

fn runtime_gateway_admin_scim_scope_forbidden_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        403,
        "gateway_admin_key_scope_forbidden",
        "gateway admin token is not allowed to access this virtual key",
    )
}
