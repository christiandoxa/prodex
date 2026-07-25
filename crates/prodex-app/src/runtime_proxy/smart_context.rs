use super::*;
mod artifact_manifest;
mod artifact_refs;
mod body;
mod budgeting;
mod constants;
mod cooldown;
mod panic_guard;
mod proxy_state;
mod replay;
mod rewrite_telemetry;
mod rewrite_validation;
mod rollout;
mod runtime_rehydrate;
mod safety;
mod self_test;
mod static_context;
mod token_calibration;
mod types;

use artifact_manifest::*;
use artifact_refs::*;
use body::*;
use budgeting::*;
pub(crate) use budgeting::{
    runtime_smart_context_model_name_from_body, runtime_smart_context_normalized_model_name,
};
use constants::*;
use cooldown::*;
use panic_guard::*;
pub(crate) use proxy_state::*;
pub(crate) use replay::*;
use rewrite_telemetry::*;
use rewrite_validation::*;
use rollout::*;
use runtime_rehydrate::*;
use safety::*;
pub(crate) use self_test::*;
use static_context::*;
use std::borrow::Cow;
use token_calibration::*;
pub(crate) use types::RuntimeSmartContextEngine;
use types::*;

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

#[cfg(test)]
fn observe_runtime_smart_context_rewrite_safety(
    shared: &RuntimeRotationProxyShared,
    observation: RuntimeSmartContextRewriteSafetyObservation,
) {
    let Some(scope) = runtime_smart_context_scope_id(shared, None) else {
        return;
    };
    let Ok(mut states) = shared.smart_context_engine.states.write() else {
        return;
    };
    let mut save_job = None;
    if let Some(state) = states.get_mut(&scope)
        && state.enabled
    {
        state.generation = state.generation.saturating_add(1);
        observe_runtime_smart_context_rewrite_safety_with_state(state, observation);
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

fn observe_runtime_smart_context_rewrite_safety_with_state(
    state: &mut RuntimeSmartContextProxyState,
    observation: RuntimeSmartContextRewriteSafetyObservation,
) {
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
}

pub(crate) fn prepare_runtime_smart_context_http_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> std::result::Result<Cow<'a, [u8]>, RuntimeSmartContextPrepareError> {
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
) -> std::result::Result<Cow<'a, [u8]>, RuntimeSmartContextPrepareError> {
    prepare_runtime_smart_context_body_safely(
        request_id,
        request,
        shared,
        route_kind,
        RuntimeSmartContextTransport::Http,
        profile_name,
    )
}

#[cfg(feature = "bench-support")]
pub(crate) fn runtime_smart_context_rehydrate_for_benchmark(
    mut value: serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
) -> usize {
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, store, &mut stats);
    serde_json::to_vec(&value).map_or(0, |body| body.len())
}

pub(super) fn prepare_runtime_smart_context_websocket_text<'a>(
    request_id: u64,
    request_text: &'a str,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> std::result::Result<Cow<'a, str>, RuntimeSmartContextPrepareError> {
    if !runtime_smart_context_enabled(shared) {
        return Ok(Cow::Borrowed(request_text));
    }

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: handshake_request.path_and_query.clone(),
        headers: handshake_request.headers.clone(),
        body: request_text.as_bytes().to_vec(),
    };
    Ok(
        match prepare_runtime_smart_context_body_safely(
            request_id,
            &request,
            shared,
            RuntimeRouteKind::Websocket,
            RuntimeSmartContextTransport::Websocket,
            Some(profile_name),
        )? {
            Cow::Borrowed(_) => Cow::Borrowed(request_text),
            Cow::Owned(body) => String::from_utf8(body)
                .map(Cow::Owned)
                .unwrap_or(Cow::Borrowed(request_text)),
        },
    )
}

