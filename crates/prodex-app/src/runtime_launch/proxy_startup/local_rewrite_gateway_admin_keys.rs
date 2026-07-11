use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_virtual_key_store_load,
};
use super::local_rewrite_gateway_admin_audit::{
    runtime_gateway_audit_admin_authorization_denied_event, runtime_gateway_audit_admin_key_event,
    runtime_gateway_audit_admin_key_mutation_denied_event,
};
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
use prodex_application::{
    ApplicationVirtualKeyLifecycleErrorStatus, ApplicationVirtualKeyLifecycleRequest,
    plan_application_virtual_key_lifecycle, plan_application_virtual_key_lifecycle_error_response,
};
use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneOperation, ControlPlaneResourceRef,
};
use prodex_domain::{
    AuditDigest, CredentialScope, Principal, PrincipalId, PrincipalKind, ResourceKind, Role,
    SecretRef, TenantId, VirtualKeyId,
};
use prodex_storage::{
    DurableStoreKind, TenantStorageKey, VirtualKeySecretReferenceCommand,
    VirtualKeySecretReferenceKind,
};
use sha2::{Digest, Sha256};

const RUNTIME_GATEWAY_KEY_GENERATION_FAILED_MESSAGE: &str =
    "gateway key token could not be generated";
const RUNTIME_GATEWAY_KEY_TENANT_REQUIRED_MESSAGE: &str =
    "gateway virtual key tenant_id is required for durable key storage";
const RUNTIME_GATEWAY_KEY_TENANT_INVALID_MESSAGE: &str =
    "gateway virtual key tenant_id must be a valid tenant identifier";
const RUNTIME_GATEWAY_KEY_ID_REQUIRED_MESSAGE: &str =
    "gateway virtual key id is required for durable key storage";
const RUNTIME_GATEWAY_KEY_ID_INVALID_MESSAGE: &str =
    "gateway virtual key id must be a valid virtual key identifier";

