use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_audit::runtime_gateway_audit_admin_key_event;
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_fields::{
    runtime_gateway_admin_body_tenant_id, runtime_gateway_admin_key_patch_fields,
    runtime_gateway_admin_optional_dimension_from_body, runtime_gateway_admin_request_dimension,
    runtime_gateway_validate_virtual_key_name,
};
use super::local_rewrite_gateway_admin_response::{
    RuntimeGatewayAdminError, runtime_gateway_admin_json_body, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_store_mutation::runtime_gateway_mutate_admin_key_store;
use super::local_rewrite_gateway_key_patch::runtime_gateway_apply_virtual_key_patch;
use super::local_rewrite_gateway_key_payloads::{
    runtime_gateway_admin_key_json, runtime_gateway_admin_stored_key_json,
    runtime_gateway_virtual_key_entry_by_name, runtime_gateway_virtual_key_name_exists,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeySource,
};
use super::local_rewrite_gateway_util::{
    runtime_gateway_generate_virtual_key_token, runtime_gateway_unix_epoch_seconds,
};
use super::*;

pub(super) fn runtime_gateway_admin_get_key_response(
    name: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(name))
    else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        );
    };
    if !admin_auth.can_access_entry(entry) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    let usage = shared
        .gateway_usage
        .usage
        .lock()
        .ok()
        .and_then(|usage| usage.get(&entry.key.name).cloned());
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "object": "gateway.key",
            "key": runtime_gateway_admin_key_json(entry, usage),
        }),
    )
}

pub(super) fn runtime_gateway_admin_create_key_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let name = match body
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(name) => name.to_string(),
        None => {
            return build_runtime_proxy_json_error_response(
                400,
                "invalid_gateway_key_name",
                "gateway virtual key name is required",
            );
        }
    };
    if let Err(message) = runtime_gateway_validate_virtual_key_name(&name) {
        return build_runtime_proxy_json_error_response(400, "invalid_gateway_key_name", message);
    }
    let requested_tenant_id = runtime_gateway_admin_request_tenant_id(&body, admin_auth);
    if !admin_auth.can_access_tenant(requested_tenant_id.as_deref())
        || !admin_auth.can_access_dimensions(
            runtime_gateway_admin_request_dimension(
                &body,
                "team_id",
                admin_auth.team_id.as_deref(),
            )
            .as_deref(),
            runtime_gateway_admin_request_dimension(
                &body,
                "project_id",
                admin_auth.project_id.as_deref(),
            )
            .as_deref(),
            runtime_gateway_admin_request_dimension(
                &body,
                "user_id",
                admin_auth.user_id.as_deref(),
            )
            .as_deref(),
            runtime_gateway_admin_request_dimension(
                &body,
                "budget_id",
                admin_auth.budget_id.as_deref(),
            )
            .as_deref(),
        )
        || !admin_auth.can_access_key(&name)
    {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    if runtime_gateway_virtual_key_name_exists(shared, &name) {
        return build_runtime_proxy_json_error_response(
            409,
            "gateway_key_exists",
            "gateway virtual key name already exists",
        );
    }
    let supplied_token = body
        .get("token")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let generated_token = match supplied_token {
        Some(token) => token,
        None => match runtime_gateway_generate_virtual_key_token() {
            Ok(token) => token,
            Err(err) => {
                return build_runtime_proxy_json_error_response(
                    500,
                    "gateway_key_generation_failed",
                    &err.to_string(),
                );
            }
        },
    };
    let now = runtime_gateway_unix_epoch_seconds();
    let mut record = RuntimeGatewayStoredVirtualKey {
        name: name.clone(),
        tenant_id: requested_tenant_id,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        token_hash_base64: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            &generated_token,
        )
        .hash_base64(),
        allowed_models: Vec::new(),
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
        disabled: Some(false),
        created_at_epoch: now,
        updated_at_epoch: now,
    };
    if let Err(err) = runtime_gateway_apply_virtual_key_patch(&mut record, &body, false) {
        return err.into_response();
    }
    if runtime_gateway_admin_body_tenant_id(&body).is_none() && record.tenant_id.is_none() {
        record.tenant_id = admin_auth.tenant_id.clone();
    }
    if body.get("team_id").is_none() && record.team_id.is_none() {
        record.team_id = admin_auth.team_id.clone();
    }
    if body.get("project_id").is_none() && record.project_id.is_none() {
        record.project_id = admin_auth.project_id.clone();
    }
    if body.get("user_id").is_none() && record.user_id.is_none() {
        record.user_id = admin_auth.user_id.clone();
    }
    if body.get("budget_id").is_none() && record.budget_id.is_none() {
        record.budget_id = admin_auth.budget_id.clone();
    }
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        if store
            .keys
            .iter()
            .any(|key| key.name.eq_ignore_ascii_case(&name))
        {
            return Err(RuntimeGatewayAdminError::new(
                409,
                "gateway_key_exists",
                "gateway virtual key name already exists",
            ));
        }
        store.keys.push(record.clone());
        Ok(())
    }) {
        Ok(()) => {
            runtime_gateway_audit_admin_key_event(
                shared,
                "create_key",
                "success",
                &name,
                serde_json::json!({
                    "generated_token": body.get("token").is_none(),
                    "tenant_id": record.tenant_id,
                    "team_id": record.team_id,
                    "project_id": record.project_id,
                    "user_id": record.user_id,
                    "budget_id": record.budget_id,
                    "allowed_models": record.allowed_models,
                    "disabled": record.disabled.unwrap_or(false),
                    "request_budget": record.request_budget,
                    "rpm_limit": record.rpm_limit,
                    "tpm_limit": record.tpm_limit,
                    "budget_microusd": record.budget_microusd,
                }),
            );
            runtime_gateway_admin_json_response(
                201,
                serde_json::json!({
                    "object": "gateway.key",
                    "key": runtime_gateway_admin_stored_key_json(&record),
                    "token": generated_token,
                }),
            )
        }
        Err(response) => response,
    }
}

