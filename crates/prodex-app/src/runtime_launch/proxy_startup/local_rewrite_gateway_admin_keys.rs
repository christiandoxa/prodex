use prodex_application::{
    ApplicationGatewayIdentityMutationError, ApplicationGatewayVirtualKeyMutationRequest,
    ApplicationSecretFingerprint, plan_application_gateway_virtual_key_mutation,
};
use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use zeroize::Zeroizing;

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_audit::{
    runtime_gateway_audit_admin_authorization_denied_event,
    runtime_gateway_audit_admin_key_mutation_denied_event,
    runtime_gateway_audit_admin_request_denied_event,
};
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_execution::{
    RuntimeGatewayAdminMutationExecution, runtime_gateway_admin_mutation_execution,
};
use super::local_rewrite_gateway_admin_identity::{
    RuntimeGatewayIdentityKind, runtime_gateway_apply_virtual_key_projection,
    runtime_gateway_identity_error, runtime_gateway_virtual_key_create_mutation,
    runtime_gateway_virtual_key_delete_mutation, runtime_gateway_virtual_key_record,
    runtime_gateway_virtual_key_update_mutation,
};
use super::local_rewrite_gateway_admin_response::{
    RuntimeGatewayAdminError, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_store_mutation::runtime_gateway_mutate_admin_key_store_atomic;
use super::local_rewrite_gateway_key_payloads::{
    runtime_gateway_admin_key_json, runtime_gateway_admin_stored_key_json,
    runtime_gateway_virtual_key_entry_by_name,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeySource,
    RuntimeGatewayVirtualKeyStoreFile,
};
use super::local_rewrite_gateway_util::runtime_gateway_generate_virtual_key_token;
use super::*;

mod intent;
use intent::RuntimeGatewayKeyMutationIntent;

#[cfg(test)]
#[path = "local_rewrite_gateway_admin_keys_tests.rs"]
mod tests;

const RUNTIME_GATEWAY_KEY_GENERATION_FAILED_MESSAGE: &str =
    "gateway key token could not be generated";

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
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        request_path(captured),
        admin_auth,
        base_action,
        ControlPlaneOperation::VirtualKeyCreate,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let RuntimeGatewayAdminMutationExecution {
        authorized_action,
        governance,
        atomic_write,
        entity_tag: _,
    } = execution;
    let now_unix_ms = atomic_write.completed_at_unix_ms;
    let mut created = None;
    let mut returned_token = None;
    let result = runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let mut body = mutation_json(captured)?;
        let records = application_key_records(store)?;
        let supplied = take_supplied_token(&mut body);
        let token = match supplied {
            Some(token) => token,
            None => Zeroizing::new(
                runtime_gateway_generate_virtual_key_token()
                    .map_err(|_| runtime_gateway_admin_key_generation_failed_error())?,
            ),
        };
        let fingerprint = token_fingerprint(&token)?;
        let mutation = runtime_gateway_virtual_key_create_mutation(
            prodex_domain::VirtualKeyId::new(),
            &body,
            fingerprint,
        )?;
        let plan = plan_application_gateway_virtual_key_mutation(
            ApplicationGatewayVirtualKeyMutationRequest {
                authorized_action: &authorized_action,
                governance: &governance,
                current_records: &records,
                mutation,
                now_unix_ms,
            },
        )
        .map_err(|error| {
            audit_key_identity_denial(
                shared,
                admin_auth,
                "create_key",
                body.get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<invalid>"),
                error,
            )
        })?;
        created = Some(runtime_gateway_apply_virtual_key_projection(store, &plan)?);
        returned_token = Some(token);
        Ok(())
    });
    match result {
        Ok(()) => runtime_gateway_admin_json_response(
            201,
            serde_json::json!({
                "object": "gateway.key",
                "key": created.as_ref().map(runtime_gateway_admin_stored_key_json),
                "token": returned_token.as_ref().map(|token| token.as_str()),
            }),
        ),
        Err(response) => response,
    }
}

