use super::*;
use std::path::Path;

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_entries_is_empty(
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.is_empty())
        .unwrap_or(false)
}

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayVirtualKeySnapshot {
    pub(in crate::runtime_launch::proxy_startup) active_keys:
        Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(in crate::runtime_launch::proxy_startup) configured_count: usize,
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_snapshot(
    entries: std::sync::LockResult<std::sync::MutexGuard<'_, Vec<RuntimeGatewayVirtualKeyEntry>>>,
) -> Result<RuntimeGatewayVirtualKeySnapshot, runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection>
{
    let entries = entries.map_err(|_| {
        runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
    })?;
    Ok(RuntimeGatewayVirtualKeySnapshot {
        active_keys: entries
            .iter()
            .filter(|entry| !entry.disabled)
            .map(|entry| entry.key.clone())
            .collect(),
        configured_count: entries.len(),
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_entries_from_sources(
    policy_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<Vec<RuntimeGatewayVirtualKeyEntry>> {
    let mut entries = policy_keys
        .into_iter()
        .map(|key| RuntimeGatewayVirtualKeyEntry {
            virtual_key_id: None,
            tenant_id: key.tenant_id.clone(),
            key,
            source: RuntimeGatewayVirtualKeySource::Policy,
            group_ids: Vec::new(),
            department_id: None,
            created_at_epoch: None,
            updated_at_epoch: None,
            disabled: false,
        })
        .collect::<Vec<_>>();
    let mut seen = entries
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let store = runtime_gateway_virtual_key_store_load_strict(state_store, log_path)?;
    for record in &store.keys {
        let key_name = record.name.trim().to_string();
        if key_name.is_empty() {
            continue;
        }
        let normalized = key_name.to_ascii_lowercase();
        if seen.iter().any(|seen| seen == &normalized) {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_duplicate_ignored",
                    [runtime_proxy_log_field("key", key_name)],
                ),
            );
            continue;
        }
        let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(record) else {
            runtime_proxy_log_to_path(
                log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_store_invalid_hash",
                    [runtime_proxy_log_field("key", &key_name)],
                ),
            );
            anyhow::bail!("gateway virtual key store contains invalid token hash for {key_name}");
        };
        seen.push(normalized);
        entries.push(entry);
    }
    runtime_gateway_apply_scim_policy_attributes(&mut entries, &store.scim_users);
    Ok(entries)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_store_load_strict(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let path = state_store.key_store_path();
    let store = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => runtime_gateway_sqlite_load_key_store(path),
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            runtime_gateway_postgres_load_key_store(url, tls)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_redis_load_key_store(url, RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)
        }
        RuntimeGatewayStateStore::File { .. } => runtime_gateway_virtual_key_store_file_load(path)
            .map_err(|err| anyhow::anyhow!(err.to_string())),
    }
    .inspect_err(|_err| {
        runtime_proxy_log_to_path(
            log_path,
            &runtime_proxy_structured_log_message(
                "gateway_virtual_key_store_load_failed",
                [
                    runtime_proxy_log_field("backend", state_store.label()),
                    runtime_proxy_log_field("error_kind", "gateway_key_store_persistence_failed"),
                ],
            ),
        );
    })?;
    Ok(runtime_gateway_prepare_virtual_key_store(store))
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_request_header_virtual_key(
    request_id: u64,
    request: &super::local_rewrite_request::RuntimeLocalRewriteRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<
    Option<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
> {
    if path_without_query(request.url()) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH {
        return Ok(None);
    }
    let snapshot = match runtime_gateway_virtual_key_snapshot(shared.gateway_virtual_keys.lock()) {
        Ok(snapshot) => snapshot,
        Err(rejection) => {
            runtime_proxy_log_to_path(
                &shared.runtime_shared.log_path,
                &runtime_proxy_structured_log_message(
                    "gateway_virtual_key_state_unavailable",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
            return Err(rejection);
        }
    };
    if snapshot.active_keys.is_empty() && snapshot.configured_count > 0 {
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    }
    runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        request.headers(),
        &snapshot.active_keys,
    )
    .map(|key| key.cloned())
}

fn runtime_gateway_prepare_virtual_key_store(
    mut store: RuntimeGatewayVirtualKeyStoreFile,
) -> RuntimeGatewayVirtualKeyStoreFile {
    store.canonicalize_for_active_state();
    store.sort_for_rendering();
    store
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_admission_snapshot(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGatewayVirtualKeySnapshot, RuntimeGatewayVirtualKeyAdmissionFailure> {
    let snapshot = match runtime_gateway_virtual_key_snapshot(shared.gateway_virtual_keys.lock()) {
        Ok(snapshot) => snapshot,
        Err(rejection) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_virtual_key_state_unavailable",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
            return Err(rejection.into());
        }
    };
    if snapshot.active_keys.is_empty() && snapshot.configured_count > 0 {
        return Err(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken.into(),
        );
    }
    Ok(snapshot)
}
