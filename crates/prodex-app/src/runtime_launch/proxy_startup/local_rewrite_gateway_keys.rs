use super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, RuntimeGatewayVirtualKeyUsageDelta,
    RuntimeLocalRewriteProxyShared, schedule_runtime_gateway_virtual_key_usage_save,
};
use super::local_rewrite_gateway_budget::runtime_gateway_budget_group_rejection;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_key_store_backend::{
    runtime_gateway_postgres_load_key_store, runtime_gateway_redis_load_key_store,
    runtime_gateway_sqlite_load_key_store,
};
use super::local_rewrite_gateway_store_file::runtime_gateway_virtual_key_store_file_load;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeyEntry, RuntimeGatewayVirtualKeySource,
    RuntimeGatewayVirtualKeyStoreFile, runtime_gateway_virtual_key_entry_from_stored,
};
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_seconds;
use super::provider_bridge::runtime_provider_gateway_cost_for_request;
use super::*;
use anyhow::Result;
use prodex_domain::{CallId, RequestId};
use prodex_provider_core::{calculate_cost_microusd, estimate_request_input_tokens};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn runtime_gateway_virtual_key_entries_is_empty(
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.is_empty())
        .unwrap_or(false)
}

struct RuntimeGatewayVirtualKeySnapshot {
    active_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    configured_count: usize,
}