fn prepare_runtime_smart_context_body_safely<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> std::result::Result<Cow<'a, [u8]>, RuntimeSmartContextPrepareError> {
    if let Some(body) = runtime_smart_context_exact_passthrough(request) {
        return Ok(body);
    }
    if !runtime_smart_context_enabled(shared) {
        return Ok(Cow::Borrowed(&request.body));
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
        return Ok(Cow::Borrowed(&request.body));
    }

    if runtime_take_fault_injection_budget(
        "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE",
        shared.runtime_config.fault_smart_context_panic_once,
    ) {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "fault_injection",
        );
        return Ok(Cow::Borrowed(&request.body));
    }

    if !matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "unsupported_route",
        );
        return Ok(Cow::Borrowed(&request.body));
    }
    if runtime_proxy_crate::runtime_proxy_request_header_value(&request.headers, "content-type")
        .is_some_and(|value| !value.to_ascii_lowercase().starts_with("application/json"))
    {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "unsupported_content_type",
        );
        return Ok(Cow::Borrowed(&request.body));
    }

    let rollout = runtime_smart_context_rollout_decision(
        request_id,
        request,
        shared,
        route_kind,
        transport,
        profile_name,
    );
    let shadow = rollout.mode == runtime_proxy_crate::SmartContextRolloutMode::Shadow;
    if rollout.mode == runtime_proxy_crate::SmartContextRolloutMode::Disabled
        || (shadow && rollout.canary_bucket >= SMART_CONTEXT_SHADOW_SAMPLE_BASIS_POINTS)
    {
        let reason = if shadow {
            "rollout_shadow_sampled_out"
        } else {
            "rollout_canary_out"
        };
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            reason,
        );
        return Ok(Cow::Borrowed(&request.body));
    }

    let result = catch_runtime_smart_context_unwind_silently(|| {
        if runtime_take_fault_injection_budget(
            "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_UNWIND_ONCE",
            shared.runtime_config.fault_smart_context_unwind_once,
        ) {
            std::panic::panic_any(RuntimeSmartContextInjectedPanic);
        }
        if request.body.len() < SMART_CONTEXT_ADMISSION_MIN_BODY_BYTES
            && !runtime_smart_context_body_may_contain_artifact_ref(&request.body)
        {
            runtime_smart_context_log_prepare_fallback(
                request_id,
                shared,
                route_kind,
                transport,
                profile_name,
                request.body.len(),
                "below_minimum_body",
            );
            return Ok(Cow::Borrowed(request.body.as_slice()));
        }
        if transport == RuntimeSmartContextTransport::Websocket
            && runtime_smart_context_websocket_generate_false_request(&request.body)
        {
            runtime_smart_context_log_prepare_fallback(
                request_id,
                shared,
                route_kind,
                transport,
                profile_name,
                request.body.len(),
                "websocket_generate_false",
            );
            return Ok(Cow::Borrowed(request.body.as_slice()));
        }
        let rewrite_max_bytes = if transport == RuntimeSmartContextTransport::Websocket {
            SMART_CONTEXT_WEBSOCKET_REWRITE_MAX_BYTES
        } else {
            SMART_CONTEXT_HTTP_REWRITE_MAX_BYTES
        };
        if request.body.len() > rewrite_max_bytes
            && !runtime_smart_context_body_may_contain_artifact_ref(&request.body)
        {
            runtime_smart_context_log_prepare_fallback(
                request_id,
                shared,
                route_kind,
                transport,
                profile_name,
                request.body.len(),
                if transport == RuntimeSmartContextTransport::Websocket {
                    "websocket_large_payload"
                } else {
                    "body_too_large"
                },
            );
            return Ok(Cow::Borrowed(request.body.as_slice()));
        }
        prepare_runtime_smart_context_body(
            request_id,
            request,
            shared,
            route_kind,
            transport,
            profile_name,
            &rollout,
        )
    });

    match result {
        Ok(Ok(body)) => Ok(body),
        Ok(Err(error)) => {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_prepare_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", transport.label()),
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", profile_name.unwrap_or("-")),
                        runtime_proxy_log_field("reason", "missing_artifact_refs"),
                        runtime_proxy_log_field(
                            "missing_artifact_count",
                            error.missing_artifact_count.to_string(),
                        ),
                    ],
                ),
            );
            Err(error)
        }
        Err(panic) => {
            let disabled_until = runtime_smart_context_disable_temporarily(shared, now);
            runtime_smart_context_log_panic(
                request_id,
                shared,
                route_kind,
                transport,
                profile_name,
                request.body.len(),
                panic.as_ref(),
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
            Ok(Cow::Borrowed(&request.body))
        }
    }
}

fn runtime_smart_context_exact_passthrough<'a>(
    request: &'a RuntimeProxyRequest,
) -> Option<Cow<'a, [u8]>> {
    runtime_smart_context_exact_header(request).then_some(Cow::Borrowed(request.body.as_slice()))
}

fn runtime_smart_context_websocket_generate_false_request(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    value.get("type").and_then(serde_json::Value::as_str) == Some("response.create")
        && value.get("generate").and_then(serde_json::Value::as_bool) == Some(false)
}

fn runtime_smart_context_body_may_contain_artifact_ref(body: &[u8]) -> bool {
    std::str::from_utf8(body).is_ok_and(|text| {
        text.contains("psc:") || text.contains("psc2:") || text.contains("prodex-artifact:")
    })
}

fn runtime_smart_context_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    shared.smart_context_engine.is_enabled()
}

#[cfg(test)]
fn with_runtime_smart_context_artifacts<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextArtifactStore) -> R,
) -> Option<R> {
    with_runtime_smart_context_proxy_state(shared, |state| {
        action(Arc::make_mut(&mut state.artifacts))
    })
}

#[cfg(test)]
fn with_runtime_smart_context_proxy_state<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextProxyState) -> R,
) -> Option<R> {
    let scope = runtime_smart_context_scope_id(shared, None)?;
    let mut states = shared.smart_context_engine.states.write().ok()?;
    let state = states.get_mut(&scope)?;
    if !state.enabled {
        return None;
    }
    let result = action(state);
    state.generation = state.generation.saturating_add(1);
    Some(result)
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/smart_context.rs"]
mod tests;
