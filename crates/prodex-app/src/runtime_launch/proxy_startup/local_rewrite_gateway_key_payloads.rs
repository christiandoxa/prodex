use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuth, runtime_gateway_admin_auth_is_unscoped,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
};
use prodex_provider_core::microusd_to_usd;

pub(super) fn runtime_gateway_admin_keys_payload(
    shared: &RuntimeLocalRewriteProxyShared,
    object: &str,
    admin_auth: Option<&RuntimeGatewayAdminAuth>,
) -> serde_json::Value {
    let usage = shared
        .gateway_usage
        .usage
        .lock()
        .map(|usage| usage.clone())
        .unwrap_or_default();
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let configured_names = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| entry.key.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let keys = entries
        .iter()
        .filter(|entry| {
            admin_auth
                .map(|admin_auth| admin_auth.can_access_entry(entry))
                .unwrap_or(true)
        })
        .map(|entry| runtime_gateway_admin_key_json(entry, usage.get(&entry.key.name).cloned()))
        .collect::<Vec<_>>();
    let unknown_persisted_keys = usage
        .keys()
        .filter(|name| {
            !configured_names
                .iter()
                .any(|configured| *configured == name.as_str())
                && admin_auth
                    .map(|admin_auth| {
                        runtime_gateway_admin_auth_is_unscoped(admin_auth)
                            && admin_auth.can_access_key(name)
                    })
                    .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();
    serde_json::json!({
        "object": object,
        "keys": keys,
        "state_backend": shared.gateway_state_store.label(),
        "state_path": shared.gateway_state_store.key_store_path().display().to_string(),
        "key_store_path": shared.gateway_virtual_key_store_path.display().to_string(),
        "usage_path": shared.gateway_usage.path.as_ref().map(|path| path.display().to_string()),
        "unknown_persisted_keys": unknown_persisted_keys,
    })
}

pub(super) fn runtime_gateway_admin_key_json(
    entry: &RuntimeGatewayVirtualKeyEntry,
    usage: Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
) -> serde_json::Value {
    let usage = usage.unwrap_or_default();
    serde_json::json!({
        "virtual_key_id": entry.virtual_key_id.map(|id| id.to_string()),
        "name": entry.key.name,
        "tenant_id": entry.tenant_id,
        "team_id": entry.key.team_id,
        "project_id": entry.key.project_id,
        "user_id": entry.key.user_id,
        "budget_id": entry.key.budget_id,
        "source": entry.source.as_str(),
        "disabled": entry.disabled,
        "editable": entry.source == RuntimeGatewayVirtualKeySource::Admin,
        "created_at_epoch": entry.created_at_epoch,
        "updated_at_epoch": entry.updated_at_epoch,
        "allowed_models": entry.key.allowed_models,
        "budget_microusd": entry.key.budget_microusd,
        "budget_usd": entry.key.budget_microusd.map(microusd_to_usd),
        "request_budget": entry.key.request_budget,
        "rpm_limit": entry.key.rpm_limit,
        "tpm_limit": entry.key.tpm_limit,
        "usage": {
            "minute_epoch": usage.minute_epoch,
            "requests_this_minute": usage.requests_this_minute,
            "tokens_this_minute": usage.tokens_this_minute,
            "requests_total": usage.requests_total,
            "spend_microusd": usage.spend_microusd,
            "spend_usd": microusd_to_usd(usage.spend_microusd),
        }
    })
}

pub(super) fn runtime_gateway_admin_stored_key_json(
    record: &RuntimeGatewayStoredVirtualKey,
) -> serde_json::Value {
    serde_json::json!({
        "virtual_key_id": record.virtual_key_id,
        "name": record.name,
        "tenant_id": record.tenant_id,
        "team_id": record.team_id,
        "project_id": record.project_id,
        "user_id": record.user_id,
        "budget_id": record.budget_id,
        "source": "admin",
        "disabled": record.disabled.unwrap_or(false),
        "editable": true,
        "created_at_epoch": record.created_at_epoch,
        "updated_at_epoch": record.updated_at_epoch,
        "allowed_models": record.allowed_models,
        "budget_microusd": record.budget_microusd,
        "budget_usd": record.budget_microusd.map(microusd_to_usd),
        "request_budget": record.request_budget,
        "rpm_limit": record.rpm_limit,
        "tpm_limit": record.tpm_limit,
    })
}

pub(super) fn runtime_gateway_virtual_key_entry_by_name(
    shared: &RuntimeLocalRewriteProxyShared,
    name: &str,
) -> Option<RuntimeGatewayVirtualKeyEntry> {
    shared
        .gateway_virtual_keys
        .lock()
        .ok()?
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(name))
        .cloned()
}