pub(super) fn runtime_gateway_admin_update_key_response(
    name: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    force_rotate: bool,
) -> tiny_http::ResponseBox {
    if let Some(response) = policy_key_mutation_denied(shared, admin_auth, name, "update_key") {
        return response;
    }
    let intent = match force_rotate && captured.body.is_empty() {
        true => RuntimeGatewayKeyMutationIntent::default(),
        false => match RuntimeGatewayKeyMutationIntent::parse(&captured.body) {
            Ok(intent) => intent,
            Err(error) => return error.into_response(),
        },
    };
    let requested_operation = if force_rotate || intent.rotates_secret() {
        ControlPlaneOperation::VirtualKeyRotateSecret
    } else {
        ControlPlaneOperation::VirtualKeyUpdate
    };
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        request_path(captured),
        admin_auth,
        base_action,
        requested_operation,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let RuntimeGatewayAdminMutationExecution {
        authorized_action,
        governance,
        atomic_write,
        entity_tag,
    } = execution;
    let now_unix_ms = atomic_write.completed_at_unix_ms;
    let mut returned_token = None;
    let result = runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let current = store
            .keys
            .iter()
            .find(|record| record.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                runtime_gateway_identity_error(
                    RuntimeGatewayIdentityKind::VirtualKey,
                    ApplicationGatewayIdentityMutationError::NotFound,
                )
            })?;
        enforce_key_entity_tag_with_audit(
            entity_tag.as_ref(),
            current,
            shared,
            admin_auth,
            captured,
        )?;
        let id = runtime_gateway_virtual_key_record(current)?.id;
        let records = application_key_records(store)?;
        let mut body = if force_rotate && captured.body.is_empty() {
            serde_json::json!({})
        } else {
            mutation_json(captured)?
        };
        let supplied = take_supplied_token(&mut body);
        let secret = if force_rotate || intent.rotate {
            drop(supplied);
            Some(Zeroizing::new(
                runtime_gateway_generate_virtual_key_token()
                    .map_err(|_| runtime_gateway_admin_key_generation_failed_error())?,
            ))
        } else {
            supplied
        };
        let fingerprint = secret.as_ref().map(token_fingerprint).transpose()?;
        let mutation = runtime_gateway_virtual_key_update_mutation(id, &body, fingerprint)?;
        let plan = plan_application_gateway_virtual_key_mutation(
            ApplicationGatewayVirtualKeyMutationRequest {
                authorized_action: &authorized_action,
                governance: &governance,
                current_records: &records,
                mutation,
                now_unix_ms,
            },
        )
        .map_err(|error| {
            audit_key_identity_denial(shared, admin_auth, "update_key", name, error)
        })?;
        runtime_gateway_apply_virtual_key_projection(store, &plan)?;
        if force_rotate || intent.rotate {
            returned_token = secret;
        }
        Ok(())
    });
    match result {
        Ok(()) => {
            let entry = runtime_gateway_virtual_key_entry_by_name(shared, name);
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.key",
                    "key": entry.map(|entry| runtime_gateway_admin_key_json(
                        &entry,
                        shared.gateway_usage.usage.lock().ok()
                            .and_then(|usage| usage.get(&entry.key.name).cloned()),
                    )),
                    "token": returned_token.as_ref().map(|token| token.as_str()),
                }),
            )
        }
        Err(response) => response,
    }
}

pub(super) fn runtime_gateway_admin_delete_key_response(
    name: &str,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    if let Some(response) = policy_key_mutation_denied(shared, admin_auth, name, "delete_key") {
        return response;
    }
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        request_path(captured),
        admin_auth,
        base_action,
        ControlPlaneOperation::VirtualKeyDelete,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let RuntimeGatewayAdminMutationExecution {
        authorized_action,
        governance,
        atomic_write,
        entity_tag,
    } = execution;
    let now_unix_ms = atomic_write.completed_at_unix_ms;
    let mut deleted = None;
    let result = runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let current = store
            .keys
            .iter()
            .find(|record| record.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                runtime_gateway_identity_error(
                    RuntimeGatewayIdentityKind::VirtualKey,
                    ApplicationGatewayIdentityMutationError::NotFound,
                )
            })?;
        enforce_key_entity_tag_with_audit(
            entity_tag.as_ref(),
            current,
            shared,
            admin_auth,
            captured,
        )?;
        let id = runtime_gateway_virtual_key_record(current)?.id;
        let records = application_key_records(store)?;
        let plan = plan_application_gateway_virtual_key_mutation(
            ApplicationGatewayVirtualKeyMutationRequest {
                authorized_action: &authorized_action,
                governance: &governance,
                current_records: &records,
                mutation: runtime_gateway_virtual_key_delete_mutation(id),
                now_unix_ms,
            },
        )
        .map_err(|error| {
            audit_key_identity_denial(shared, admin_auth, "delete_key", name, error)
        })?;
        deleted = Some(runtime_gateway_apply_virtual_key_projection(store, &plan)?);
        Ok(())
    });
    match result {
        Ok(()) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": "gateway.key.deleted",
                "name": deleted.as_ref().map(|record| record.name.as_str()),
                "deleted": true,
            }),
        ),
        Err(response) => response,
    }
}

