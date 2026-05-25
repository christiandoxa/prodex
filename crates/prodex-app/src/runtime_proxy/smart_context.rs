use super::*;
mod artifact_manifest;
mod artifact_refs;
mod body;
mod budgeting;
mod constants;
mod intent;
mod panic_guard;
mod proxy_state;
mod rehydration;
mod repo_state;
mod rewrite_telemetry;
mod rewrite_validation;
mod runtime_rehydrate;
mod safety;
mod static_context;
mod token_calibration;
mod tool_outputs;
mod types;

use artifact_manifest::*;
use artifact_refs::*;
use body::*;
use budgeting::*;
pub(crate) use budgeting::{
    runtime_smart_context_model_name_from_body, runtime_smart_context_normalized_model_name,
};
use constants::*;
use intent::*;
use panic_guard::*;
pub(crate) use proxy_state::*;
use rehydration::*;
use repo_state::*;
use rewrite_telemetry::*;
use rewrite_validation::*;
use runtime_rehydrate::*;
use safety::*;
use static_context::*;
use std::borrow::Cow;
use std::path::PathBuf;
use token_calibration::*;
use tool_outputs::*;
use types::*;

static RUNTIME_SMART_CONTEXT_PROXY_STATES: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeSmartContextProxyState>>,
> = OnceLock::new();
const RUNTIME_SMART_CONTEXT_PANIC_COOLDOWN_SECS: u64 = 60;
static RUNTIME_SMART_CONTEXT_DISABLED_UNTIL: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> =
    OnceLock::new();

fn runtime_smart_context_disabled_until_for(shared: &RuntimeRotationProxyShared) -> u64 {
    let Some(disabled) = RUNTIME_SMART_CONTEXT_DISABLED_UNTIL.get() else {
        return 0;
    };
    let Ok(disabled) = disabled.lock() else {
        return 0;
    };
    disabled.get(&shared.log_path).copied().unwrap_or_default()
}

fn runtime_smart_context_disable_temporarily(shared: &RuntimeRotationProxyShared, now: u64) -> u64 {
    let disabled_until = now.saturating_add(RUNTIME_SMART_CONTEXT_PANIC_COOLDOWN_SECS);
    let disabled = RUNTIME_SMART_CONTEXT_DISABLED_UNTIL.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut disabled) = disabled.lock() {
        disabled.insert(shared.log_path.clone(), disabled_until);
    }
    disabled_until
}

pub(crate) fn runtime_smart_context_effective_prompt_cache_key(
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    allow_internal_derivation: bool,
) -> Option<String> {
    if let Some(prompt_cache_key) = runtime_request_prompt_cache_key(request) {
        return Some(prompt_cache_key);
    }
    if !allow_internal_derivation || !runtime_smart_context_enabled(shared) {
        return None;
    }
    runtime_smart_context_static_prompt_cache_key_from_body(&request.body)
}

pub(super) fn runtime_smart_context_effective_websocket_prompt_cache_key(
    request_text: &str,
    explicit_prompt_cache_key: Option<&str>,
    shared: &RuntimeRotationProxyShared,
    allow_internal_derivation: bool,
) -> Option<String> {
    if let Some(prompt_cache_key) = explicit_prompt_cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(prompt_cache_key.to_string());
    }
    if !allow_internal_derivation || !runtime_smart_context_enabled(shared) {
        return None;
    }
    runtime_smart_context_static_prompt_cache_key_from_body(request_text.as_bytes())
}