fn runtime_gateway_virtual_key_snapshot(
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

fn runtime_gateway_route_load_snapshot(
    route_load: std::sync::LockResult<
        std::sync::MutexGuard<
            '_,
            BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
        >,
    >,
) -> Result<
    BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
> {
    route_load
        .map(|route_load| route_load.clone())
        .map_err(|_| runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)
}

fn runtime_gateway_admit_virtual_key(
    usage: &std::sync::Mutex<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>>,
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    active_keys: &[runtime_proxy_crate::RuntimeGatewayVirtualKey],
    body: &[u8],
    estimated_cost_microusd: Option<u64>,
    minute_epoch: u64,
) -> Result<
    runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission,
    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
> {
    let mut usage = usage.lock().map_err(|_| {
        runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
    })?;
    if let Some(rejection) =
        runtime_gateway_budget_group_rejection(key, active_keys, &usage, estimated_cost_microusd)
    {
        return Err(rejection);
    }
    let admission = runtime_proxy_crate::runtime_gateway_virtual_key_admission(
        key,
        usage.get(&key.name),
        body,
        estimated_cost_microusd,
        minute_epoch,
    )?;
    runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(
        usage.entry(admission.key_name.clone()).or_default(),
        &admission,
        minute_epoch,
    );
    Ok(admission)
}

pub(super) fn runtime_gateway_virtual_key_entries_from_sources(
    policy_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<Vec<RuntimeGatewayVirtualKeyEntry>> {
    let mut entries = policy_keys
        .into_iter()
        .map(|key| RuntimeGatewayVirtualKeyEntry {
            tenant_id: key.tenant_id.clone(),
            key,
            source: RuntimeGatewayVirtualKeySource::Policy,
            created_at_epoch: None,
            updated_at_epoch: None,
            disabled: false,
        })
        .collect::<Vec<_>>();
    let mut seen = entries
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for record in runtime_gateway_virtual_key_store_load_strict(state_store, log_path)?.keys {
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
        let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(&record) else {
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
    Ok(entries)
}

pub(super) fn runtime_gateway_virtual_key_store_load_strict(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let path = state_store.key_store_path();
    let mut store = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => runtime_gateway_sqlite_load_key_store(path),
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_postgres_load_key_store(url)
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
    store.sort_for_rendering();
    Ok(store)
}

pub(super) fn runtime_gateway_virtual_key_store_load(
    state_store: &RuntimeGatewayStateStore,
    log_path: &Path,
) -> RuntimeGatewayVirtualKeyStoreFile {
    runtime_gateway_virtual_key_store_load_strict(state_store, log_path).unwrap_or_default()
}

pub(super) fn runtime_gateway_request_header_rejection(
    request_id: u64,
    request: &tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    if path_without_query(request.url()) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH {
        return None;
    }
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
            return Some(rejection);
        }
    };
    if snapshot.active_keys.is_empty() && snapshot.configured_count > 0 {
        return Some(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    }
    let headers = request
        .headers()
        .iter()
        .map(|header| (header.field.to_string(), header.value.as_str().to_string()))
        .collect::<Vec<_>>();
    runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(&headers, &snapshot.active_keys)
        .err()
}

pub(super) fn runtime_gateway_virtual_key_admission(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<Option<String>, runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    if path_without_query(&captured.path_and_query) == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
    {
        return Ok(None);
    }
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
            return Err(rejection);
        }
    };
    let active_keys = snapshot.active_keys;
    if active_keys.is_empty() && snapshot.configured_count > 0 {
        return Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    }
    let key = match runtime_proxy_crate::runtime_gateway_virtual_key_from_headers(
        &captured.headers,
        &active_keys,
    ) {
        Ok(Some(key)) => key,
        Ok(None) => return Ok(None),
        Err(rejection) => return Err(rejection),
    };
    let model = runtime_proxy_crate::runtime_gateway_request_model(&captured.body)
        .unwrap_or_else(|| "unknown".to_string());
    let input_tokens = estimate_request_input_tokens(&captured.body);
    let route_load = match runtime_gateway_route_load_snapshot(shared.gateway_route_load.lock()) {
        Ok(route_load) => route_load,
        Err(rejection) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_route_load_state_unavailable",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
            return Err(rejection);
        }
    };
    let cost = runtime_provider_gateway_cost_for_request(
        shared.provider.bridge_kind(),
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        &model,
    );
    let estimated_cost_microusd = calculate_cost_microusd(Some(input_tokens), None, cost);
    let minute_epoch = runtime_proxy_crate::runtime_gateway_minute_epoch();
    let admission = match runtime_gateway_admit_virtual_key(
        &shared.gateway_usage.usage,
        key,
        &active_keys,
        &captured.body,
        estimated_cost_microusd,
        minute_epoch,
    ) {
        Ok(admission) => admission,
        Err(rejection) => {
            if rejection
                == runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "gateway_virtual_key_usage_state_unavailable",
                        [runtime_proxy_log_field("request", request_id.to_string())],
                    ),
                );
            }
            return Err(rejection);
        }
    };
    if let Ok(mut request_ids) = shared.gateway_usage.request_ids.lock() {
        request_ids.insert(request_id);
    }
    let typed_request_id = format!("prodex-{}", RequestId::new());
    if let Ok(mut typed_request_ids) = shared.gateway_usage.typed_request_ids.lock() {
        typed_request_ids.insert(request_id, typed_request_id.clone());
    }
    let call_id = format!("prodex-{}", CallId::new());
    if let Ok(mut call_ids) = shared.gateway_usage.call_ids.lock() {
        call_ids.insert(request_id, call_id.clone());
    }
    schedule_runtime_gateway_virtual_key_usage_save(
        shared,
        RuntimeGatewayVirtualKeyUsageDelta {
            request_id,
            typed_request_id,
            call_id,
            key_name: admission.key_name.clone(),
            tenant_id: key.tenant_id.clone(),
            team_id: key.team_id.clone(),
            project_id: key.project_id.clone(),
            user_id: key.user_id.clone(),
            budget_id: key.budget_id.clone(),
            model: model.clone(),
            minute_epoch,
            input_tokens: admission.input_tokens,
            estimated_cost_microusd: admission.estimated_cost_microusd,
            created_at_epoch: runtime_gateway_unix_epoch_seconds(),
        },
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_virtual_key_admitted",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("key", admission.key_name.as_str()),
                runtime_proxy_log_field("model", model.as_str()),
                runtime_proxy_log_field("input_tokens", admission.input_tokens.to_string()),
                runtime_proxy_log_field(
                    "estimated_cost_microusd",
                    admission
                        .estimated_cost_microusd
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                ),
            ],
        ),
    );
    Ok(Some(admission.key_name))
}

