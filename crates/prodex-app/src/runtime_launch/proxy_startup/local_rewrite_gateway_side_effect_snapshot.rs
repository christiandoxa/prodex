use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use crate::RuntimeGatewaySideEffectSnapshot;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub(super) fn gateway_snapshot_handle(
    shared: RuntimeLocalRewriteProxyShared,
) -> Arc<dyn Fn() -> RuntimeGatewaySideEffectSnapshot + Send + Sync> {
    Arc::new(move || runtime_gateway_side_effect_snapshot(&shared))
}

fn runtime_gateway_side_effect_snapshot(
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeGatewaySideEffectSnapshot {
    fn fingerprint(value: &impl std::fmt::Debug) -> u64 {
        let mut hasher = DefaultHasher::new();
        format!("{value:?}").hash(&mut hasher);
        hasher.finish()
    }

    let runtime_state_fingerprint = shared
        .runtime_shared
        .runtime
        .lock()
        .map(|state| fingerprint(&*state))
        .unwrap_or(u64::MAX);
    let model_memory_fingerprint = shared
        .model_memory
        .lock()
        .map(|memory| fingerprint(&*memory))
        .unwrap_or(u64::MAX);
    let admin_idempotency_fingerprint = shared
        .gateway_admin_idempotency_keys
        .lock()
        .map(|keys| fingerprint(&*keys))
        .unwrap_or(u64::MAX);
    macro_rules! lock_len {
        ($lock:expr) => {
            $lock
                .lock()
                .map(|values| values.len())
                .unwrap_or(usize::MAX)
        };
    }

    RuntimeGatewaySideEffectSnapshot {
        runtime_state_fingerprint,
        model_memory_fingerprint,
        api_key_cursor: shared.api_key_cursor.load(Ordering::Relaxed),
        credential_fingerprint: shared.gateway_credentials.current.load().fingerprint,
        admin_idempotency_fingerprint,
        oidc_cache_entries: lock_len!(shared.gateway_oidc_http_cache),
        pending_usage_deltas: lock_len!(shared.gateway_usage.pending_deltas),
        usage_request_ids: lock_len!(shared.gateway_usage.request_ids),
        usage_typed_request_ids: lock_len!(shared.gateway_usage.typed_request_ids),
        usage_call_ids: lock_len!(shared.gateway_usage.call_ids),
        usage_ledger_scopes: lock_len!(shared.gateway_usage.ledger_scopes),
        usage_durable_reservations: lock_len!(shared.gateway_usage.durable_reservations),
    }
}