fn observe_runtime_smart_context_rewrite_safety(
    shared: &RuntimeRotationProxyShared,
    observation: RuntimeSmartContextRewriteSafetyObservation,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(mut states) = states.lock() else {
        return;
    };
    let mut save_job = None;
    if let Some(state) = states.get_mut(&shared.log_path)
        && state.enabled
    {
        let now = runtime_smart_context_unix_secs_now();
        state
            .rewrite_safety_history
            .retain(|record| runtime_smart_context_rewrite_safety_record_fresh(*record, now));
        state
            .rewrite_safety_history
            .push(RuntimeSmartContextRewriteSafetyRecord {
                observation,
                observed_at_unix_secs: now,
            });
        if state.rewrite_safety_history.len() > SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT {
            let overflow = state
                .rewrite_safety_history
                .len()
                .saturating_sub(SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT);
            state.rewrite_safety_history.drain(0..overflow);
        }
        save_job = state.artifact_path.as_deref().map(|artifact_path| {
            (
                runtime_smart_context_token_calibration_path(artifact_path),
                runtime_smart_context_token_calibration_snapshot(state),
            )
        });
    }
    drop(states);
    if let Some((path, snapshot)) = save_job {
        schedule_runtime_smart_context_token_calibration_save(
            shared,
            path,
            snapshot,
            "smart_context_rewrite_safety",
        );
    }
}

fn persist_runtime_smart_context_token_calibration_metadata(
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(states) = states.lock() else {
        return;
    };
    let Some(state) = states.get(&shared.log_path) else {
        return;
    };
    if !state.enabled {
        return;
    }
    let Some(artifact_path) = state.artifact_path.as_deref() else {
        return;
    };
    let save_job = (
        runtime_smart_context_token_calibration_path(artifact_path),
        runtime_smart_context_token_calibration_snapshot(state),
    );
    drop(states);
    schedule_runtime_smart_context_token_calibration_save(shared, save_job.0, save_job.1, reason);
}

pub(crate) fn prepare_runtime_smart_context_http_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> Cow<'a, [u8]> {
    prepare_runtime_smart_context_http_body_for_profile(
        request_id, request, shared, route_kind, None,
    )
}

pub(crate) fn prepare_runtime_smart_context_http_body_for_profile<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    profile_name: Option<&str>,
) -> Cow<'a, [u8]> {
    prepare_runtime_smart_context_body_safely(
        request_id,
        request,
        shared,
        route_kind,
        RuntimeSmartContextTransport::Http,
        profile_name,
    )
}

pub(super) fn prepare_runtime_smart_context_websocket_text<'a>(
    request_id: u64,
    request_text: &'a str,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Cow<'a, str> {
    if !runtime_smart_context_enabled(shared) {
        return Cow::Borrowed(request_text);
    }

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: handshake_request.path_and_query.clone(),
        headers: handshake_request.headers.clone(),
        body: request_text.as_bytes().to_vec(),
    };
    match prepare_runtime_smart_context_body_safely(
        request_id,
        &request,
        shared,
        RuntimeRouteKind::Websocket,
        RuntimeSmartContextTransport::Websocket,
        Some(profile_name),
    ) {
        Cow::Borrowed(_) => Cow::Borrowed(request_text),
        Cow::Owned(body) => String::from_utf8(body)
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(request_text)),
    }
}

fn prepare_runtime_smart_context_body_safely<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> Cow<'a, [u8]> {
    if !runtime_smart_context_enabled(shared) {
        return Cow::Borrowed(&request.body);
    }
    let now = runtime_smart_context_unix_secs_now();
    let disabled_until = runtime_smart_context_disabled_until_for(shared);
    if disabled_until > now {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "panic_cooldown",
        );
        return Cow::Borrowed(&request.body);
    }

    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE") {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "fault_injection",
        );
        return Cow::Borrowed(&request.body);
    }

    let result = catch_runtime_smart_context_unwind_silently(|| {
        if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_SMART_CONTEXT_UNWIND_ONCE") {
            std::panic::panic_any(RuntimeSmartContextInjectedPanic);
        }
        prepare_runtime_smart_context_body(
            request_id,
            request,
            shared,
            route_kind,
            transport,
            profile_name,
        )
    });

    match result {
        Ok(body) => body,
        Err(panic) => {
            let disabled_until = runtime_smart_context_disable_temporarily(shared, now);
            runtime_smart_context_log_panic(
                request_id,
                shared,
                route_kind,
                transport,
                profile_name,
                request.body.len(),
                &panic,
            );
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_disabled",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", transport.label()),
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", profile_name.unwrap_or("-")),
                        runtime_proxy_log_field("reason", "panic"),
                        runtime_proxy_log_field("until", disabled_until.to_string()),
                    ],
                ),
            );
            Cow::Borrowed(&request.body)
        }
    }
}