fn runtime_gateway_admin_key_generation_failed_error() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        500,
        "gateway_key_generation_failed",
        RUNTIME_GATEWAY_KEY_GENERATION_FAILED_MESSAGE,
    )
}

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
        return runtime_gateway_admin_key_scope_forbidden_response(
            shared,
            admin_auth,
            "get_key",
            &entry.key.name,
        );
    }
    let usage = shared
        .gateway_usage
        .usage
        .lock()
        .ok()
        .and_then(|usage| usage.get(&entry.key.name).cloned());
    runtime_gateway_admin_key_json_response_with_etag(
        200,
        serde_json::json!({
            "object": "gateway.key",
            "key": runtime_gateway_admin_key_json(entry, usage),
        }),
        &runtime_gateway_admin_key_etag(entry.updated_at_epoch),
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
        let requested_name = body
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("<invalid>");
        return runtime_gateway_admin_key_scope_forbidden_response(
            shared,
            admin_auth,
            "create_key",
            requested_name,
        );
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
            Err(_err) => {
                return build_runtime_proxy_json_error_response(
                    500,
                    "gateway_key_generation_failed",
                    RUNTIME_GATEWAY_KEY_GENERATION_FAILED_MESSAGE,
                );
            }
        },
    };
    let now = runtime_gateway_unix_epoch_seconds();
    let mut record = RuntimeGatewayStoredVirtualKey {
        name: name.clone(),
        tenant_id: requested_tenant_id,
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
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
    if let Some(response) = runtime_gateway_admin_virtual_key_lifecycle_response(
        shared,
        admin_auth,
        &record,
        VirtualKeySecretReferenceKind::Create,
    ) {
        return response;
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
        return runtime_gateway_admin_key_scope_forbidden_response(
            shared,
            admin_auth,
            "update_key",
            &entry.key.name,
        );
    }
    if entry.source != RuntimeGatewayVirtualKeySource::Admin {
        runtime_gateway_audit_admin_key_mutation_denied_event(
            shared,
            &admin_auth.name,
            admin_auth.role.as_str(),
            "gateway_key_read_only",
            "update_key",
            &entry.key.name,
        );
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
        return runtime_gateway_admin_key_scope_forbidden_response(
            shared,
            admin_auth,
            "update_key",
            &entry.key.name,
        );
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
        return runtime_gateway_admin_key_scope_forbidden_response(
            shared,
            admin_auth,
            "update_key",
            &entry.key.name,
        );
    }
    let rotate = body
        .get("rotate")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if rotate {
        let store = runtime_gateway_virtual_key_store_load(
            &shared.gateway_state_store,
            &shared.runtime_shared.log_path,
        );
        let Some(mut planned_record) = store
            .keys
            .iter()
            .find(|record| record.name.eq_ignore_ascii_case(name))
            .cloned()
        else {
            return build_runtime_proxy_json_error_response(
                404,
                "gateway_key_not_found",
                "gateway virtual key was not found",
            );
        };
        if let Err(err) = runtime_gateway_apply_virtual_key_patch(&mut planned_record, &body, true)
        {
            return err.into_response();
        }
        planned_record.updated_at_epoch = runtime_gateway_unix_epoch_seconds();
        if let Some(response) = runtime_gateway_admin_virtual_key_lifecycle_response(
            shared,
            admin_auth,
            &planned_record,
            VirtualKeySecretReferenceKind::Rotate,
        ) {
            return response;
        }
    }
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
            let token = runtime_gateway_generate_virtual_key_token()
                .map_err(|_err| runtime_gateway_admin_key_generation_failed_error())?;
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
        return runtime_gateway_admin_key_scope_forbidden_response(
            shared,
            admin_auth,
            "delete_key",
            &entry.key.name,
        );
    }
    if entry.source != RuntimeGatewayVirtualKeySource::Admin {
        runtime_gateway_audit_admin_key_mutation_denied_event(
            shared,
            &admin_auth.name,
            admin_auth.role.as_str(),
            "gateway_key_read_only",
            "delete_key",
            &entry.key.name,
        );
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

fn runtime_gateway_admin_key_scope_forbidden_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    action: &'static str,
    key_name: &str,
) -> tiny_http::ResponseBox {
    runtime_gateway_audit_admin_authorization_denied_event(
        shared,
        &admin_auth.name,
        admin_auth.role.as_str(),
        "key",
        action,
        key_name,
    );
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

fn runtime_gateway_admin_virtual_key_lifecycle_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    record: &RuntimeGatewayStoredVirtualKey,
    kind: VirtualKeySecretReferenceKind,
) -> Option<tiny_http::ResponseBox> {
    let durable_store = match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Postgres { .. } => DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::Sqlite { .. } => DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return None;
        }
    };
    let tenant_id = match runtime_gateway_admin_durable_key_tenant_id(record) {
        Ok(tenant_id) => tenant_id,
        Err(err) => return Some(err.into_response()),
    };
    let virtual_key_id = match runtime_gateway_admin_durable_key_virtual_key_id(record) {
        Ok(virtual_key_id) => virtual_key_id,
        Err(err) => return Some(err.into_response()),
    };
    let principal_id = runtime_gateway_admin_key_actor_principal_id(admin_auth, tenant_id);
    let occurred_at_unix_ms = record.updated_at_epoch.saturating_mul(1000);
    let request = ApplicationVirtualKeyLifecycleRequest {
        durable_store,
        action: ControlPlaneActionRequest {
            principal: Principal::new(
                principal_id,
                Some(tenant_id),
                PrincipalKind::User,
                runtime_gateway_admin_key_domain_role(admin_auth.role),
                CredentialScope::ControlPlane,
            ),
            operation: match kind {
                VirtualKeySecretReferenceKind::Create => ControlPlaneOperation::VirtualKeyCreate,
                VirtualKeySecretReferenceKind::Rotate => {
                    ControlPlaneOperation::VirtualKeyRotateSecret
                }
            },
            resource: ControlPlaneResourceRef::new(
                tenant_id,
                ResourceKind::VirtualKey,
                Some(record.name.clone()),
            ),
            occurred_at_unix_ms,
        },
        reference: VirtualKeySecretReferenceCommand {
            storage_key: TenantStorageKey::virtual_key(tenant_id, virtual_key_id),
            tenant_id,
            virtual_key_id,
            principal_id,
            display_name: record.name.clone(),
            secret_ref: SecretRef::new(
                "gateway-admin",
                format!("virtual-keys/{}", record.name),
                Some("compat"),
            ),
            kind,
            occurred_at_unix_ms,
        },
        previous_digest: None,
        event_digest: AuditDigest::new(match kind {
            VirtualKeySecretReferenceKind::Create => "sha256:gateway-virtual-key-create",
            VirtualKeySecretReferenceKind::Rotate => "sha256:gateway-virtual-key-rotate",
        })
        .expect("static virtual-key lifecycle audit digest should be valid"),
    };
    match plan_application_virtual_key_lifecycle(request) {
        Ok(_) => None,
        Err(error) => {
            let response = plan_application_virtual_key_lifecycle_error_response(&error);
            Some(build_runtime_proxy_json_error_response(
                runtime_gateway_application_virtual_key_lifecycle_status_code(response.status),
                response.code,
                response.message,
            ))
        }
    }
}