pub(super) fn runtime_local_rewrite_request_is_authorized(
    request: &tiny_http::Request,
    auth_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
) -> bool {
    let path = path_without_query(request.url());
    if path == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
        || path.ends_with("/prodex/gateway/admin")
    {
        return true;
    }
    request.headers().iter().any(|header| {
        header.field.equiv("Authorization")
            && auth_token_hash.verify_authorization_header(header.value.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex};

    #[test]
    fn key_store_load_failure_log_uses_stable_error_without_path_details() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("prodex-key-store-load-log-redaction-{nonce}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test root should be created");
        let key_store_path = root.join("gateway-virtual-keys.json");
        std::fs::create_dir_all(&key_store_path).expect("key store path directory should exist");
        let log_path = root.join("runtime.log");
        let state_store = RuntimeGatewayStateStore::File {
            key_store_path: key_store_path.clone(),
            usage_path: root.join("gateway-virtual-key-usage.json"),
            ledger_path: root.join("gateway-billing-ledger.jsonl"),
        };

        let store = runtime_gateway_virtual_key_store_load(&state_store, &log_path);
        assert!(store.keys.is_empty());
        let runtime_log = std::fs::read_to_string(&log_path).expect("runtime log should exist");
        assert!(runtime_log.contains("gateway_virtual_key_store_load_failed"));
        assert!(runtime_log.contains("error_kind=gateway_key_store_persistence_failed"));
        assert!(!runtime_log.contains("gateway-virtual-keys.json"));
        assert!(!runtime_log.contains(&root.display().to_string()));
        assert!(!runtime_log.contains("Is a directory"));
    }

    fn gateway_test_key(
        request_budget: Option<u64>,
    ) -> runtime_proxy_crate::RuntimeGatewayVirtualKey {
        runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret"),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn gateway_virtual_key_snapshot_rejects_poisoned_lock() {
        let entries = Arc::new(Mutex::new(vec![RuntimeGatewayVirtualKeyEntry {
            tenant_id: None,
            key: gateway_test_key(None),
            source: RuntimeGatewayVirtualKeySource::Policy,
            created_at_epoch: None,
            updated_at_epoch: None,
            disabled: false,
        }]));
        let poisoned = Arc::clone(&entries);
        let _ = std::panic::catch_unwind(move || {
            let _guard = poisoned.lock().expect("test lock should be acquired");
            panic!("poison gateway key lock");
        });

        assert!(matches!(
            runtime_gateway_virtual_key_snapshot(entries.lock()),
            Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)
        ));
    }

    #[test]
    fn gateway_policy_snapshots_reject_poisoned_budget_state() {
        let usage = Arc::new(Mutex::new(BTreeMap::<
            String,
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage,
        >::new()));
        let poisoned = Arc::clone(&usage);
        let _ = std::panic::catch_unwind(move || {
            let _guard = poisoned.lock().expect("test lock should be acquired");
            panic!("poison gateway usage lock");
        });

        let key = gateway_test_key(None);
        assert_eq!(
            runtime_gateway_admit_virtual_key(
                &usage,
                &key,
                std::slice::from_ref(&key),
                b"{}",
                None,
                1,
            ),
            Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)
        );
    }

    #[test]
    fn gateway_budget_admission_is_atomic() {
        let usage = Arc::new(Mutex::new(BTreeMap::new()));
        let key = Arc::new(gateway_test_key(Some(1)));
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let usage = Arc::clone(&usage);
                let key = Arc::clone(&key);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    runtime_gateway_admit_virtual_key(
                        &usage,
                        &key,
                        std::slice::from_ref(key.as_ref()),
                        br#"{"model":"gpt-5.4","input":"hello"}"#,
                        None,
                        1,
                    )
                })
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::RequestBudgetExceeded)))
                .count(),
            1
        );
    }
}