fn runtime_smart_context_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return false;
    };
    let Ok(states) = states.lock() else {
        return false;
    };
    states
        .get(&shared.log_path)
        .is_some_and(|state| state.enabled)
}

fn runtime_smart_context_observe_static_context(
    shared: &RuntimeRotationProxyShared,
    value: &serde_json::Value,
) -> RuntimeSmartContextStaticContextObservation {
    let cache = runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(
        runtime_smart_context_static_context_items(value),
    );
    if cache.items.is_empty() {
        return RuntimeSmartContextStaticContextObservation::default();
    }

    let current = cache
        .items
        .iter()
        .map(|item| runtime_proxy_crate::SmartContextFingerprint {
            id: item.id.clone(),
            kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
            content_hash: item.content_hash.clone(),
            byte_len: item.byte_len,
        })
        .collect::<Vec<_>>();

    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };
    let Ok(mut states) = states.lock() else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };
    let Some(state) = states.get_mut(&shared.log_path) else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };

    let seen_before = !state.last_static_context_fingerprints.is_empty();
    let delta = if seen_before {
        runtime_proxy_crate::smart_context_fingerprint_delta(
            state.last_static_context_fingerprints.clone(),
            current.clone(),
        )
    } else {
        Vec::new()
    };
    let changed = delta
        .iter()
        .any(runtime_smart_context_fingerprint_change_is_substantive);
    let changed_item_ids =
        runtime_smart_context_substantive_static_context_changed_item_ids(&delta);
    let observation = RuntimeSmartContextStaticContextObservation {
        seen_before,
        changed,
        item_count: current.len(),
        delta_count: delta.len(),
        prompt_cache_hash: Some(cache.content_hash.clone()),
        changed_item_ids,
    };
    state.last_static_context_fingerprints = current;
    state.last_static_context_prompt_cache_hash = Some(cache.content_hash);
    state.artifacts.set_static_context_fingerprints(
        observation.prompt_cache_hash.clone(),
        state.last_static_context_fingerprints.clone(),
    );
    let save_job = state
        .artifact_path
        .clone()
        .map(|path| (path, state.artifacts.clone()));
    drop(states);
    if let Some((path, store)) = save_job {
        schedule_runtime_smart_context_artifact_save(
            shared,
            path,
            store,
            "smart_context_static_fingerprints",
        );
    }
    observation
}

fn with_runtime_smart_context_artifacts<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextArtifactStore) -> R,
) -> Option<R> {
    with_runtime_smart_context_proxy_state(shared, |state| action(&mut state.artifacts))
}

fn with_runtime_smart_context_proxy_state<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextProxyState) -> R,
) -> Option<R> {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get()?;
    let mut states = states.lock().ok()?;
    let state = states.get_mut(&shared.log_path)?;
    state.enabled.then(|| action(state))
}

fn persist_runtime_smart_context_artifacts(shared: &RuntimeRotationProxyShared) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(states) = states.lock() else {
        return;
    };
    let Some(state) = states.get(&shared.log_path) else {
        return;
    };
    if !state.enabled {
        return;
    }
    let Some(path) = state.artifact_path.clone() else {
        return;
    };
    let store = state.artifacts.clone();
    schedule_runtime_smart_context_artifact_save(shared, path, store, "smart_context_artifacts");
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/smart_context.rs"]
mod tests;