fn application_key_records(
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<Vec<prodex_application::ApplicationGatewayVirtualKeyRecord>, RuntimeGatewayAdminError> {
    store
        .keys
        .iter()
        .map(runtime_gateway_virtual_key_record)
        .collect()
}

fn mutation_json(
    captured: &RuntimeProxyRequest,
) -> Result<serde_json::Value, RuntimeGatewayAdminError> {
    serde_json::from_slice(&captured.body).map_err(|_| {
        RuntimeGatewayAdminError::new(400, "invalid_json", "request body is not valid JSON")
    })
}

fn take_supplied_token(body: &mut serde_json::Value) -> Option<Zeroizing<String>> {
    let serde_json::Value::String(raw) = body.get_mut("token")? else {
        return None;
    };
    let raw = Zeroizing::new(std::mem::take(raw));
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| Zeroizing::new(trimmed.to_string()))
}

fn token_fingerprint(
    token: &Zeroizing<String>,
) -> Result<ApplicationSecretFingerprint, RuntimeGatewayAdminError> {
    ApplicationSecretFingerprint::new(
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token).hash_base64(),
    )
    .map_err(|error| runtime_gateway_identity_error(RuntimeGatewayIdentityKind::VirtualKey, error))
}

fn enforce_key_entity_tag(
    expected: Option<&prodex_domain::EntityTag>,
    current: &RuntimeGatewayStoredVirtualKey,
) -> Result<(), RuntimeGatewayAdminError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected.as_str() == "*"
        || expected.as_str()
            == runtime_gateway_admin_key_etag(Some(current.updated_at_epoch)).as_str()
    {
        return Ok(());
    }
    Err(RuntimeGatewayAdminError::new(
        412,
        "precondition_failed",
        "If-Match does not match the current gateway key ETag",
    ))
}

fn enforce_key_entity_tag_with_audit(
    expected: Option<&prodex_domain::EntityTag>,
    current: &RuntimeGatewayStoredVirtualKey,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    captured: &RuntimeProxyRequest,
) -> Result<(), RuntimeGatewayAdminError> {
    let Some(error) = enforce_key_entity_tag(expected, current).err() else {
        return Ok(());
    };
    runtime_gateway_audit_admin_request_denied_event(
        shared,
        admin_auth,
        "precondition_failed",
        &captured.method,
        request_path(captured),
    );
    Err(error)
}

fn audit_key_identity_denial(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    action: &'static str,
    key_name: &str,
    error: ApplicationGatewayIdentityMutationError,
) -> RuntimeGatewayAdminError {
    if matches!(
        error,
        ApplicationGatewayIdentityMutationError::GovernanceDenied
            | ApplicationGatewayIdentityMutationError::TenantMismatch
            | ApplicationGatewayIdentityMutationError::ResourceIdMismatch
    ) {
        runtime_gateway_audit_admin_authorization_denied_event(
            shared, admin_auth, "key", action, key_name,
        );
    }
    runtime_gateway_identity_error(RuntimeGatewayIdentityKind::VirtualKey, error)
}

fn policy_key_mutation_denied(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    name: &str,
    action: &'static str,
) -> Option<tiny_http::ResponseBox> {
    let entry = runtime_gateway_virtual_key_entry_by_name(shared, name)?;
    if entry.source != RuntimeGatewayVirtualKeySource::Policy {
        return None;
    }
    // Policy keys live outside the mutable admin store, so source routing must reject them here.
    runtime_gateway_audit_admin_key_mutation_denied_event(
        shared,
        admin_auth,
        "gateway_key_read_only",
        action,
        &entry.key.name,
    );
    Some(build_runtime_proxy_json_error_response(
        403,
        "gateway_key_read_only",
        if action == "delete_key" {
            "policy-backed gateway virtual keys cannot be deleted through the admin API"
        } else {
            "policy-backed gateway virtual keys cannot be edited through the admin API"
        },
    ))
}

fn runtime_gateway_admin_key_scope_forbidden_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    action: &'static str,
    key_name: &str,
) -> tiny_http::ResponseBox {
    runtime_gateway_audit_admin_authorization_denied_event(
        shared, admin_auth, "key", action, key_name,
    );
    build_runtime_proxy_json_error_response(
        403,
        "gateway_admin_key_scope_forbidden",
        "gateway admin token is not allowed to access this virtual key",
    )
}

fn runtime_gateway_admin_key_generation_failed_error() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        500,
        "gateway_key_generation_failed",
        RUNTIME_GATEWAY_KEY_GENERATION_FAILED_MESSAGE,
    )
}

fn request_path(captured: &RuntimeProxyRequest) -> &str {
    runtime_proxy_crate::path_without_query(&captured.path_and_query)
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