pub(super) fn runtime_gateway_admin_update_key_response(
    name: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let Some(entry) = runtime_gateway_virtual_key_entry_by_name(shared, name) else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        );
    };
    if !admin_auth.can_access_entry(&entry) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    if entry.source != RuntimeGatewayVirtualKeySource::Admin {
        return build_runtime_proxy_json_error_response(
            403,
            "gateway_key_read_only",
            "policy-backed gateway virtual keys cannot be edited through the admin API",
        );
    }
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    if let Some(tenant_id) = runtime_gateway_admin_body_tenant_id(&body)
        && !admin_auth.can_access_tenant(tenant_id.as_deref())
    {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    let requested_team_id = runtime_gateway_admin_optional_dimension_from_body(
        &body,
        "team_id",
        entry.key.team_id.as_deref(),
    );
    let requested_project_id = runtime_gateway_admin_optional_dimension_from_body(
        &body,
        "project_id",
        entry.key.project_id.as_deref(),
    );
    let requested_user_id = runtime_gateway_admin_optional_dimension_from_body(
        &body,
        "user_id",
        entry.key.user_id.as_deref(),
    );
    let requested_budget_id = runtime_gateway_admin_optional_dimension_from_body(
        &body,
        "budget_id",
        entry.key.budget_id.as_deref(),
    );
    if !admin_auth.can_access_dimensions(
        requested_team_id.as_deref(),
        requested_project_id.as_deref(),
        requested_user_id.as_deref(),
        requested_budget_id.as_deref(),
    ) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    let rotate = body
        .get("rotate")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut rotated_token = None;
    let update_result = runtime_gateway_mutate_admin_key_store(shared, |store| {
        let Some(record) = store
            .keys
            .iter_mut()
            .find(|key| key.name.eq_ignore_ascii_case(name))
        else {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_key_not_found",
                "gateway virtual key was not found",
            ));
        };
        if rotate {
            let token = runtime_gateway_generate_virtual_key_token().map_err(|err| {
                RuntimeGatewayAdminError::new(500, "gateway_key_generation_failed", err.to_string())
            })?;
            record.token_hash_base64 =
                runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token).hash_base64();
            rotated_token = Some(token);
        } else if let Some(token) = body
            .get("token")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            record.token_hash_base64 =
                runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token).hash_base64();
        }
        runtime_gateway_apply_virtual_key_patch(record, &body, true)?;
        record.updated_at_epoch = runtime_gateway_unix_epoch_seconds();
        Ok(())
    });
    match update_result {
        Ok(()) => {
            let entry = runtime_gateway_virtual_key_entry_by_name(shared, name);
            runtime_gateway_audit_admin_key_event(
                shared,
                if rotate { "rotate_key" } else { "update_key" },
                "success",
                name,
                serde_json::json!({
                    "rotated": rotate,
                    "token_replaced": !rotate && body.get("token").is_some(),
                    "updated_fields": runtime_gateway_admin_key_patch_fields(&body),
                }),
            );
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.key",
                    "key": entry.map(|entry| {
                        runtime_gateway_admin_key_json(&entry, shared.gateway_usage.usage.lock().ok().and_then(|usage| usage.get(&entry.key.name).cloned()))
                    }),
                    "token": rotated_token,
                }),
            )
        }
        Err(response) => response,
    }
}

pub(super) fn runtime_gateway_admin_delete_key_response(
    name: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let Some(entry) = runtime_gateway_virtual_key_entry_by_name(shared, name) else {
        return build_runtime_proxy_json_error_response(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        );
    };
    if !admin_auth.can_access_entry(&entry) {
        return runtime_gateway_admin_key_scope_forbidden_response();
    }
    if entry.source != RuntimeGatewayVirtualKeySource::Admin {
        return build_runtime_proxy_json_error_response(
            403,
            "gateway_key_read_only",
            "policy-backed gateway virtual keys cannot be deleted through the admin API",
        );
    }
    match runtime_gateway_mutate_admin_key_store(shared, |store| {
        let before = store.keys.len();
        store
            .keys
            .retain(|key| !key.name.eq_ignore_ascii_case(name));
        if store.keys.len() == before {
            return Err(RuntimeGatewayAdminError::new(
                404,
                "gateway_key_not_found",
                "gateway virtual key was not found",
            ));
        }
        Ok(())
    }) {
        Ok(()) => runtime_gateway_admin_json_response(
            {
                runtime_gateway_audit_admin_key_event(
                    shared,
                    "delete_key",
                    "success",
                    &entry.key.name,
                    serde_json::json!({
                        "source": entry.source.as_str(),
                        "disabled": entry.disabled,
                    }),
                );
                200
            },
            serde_json::json!({
                "object": "gateway.key.deleted",
                "name": entry.key.name,
                "deleted": true,
            }),
        ),
        Err(response) => response,
    }
}

fn runtime_gateway_admin_key_scope_forbidden_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        403,
        "gateway_admin_key_scope_forbidden",
        "gateway admin token is not allowed to access this virtual key",
    )
}

fn runtime_gateway_admin_request_tenant_id(
    body: &serde_json::Value,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Option<String> {
    runtime_gateway_admin_body_tenant_id(body).unwrap_or_else(|| admin_auth.tenant_id.clone())
}