fn runtime_gateway_admin_key_actor_principal_id(
    admin_auth: &RuntimeGatewayAdminAuth,
    tenant_id: TenantId,
) -> PrincipalId {
    let mut hasher = Sha256::new();
    hasher.update(b"prodex:gateway-admin-key-actor:v1");
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

fn runtime_gateway_admin_durable_key_tenant_id(
    record: &RuntimeGatewayStoredVirtualKey,
) -> Result<TenantId, RuntimeGatewayAdminError> {
    let Some(tenant_id) = record.tenant_id.as_deref() else {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "gateway_key_tenant_required",
            RUNTIME_GATEWAY_KEY_TENANT_REQUIRED_MESSAGE,
        ));
    };
    tenant_id.parse::<TenantId>().map_err(|_| {
        RuntimeGatewayAdminError::new(
            400,
            "gateway_key_tenant_invalid",
            RUNTIME_GATEWAY_KEY_TENANT_INVALID_MESSAGE,
        )
    })
}

fn runtime_gateway_admin_durable_key_virtual_key_id(
    record: &RuntimeGatewayStoredVirtualKey,
) -> Result<VirtualKeyId, RuntimeGatewayAdminError> {
    let Some(virtual_key_id) = record.virtual_key_id.as_deref() else {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "gateway_key_id_required",
            RUNTIME_GATEWAY_KEY_ID_REQUIRED_MESSAGE,
        ));
    };
    virtual_key_id.parse::<VirtualKeyId>().map_err(|_| {
        RuntimeGatewayAdminError::new(
            400,
            "gateway_key_id_invalid",
            RUNTIME_GATEWAY_KEY_ID_INVALID_MESSAGE,
        )
    })
}

fn runtime_gateway_admin_key_domain_role(role: RuntimeGatewayAdminRole) -> Role {
    match role {
        RuntimeGatewayAdminRole::Admin => Role::Admin,
        RuntimeGatewayAdminRole::Viewer => Role::Viewer,
    }
}

fn runtime_gateway_application_virtual_key_lifecycle_status_code(
    status: ApplicationVirtualKeyLifecycleErrorStatus,
) -> u16 {
    match status {
        ApplicationVirtualKeyLifecycleErrorStatus::BadRequest => 400,
        ApplicationVirtualKeyLifecycleErrorStatus::ServiceUnavailable => 503,
    }
}

pub(super) fn runtime_gateway_admin_key_etag(updated_at_epoch: Option<u64>) -> String {
    format!("\"gateway-key-{}\"", updated_at_epoch.unwrap_or_default())
}

fn runtime_gateway_admin_key_json_response_with_etag(
    status: u16,
    value: serde_json::Value,
    etag: &str,
) -> tiny_http::ResponseBox {
    let body = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![
            (
                "content-type".to_string(),
                b"application/json; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
            ("etag".to_string(), etag.as_bytes().to_vec()),
        ],
        body: body.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_key_generation_error_uses_stable_redacted_message() {
        let err = runtime_gateway_admin_key_generation_failed_error();
        assert_eq!(err.test_status(), 500);
        assert_eq!(err.test_code(), "gateway_key_generation_failed");
        assert_eq!(
            err.test_message(),
            "gateway key token could not be generated"
        );
        assert!(!err.test_message().contains("getrandom"));
        assert!(
            !err.test_message()
                .contains("failed to generate gateway virtual key")
        );
    }

    fn stored_key_with_tenant(tenant_id: Option<String>) -> RuntimeGatewayStoredVirtualKey {
        stored_key_with_ids(tenant_id, Some(VirtualKeyId::new().to_string()))
    }

    fn stored_key_with_ids(
        tenant_id: Option<String>,
        virtual_key_id: Option<String>,
    ) -> RuntimeGatewayStoredVirtualKey {
        RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            tenant_id,
            virtual_key_id,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash_base64: "hash".to_string(),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
            disabled: Some(false),
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
    fn durable_key_lifecycle_rejects_missing_or_invalid_tenant_id() {
        let missing = runtime_gateway_admin_durable_key_tenant_id(&stored_key_with_tenant(None))
            .expect_err("missing tenant should fail closed");
        assert_eq!(missing.test_status(), 400);
        assert_eq!(missing.test_code(), "gateway_key_tenant_required");
        assert_eq!(
            missing.test_message(),
            RUNTIME_GATEWAY_KEY_TENANT_REQUIRED_MESSAGE
        );

        let invalid = runtime_gateway_admin_durable_key_tenant_id(&stored_key_with_tenant(Some(
            "tenant-a".to_string(),
        )))
        .expect_err("non-UUID tenant should fail closed");
        assert_eq!(invalid.test_status(), 400);
        assert_eq!(invalid.test_code(), "gateway_key_tenant_invalid");
        assert_eq!(
            invalid.test_message(),
            RUNTIME_GATEWAY_KEY_TENANT_INVALID_MESSAGE
        );

        let tenant_id = TenantId::new();
        let parsed = runtime_gateway_admin_durable_key_tenant_id(&stored_key_with_tenant(Some(
            tenant_id.to_string(),
        )));
        assert!(matches!(parsed, Ok(parsed) if parsed == tenant_id));
    }

    #[test]
    fn durable_key_lifecycle_rejects_missing_or_invalid_virtual_key_id() {
        let tenant_id = Some(TenantId::new().to_string());
        let missing = runtime_gateway_admin_durable_key_virtual_key_id(&stored_key_with_ids(
            tenant_id.clone(),
            None,
        ))
        .expect_err("missing virtual-key id should fail closed");
        assert_eq!(missing.test_status(), 400);
        assert_eq!(missing.test_code(), "gateway_key_id_required");
        assert_eq!(
            missing.test_message(),
            RUNTIME_GATEWAY_KEY_ID_REQUIRED_MESSAGE
        );

        let invalid = runtime_gateway_admin_durable_key_virtual_key_id(&stored_key_with_ids(
            tenant_id,
            Some("key-a".to_string()),
        ))
        .expect_err("non-UUID virtual-key id should fail closed");
        assert_eq!(invalid.test_status(), 400);
        assert_eq!(invalid.test_code(), "gateway_key_id_invalid");
        assert_eq!(
            invalid.test_message(),
            RUNTIME_GATEWAY_KEY_ID_INVALID_MESSAGE
        );

        let virtual_key_id = VirtualKeyId::new();
        let parsed = runtime_gateway_admin_durable_key_virtual_key_id(&stored_key_with_ids(
            Some(TenantId::new().to_string()),
            Some(virtual_key_id.to_string()),
        ));
        assert!(matches!(parsed, Ok(parsed) if parsed == virtual_key_id));
    }

    #[test]
    fn durable_key_lifecycle_actor_principal_id_is_stable_per_admin_and_tenant() {
        let tenant_id = TenantId::new();
        let admin = admin_auth_named("admin-token");

        assert_eq!(
            runtime_gateway_admin_key_actor_principal_id(&admin, tenant_id),
            runtime_gateway_admin_key_actor_principal_id(&admin, tenant_id)
        );
        assert_ne!(
            runtime_gateway_admin_key_actor_principal_id(&admin, tenant_id),
            runtime_gateway_admin_key_actor_principal_id(
                &admin_auth_named("other-admin"),
                tenant_id
            )
        );
        assert_ne!(
            runtime_gateway_admin_key_actor_principal_id(&admin, tenant_id),
            runtime_gateway_admin_key_actor_principal_id(&admin, TenantId::new())
        );
    }

    #[test]
    fn durable_key_lifecycle_does_not_synthesize_tenant_virtual_key_or_actor_ids() {
        let source = include_str!("local_rewrite_gateway_admin_keys.rs");
        for fallback in [
            ["unwrap_or_else", "(TenantId::new"].join(""),
            ["unwrap_or_else", "(VirtualKeyId::new"].join(""),
            ["PrincipalId", "::new()"].join(""),
        ] {
            assert!(!source.contains(&fallback));
        }
    }
}
