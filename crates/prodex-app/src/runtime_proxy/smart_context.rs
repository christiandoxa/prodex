use super::*;
mod artifact_refs;
mod constants;
mod panic_guard;
mod rehydration;
mod repo_state;
mod rewrite_telemetry;
mod static_context;
mod token_calibration;
mod tool_outputs;
mod types;

use artifact_refs::*;
use constants::*;
use panic_guard::*;
use rehydration::*;
use repo_state::*;
use rewrite_telemetry::*;
use static_context::*;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use token_calibration::*;
use tool_outputs::*;
use types::*;

const RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH: usize = 64;
const RUNTIME_SMART_CONTEXT_MAX_JSON_NODES: usize = 50_000;

static RUNTIME_SMART_CONTEXT_PROXY_STATES: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeSmartContextProxyState>>,
> = OnceLock::new();

fn runtime_smart_context_unix_secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn runtime_smart_context_rewrite_safety_record_fresh(
    record: RuntimeSmartContextRewriteSafetyRecord,
    now: u64,
) -> bool {
    record.observed_at_unix_secs == 0
        || now.saturating_sub(record.observed_at_unix_secs) <= SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS
}

pub(crate) fn register_runtime_smart_context_proxy_state(
    log_path: &Path,
    enabled: bool,
    model_context_window_tokens: Option<u64>,
    artifact_path: Option<PathBuf>,
) {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut states) = states.lock() else {
        return;
    };
    let artifacts = artifact_path
        .as_deref()
        .filter(|_| enabled)
        .map(RuntimeSmartContextArtifactStore::load_from_path)
        .unwrap_or_default();
    let calibration = artifact_path
        .as_deref()
        .filter(|_| enabled)
        .map(runtime_smart_context_load_token_calibration_for_artifact_path)
        .unwrap_or_default();
    let token_usage_history = calibration
        .token_usage_history
        .into_iter()
        .map(RuntimeTokenUsage::from)
        .rev()
        .take(SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let token_calibration_history = calibration
        .token_calibration_history
        .into_iter()
        .map(RuntimeSmartContextTokenCalibrationObservation::from)
        .rev()
        .take(SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let now = runtime_smart_context_unix_secs_now();
    let rewrite_safety_history = calibration
        .rewrite_safety_history
        .into_iter()
        .map(RuntimeSmartContextRewriteSafetyRecord::from)
        .filter(|record| runtime_smart_context_rewrite_safety_record_fresh(*record, now))
        .rev()
        .take(SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let (artifact_aliases, next_artifact_alias_index) =
        runtime_smart_context_artifact_alias_state_from_persisted(calibration.artifact_aliases);
    let static_section_fingerprints =
        runtime_smart_context_static_section_fingerprint_state_from_persisted(
            calibration.static_section_fingerprints,
        );
    states.insert(
        log_path.to_path_buf(),
        RuntimeSmartContextProxyState {
            enabled,
            model_context_window_tokens,
            artifacts,
            artifact_path,
            last_token_usage: token_usage_history.last().copied(),
            token_usage_history,
            token_calibration_history,
            rewrite_telemetry_history: Vec::new(),
            rewrite_safety_history,
            last_static_context_fingerprints: Vec::new(),
            last_static_context_prompt_cache_hash: None,
            last_artifact_manifest_ids: BTreeSet::new(),
            last_artifact_manifest_emitted_at: None,
            artifact_aliases,
            next_artifact_alias_index,
            static_section_fingerprints,
            repo_state_facts: RuntimeSmartContextRepoStateFacts::default(),
        },
    );
}

#[cfg(test)]
pub(crate) fn observe_runtime_smart_context_token_usage(
    shared: &RuntimeRotationProxyShared,
    usage: RuntimeTokenUsage,
) {
    observe_runtime_smart_context_token_usage_for_bucket(shared, usage, None);
}

pub(crate) fn observe_runtime_smart_context_token_usage_for_bucket(
    shared: &RuntimeRotationProxyShared,
    usage: RuntimeTokenUsage,
    bucket_key: Option<runtime_proxy_crate::SmartContextTokenCalibrationBucketKey>,
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
        state.last_token_usage = Some(usage);
        state.token_usage_history.push(usage);
        if state.token_usage_history.len() > SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT {
            let overflow = state
                .token_usage_history
                .len()
                .saturating_sub(SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT);
            state.token_usage_history.drain(0..overflow);
        }
        if let Some(bucket_key) = bucket_key.clone() {
            state
                .token_calibration_history
                .push(RuntimeSmartContextTokenCalibrationObservation { bucket_key, usage });
            if state.token_calibration_history.len() > SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT
            {
                let overflow = state
                    .token_calibration_history
                    .len()
                    .saturating_sub(SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT);
                state.token_calibration_history.drain(0..overflow);
            }
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
            "smart_context_token_calibration",
        );
    }
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
            runtime_smart_context_log_panic(
                request_id,
                shared,
                route_kind,
                transport,
                profile_name,
                request.body.len(),
                &panic,
            );
            Cow::Borrowed(&request.body)
        }
    }
}

fn prepare_runtime_smart_context_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> Cow<'a, [u8]> {
    let budget = runtime_smart_context_budget(
        shared,
        &request.body,
        route_kind,
        transport,
        profile_name,
        runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        Vec::new(),
        false,
    );
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            "invalid_json",
            "pass_through",
            "-",
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };
    if let Some(reason) = runtime_smart_context_unsupported_json_shape_reason(&value) {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(budget.tier),
            "unsupported_json_shape",
            reason,
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    }

    let missing_rehydrate_refs = runtime_smart_context_missing_artifact_refs(&value, shared);
    let exactness = runtime_proxy_crate::smart_context_exactness_guard(
        runtime_proxy_crate::SmartContextExactnessInput {
            exact_mode: runtime_smart_context_exact_header(request),
            previous_response_id: runtime_request_previous_response_id(request),
            turn_state: runtime_request_turn_state(request),
            session_id: runtime_request_session_id(request),
            tool_output_without_artifact: false,
            missing_rehydrate_refs: missing_rehydrate_refs.clone(),
        },
    );
    let static_observation = runtime_smart_context_observe_static_context(shared, &value);
    let mut budget = runtime_smart_context_budget(
        shared,
        &request.body,
        route_kind,
        transport,
        profile_name,
        exactness.clone(),
        missing_rehydrate_refs.clone(),
        static_observation.changed,
    );
    let affinity_pressure_rewrite =
        runtime_smart_context_affinity_pressure_rewrite_allowed(&exactness, &budget);
    let transform_exactness = if affinity_pressure_rewrite {
        runtime_smart_context_affinity_pressure_rewrite_guard(&exactness)
    } else {
        exactness.clone()
    };
    if affinity_pressure_rewrite {
        budget = runtime_smart_context_budget(
            shared,
            &request.body,
            route_kind,
            transport,
            profile_name,
            transform_exactness.clone(),
            missing_rehydrate_refs.clone(),
            static_observation.changed,
        );
    }
    let tier = budget.tier;
    let intent_signals = runtime_smart_context_collect_intent_signals(&value);
    let rewrite_reason_label = if affinity_pressure_rewrite {
        "affinity_pressure"
    } else {
        "-"
    };

    if exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact
        && !affinity_pressure_rewrite
    {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "require_exact",
                &runtime_smart_context_reason_labels(&exactness.reasons),
                request.body.len(),
                body.len(),
                RuntimeSmartContextTransformStats::default(),
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "require_exact",
            &runtime_smart_context_reason_labels(&exactness.reasons),
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through_exact",
        );
        return Cow::Borrowed(&request.body);
    }

    let Some(mut outcome) = with_runtime_smart_context_proxy_state(shared, |state| {
        let mut outcome = RuntimeSmartContextTransformOutcome::default();
        let budget_allows_rewrite =
            budget.policy.mode != runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough;
        {
            let store = &mut state.artifacts;
            let rehydrate_plan = runtime_smart_context_auto_rehydrate_plan(
                &value,
                store,
                budget.available_tokens,
                tier,
            );
            outcome.deferred_rehydrate_refs =
                runtime_smart_context_deferred_rehydrate_refs(&rehydrate_plan);
            runtime_smart_context_rehydrate_value_with_plan(
                &mut value,
                store,
                &rehydrate_plan,
                &mut outcome.stats,
            );
            runtime_smart_context_selective_rehydrate_budget_aware_ranges(
                &mut value,
                store,
                &transform_exactness,
                &intent_signals.semantic_terms,
                &rehydrate_plan,
                budget
                    .available_tokens
                    .saturating_sub(rehydrate_plan.used_tokens),
                &mut outcome.stats,
            );
        }
        runtime_smart_context_apply_repo_state_micro_cache(
            &mut value,
            state,
            request_id,
            &transform_exactness,
            budget_allows_rewrite,
            &mut outcome.stats,
        );
        if budget_allows_rewrite {
            {
                let store = &mut state.artifacts;
                runtime_smart_context_condense_tool_outputs(
                    &mut value,
                    store,
                    request_id,
                    tier,
                    budget.policy.max_inline_tool_output_bytes,
                    &intent_signals,
                    &mut outcome.stats,
                );
                runtime_smart_context_condense_historical_tool_call_arguments(
                    &mut value,
                    store,
                    request_id,
                    tier,
                    budget.policy.max_inline_tool_output_bytes,
                    &mut outcome.stats,
                );
                runtime_smart_context_dedupe_input_text(
                    &mut value,
                    store,
                    &transform_exactness,
                    &mut outcome.stats,
                );
            }
        }
        if budget_allows_rewrite {
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut value,
                state,
                &outcome.stats,
                &intent_signals,
            );
        }
        outcome
    }) else {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "artifact_store_unavailable",
                rewrite_reason_label,
                request.body.len(),
                body.len(),
                RuntimeSmartContextTransformStats::default(),
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "artifact_store_unavailable",
            rewrite_reason_label,
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };
    runtime_smart_context_apply_static_context_persistent_section_dedupe(
        &mut value,
        shared,
        &transform_exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &transform_exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_cross_field_dedupe(
        &mut value,
        &transform_exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_chunk_dedupe(
        &mut value,
        &transform_exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_delta(
        &mut value,
        &static_observation,
        &transform_exactness,
        &mut outcome.stats,
    );
    let aliases_used = if outcome.stats != RuntimeSmartContextTransformStats::default() {
        with_runtime_smart_context_proxy_state(shared, |state| {
            runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
                &mut value, state,
            )
        })
        .unwrap_or(false)
    } else {
        false
    };
    if aliases_used {
        persist_runtime_smart_context_token_calibration_metadata(
            shared,
            "smart_context_artifact_aliases",
        );
    }
    let path_aliases_used = runtime_smart_context_apply_path_aliases_to_generated_texts(&mut value);
    let generated_aliases_used = aliases_used || path_aliases_used;
    let stats = outcome.stats.clone();
    if stats.artifacts_stored > 0 {
        persist_runtime_smart_context_artifacts(shared);
    }

    if stats == RuntimeSmartContextTransformStats::default() && !generated_aliases_used {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "minified",
                rewrite_reason_label,
                request.body.len(),
                body.len(),
                stats,
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "pass_through",
            rewrite_reason_label,
            request.body.len(),
            request.body.len(),
            stats,
            &budget,
            "noop",
        );
        return Cow::Borrowed(&request.body);
    }

    let Ok(body) = serde_json::to_vec(&value) else {
        return Cow::Borrowed(&request.body);
    };
    let self_check =
        runtime_smart_context_rewrite_self_check(request.body.len(), body.len(), &stats);
    let mut unresolved_rehydrate_refs = missing_rehydrate_refs;
    unresolved_rehydrate_refs.extend(outcome.deferred_rehydrate_refs);
    let regression_check = runtime_smart_context_regression_self_check(
        &request.body,
        &body,
        transform_exactness.clone(),
        unresolved_rehydrate_refs.clone(),
    );
    let critical_signal_check =
        runtime_smart_context_critical_signal_self_check(&request.body, &body);
    if critical_signal_check.has_loss()
        && let Some((repaired_body, repaired_stats)) =
            runtime_smart_context_try_surgical_rehydrate_critical_ranges(
                &value,
                shared,
                &request.body,
                &transform_exactness,
                &unresolved_rehydrate_refs,
                &stats,
            )
    {
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: true,
                saved_tokens: runtime_smart_context_saved_tokens(
                    request.body.len(),
                    repaired_body.len(),
                ),
            },
        );
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "rewritten",
            if affinity_pressure_rewrite {
                "affinity_pressure,surgical_rehydrate"
            } else {
                "surgical_rehydrate"
            },
            request.body.len(),
            repaired_body.len(),
            repaired_stats,
            &budget,
            "ok_surgical_rehydrate",
        );
        return Cow::Owned(repaired_body);
    }
    if let Some(fallback_reason) = runtime_smart_context_fallback_exact_reason(
        &regression_check,
        critical_signal_check,
        &stats,
    ) {
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: false,
                saved_tokens: 0,
            },
        );
        if let Some(body) = runtime_smart_context_minified_json_body_from_original(&request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "self_check_passthrough",
                rewrite_reason_label,
                request.body.len(),
                body.len(),
                stats,
                &budget,
                fallback_reason,
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "self_check_passthrough",
            rewrite_reason_label,
            request.body.len(),
            request.body.len(),
            stats,
            &budget,
            fallback_reason,
        );
        return Cow::Borrowed(&request.body);
    }
    observe_runtime_smart_context_rewrite_safety(
        shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: regression_check.saved_tokens,
        },
    );
    runtime_smart_context_log(
        request_id,
        shared,
        route_kind,
        transport,
        runtime_smart_context_tier_label(tier),
        "rewritten",
        rewrite_reason_label,
        request.body.len(),
        body.len(),
        stats,
        &budget,
        self_check,
    );
    Cow::Owned(body)
}

fn runtime_smart_context_unsupported_json_shape_reason(
    value: &serde_json::Value,
) -> Option<&'static str> {
    let mut stack = vec![(value, 1usize)];
    let mut nodes = 0usize;
    while let Some((value, depth)) = stack.pop() {
        if depth > RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH {
            return Some("json_depth_limit");
        }
        nodes = nodes.saturating_add(1);
        if nodes > RUNTIME_SMART_CONTEXT_MAX_JSON_NODES {
            return Some("json_node_limit");
        }
        match value {
            serde_json::Value::Array(items) => {
                stack.extend(items.iter().map(|item| (item, depth.saturating_add(1))));
            }
            serde_json::Value::Object(object) => {
                stack.extend(object.values().map(|item| (item, depth.saturating_add(1))));
            }
            _ => {}
        }
    }
    None
}

fn runtime_smart_context_affinity_pressure_rewrite_allowed(
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
    budget: &RuntimeSmartContextBudget,
) -> bool {
    exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact
        && !exactness.reasons.is_empty()
        && exactness
            .reasons
            .iter()
            .all(runtime_smart_context_exactness_reason_is_affinity)
        && runtime_smart_context_budget_has_critical_pressure(budget)
        && !runtime_smart_context_budget_has_non_affinity_safety_block(budget)
}

fn runtime_smart_context_affinity_pressure_rewrite_guard(
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
) -> runtime_proxy_crate::SmartContextExactnessGuard {
    runtime_proxy_crate::SmartContextExactnessGuard {
        decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
        reasons: exactness.reasons.clone(),
    }
}

fn runtime_smart_context_exactness_reason_is_affinity(
    reason: &runtime_proxy_crate::SmartContextExactnessReason,
) -> bool {
    matches!(
        reason,
        runtime_proxy_crate::SmartContextExactnessReason::PreviousResponseAffinity
            | runtime_proxy_crate::SmartContextExactnessReason::TurnStateAffinity
            | runtime_proxy_crate::SmartContextExactnessReason::SessionAffinity
    )
}

fn runtime_smart_context_budget_has_critical_pressure(budget: &RuntimeSmartContextBudget) -> bool {
    budget.available_tokens == 0
        || budget.tier == runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal
}

fn runtime_smart_context_budget_has_non_affinity_safety_block(
    budget: &RuntimeSmartContextBudget,
) -> bool {
    budget.policy.reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged
                | runtime_proxy_crate::SmartContextBudgetPolicyReason::MissingRehydrateRefs
                | runtime_proxy_crate::SmartContextBudgetPolicyReason::UnknownTokenWindow
                | runtime_proxy_crate::SmartContextBudgetPolicyReason::UnsafeAccounting
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn runtime_smart_context_budget(
    shared: &RuntimeRotationProxyShared,
    body: &[u8],
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
    static_context_changed: bool,
) -> RuntimeSmartContextBudget {
    let model_name = runtime_smart_context_model_name_from_body(body);
    let bucket_key = runtime_smart_context_token_calibration_bucket_key_with_model(
        route_kind,
        transport,
        profile_name,
        model_name.as_deref(),
    );
    let (
        global_history,
        bucket_history,
        calibration_samples,
        configured_context_window_tokens,
        recent_rewrite_safety,
        rewrite_telemetry_samples,
    ) = runtime_smart_context_budget_inputs(shared, &bucket_key);
    let history = if bucket_history.is_empty() {
        global_history
    } else {
        bucket_history
    };
    let model_context_window_tokens =
        configured_context_window_tokens.unwrap_or(SMART_CONTEXT_FALLBACK_CONTEXT_WINDOW_TOKENS);
    let observed_context_tokens_u64 = history
        .last()
        .and_then(|usage| runtime_proxy_crate::smart_context_observed_usage_context_tokens(*usage));
    let observed_context_tokens =
        observed_context_tokens_u64.and_then(|tokens| usize::try_from(tokens).ok());
    let current_input_tokens = observed_context_tokens_u64.unwrap_or(0);
    let accounting = runtime_proxy_crate::smart_context_observed_token_accounting_with_calibration(
        runtime_proxy_crate::SmartContextObservedTokenAccountingCalibrationInput {
            accounting: runtime_proxy_crate::SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(model_context_window_tokens),
                reserved_output_tokens: SMART_CONTEXT_RESERVED_OUTPUT_TOKENS,
                current_input_tokens,
                current_request_body_bytes: body.len(),
                current_request_estimated_tokens: Some(
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body(body),
                ),
                observed_usage: history,
            },
            calibration_bucket_key: Some(bucket_key),
            calibration_samples,
        },
    );
    let available_context_tokens = accounting.available_context_tokens;
    let mut policy = runtime_proxy_crate::smart_context_adaptive_budget_policy(
        runtime_proxy_crate::SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard,
            accounting,
            recent_rewrite_safety,
            static_context_changed,
            missing_rehydrate_refs,
        },
    );
    let telemetry_decision = runtime_proxy_crate::smart_context_rewrite_telemetry_budget_decision(
        runtime_proxy_crate::SmartContextRewriteTelemetryBudgetInput {
            recent_rewrite_safety: Default::default(),
            telemetry_samples: rewrite_telemetry_samples,
        },
    );
    policy = runtime_proxy_crate::smart_context_apply_rewrite_budget_decision(
        policy,
        telemetry_decision,
        available_context_tokens,
    );
    let available_tokens = policy
        .max_rehydrate_tokens
        .min(usize::MAX as u64)
        .try_into()
        .unwrap_or(usize::MAX);
    RuntimeSmartContextBudget {
        tier: policy.tier,
        policy,
        model_context_window_tokens,
        model_context_window_source: if configured_context_window_tokens.is_some() {
            "launch_config"
        } else {
            "fallback"
        },
        available_tokens,
        observed_context_tokens,
        token_usage_source: if observed_context_tokens.is_some() {
            "runtime_usage"
        } else {
            "estimated_body"
        },
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

#[cfg(test)]
fn runtime_smart_context_token_calibration_bucket_key(
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
    runtime_smart_context_token_calibration_bucket_key_with_model(
        route_kind,
        transport,
        profile_name,
        None,
    )
}

fn runtime_smart_context_token_calibration_bucket_key_with_model(
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    model_name: Option<&str>,
) -> runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
    runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
        route: Some(runtime_route_kind_label(route_kind).to_string()),
        model: runtime_proxy_crate::smart_context_normalized_model_name(model_name),
        profile: profile_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        transport: Some(transport.label().to_string()),
    }
}

pub(crate) fn runtime_smart_context_model_name_from_body(body: &[u8]) -> Option<String> {
    runtime_proxy_crate::smart_context_model_name_from_body(body)
}

pub(crate) fn runtime_smart_context_normalized_model_name(value: Option<&str>) -> Option<String> {
    runtime_proxy_crate::smart_context_normalized_model_name(value)
}

fn runtime_smart_context_budget_inputs(
    shared: &RuntimeRotationProxyShared,
    bucket_key: &runtime_proxy_crate::SmartContextTokenCalibrationBucketKey,
) -> RuntimeSmartContextBudgetInputs {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Default::default(),
            Vec::new(),
        );
    };
    let Ok(states) = states.lock() else {
        return (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Default::default(),
            Vec::new(),
        );
    };
    states
        .get(&shared.log_path)
        .map(|state| {
            let calibration_samples = state
                .token_calibration_history
                .iter()
                .map(
                    |sample| runtime_proxy_crate::SmartContextTokenCalibrationSample {
                        bucket_key: Some(sample.bucket_key.clone()),
                        usage: sample.usage,
                    },
                )
                .collect::<Vec<_>>();
            let bucket_history = state
                .token_calibration_history
                .iter()
                .filter(|sample| &sample.bucket_key == bucket_key)
                .map(|sample| sample.usage)
                .collect::<Vec<_>>();
            (
                state.token_usage_history.clone(),
                bucket_history,
                calibration_samples,
                state.model_context_window_tokens,
                runtime_smart_context_recent_rewrite_safety(&state.rewrite_safety_history),
                runtime_smart_context_rewrite_telemetry_samples(&state.rewrite_telemetry_history),
            )
        })
        .unwrap_or_default()
}

fn runtime_smart_context_recent_rewrite_safety(
    history: &[RuntimeSmartContextRewriteSafetyRecord],
) -> runtime_proxy_crate::SmartContextRecentRewriteSafety {
    let mut safety = runtime_proxy_crate::SmartContextRecentRewriteSafety::default();
    let now = runtime_smart_context_unix_secs_now();
    for record in history
        .iter()
        .filter(|record| runtime_smart_context_rewrite_safety_record_fresh(**record, now))
    {
        let observation = record.observation;
        if observation.safe {
            safety.safe_rewrites = safety.safe_rewrites.saturating_add(1);
            safety.saved_tokens = safety.saved_tokens.saturating_add(observation.saved_tokens);
        } else {
            safety.fallback_rewrites = safety.fallback_rewrites.saturating_add(1);
        }
    }
    safety
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

fn runtime_smart_context_collect_intent_signals(
    value: &serde_json::Value,
) -> RuntimeSmartContextIntentSignals {
    let mut signals = RuntimeSmartContextIntentSignals {
        artifact_refs: runtime_smart_context_collect_rehydratable_artifact_refs(value),
        ..RuntimeSmartContextIntentSignals::default()
    };

    let artifact_ref_ids = signals
        .artifact_refs
        .iter()
        .map(|reference| reference.id.clone())
        .collect::<Vec<_>>();
    for id in artifact_ref_ids {
        signals.add_intent_text(&id);
    }

    runtime_smart_context_collect_user_intent_text(value, &mut signals);
    runtime_smart_context_collect_tool_intent_metadata(value, &mut signals);
    signals
}

impl RuntimeSmartContextIntentSignals {
    fn add_intent_text(&mut self, text: &str) {
        for term in prodex_context::extract_intent_terms_from_prompt(text) {
            if !self.add_intent_term(term) {
                break;
            }
        }
        for term in runtime_smart_context_error_code_terms_from_intent_text(text) {
            if !self.add_intent_term(term) {
                break;
            }
        }
        for kind in runtime_smart_context_command_kinds_from_intent_text(text) {
            self.semantic_terms.command_kinds.insert(kind.to_string());
        }
    }

    fn add_intent_term(&mut self, term: String) -> bool {
        if self.intent_terms.iter().any(|existing| existing == &term) {
            return true;
        }
        if self.intent_terms.len() >= prodex_context::MAX_EXTRACTED_INTENT_TERMS {
            return false;
        }
        self.add_semantic_term(&term);
        self.intent_terms.push(term);
        self.intent_terms.len() < prodex_context::MAX_EXTRACTED_INTENT_TERMS
    }

    fn add_command_kind_hint(&mut self, hint: prodex_context::CommandOutputKind) {
        self.command_kind_hints
            .insert(runtime_smart_context_command_kind_hint_label(hint).to_string());
    }

    fn add_semantic_term(&mut self, term: &str) {
        if runtime_smart_context_intent_term_is_error_code(term) {
            self.semantic_terms.error_codes.insert(term.to_string());
        } else if runtime_smart_context_intent_term_is_path(term) {
            self.semantic_terms.file_paths.insert(term.to_string());
        } else if runtime_smart_context_intent_term_is_symbol(term) {
            self.semantic_terms.test_symbols.insert(term.to_string());
        }
    }
}

fn runtime_smart_context_collect_user_intent_text(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    if let Some(text) = value.get("prompt").and_then(serde_json::Value::as_str) {
        signals.add_intent_text(text);
    }
    match value.get("input") {
        Some(serde_json::Value::String(text)) => signals.add_intent_text(text),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_smart_context_collect_user_intent_text_from_input_item(item, signals);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_collect_user_intent_text_from_input_item(
    item: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    if runtime_smart_context_value_is_static_context_item(item) {
        return;
    }
    let Some(object) = item.as_object() else {
        return;
    };
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if item_type.ends_with("_call_output") || item_type.ends_with("_call") {
        return;
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if role != "user" {
        return;
    }
    for key in ["content", "input_text", "text"] {
        if let Some(child) = object.get(key) {
            runtime_smart_context_collect_intent_text_parts(child, signals);
        }
    }
}

fn runtime_smart_context_collect_tool_intent_metadata(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return;
    };
    for item in input {
        let Some(object) = item.as_object() else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        let metadata = runtime_smart_context_tool_item_metadata(object);
        if let Some(command) = metadata.command.as_deref() {
            signals.add_intent_text(command);
        }
        if let Some(kind_hint) = metadata.kind_hint {
            signals.add_command_kind_hint(kind_hint);
        }
    }
}

fn runtime_smart_context_collect_intent_text_parts(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    match value {
        serde_json::Value::String(text) => signals.add_intent_text(text),
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_intent_text_parts(item, signals);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    runtime_smart_context_collect_intent_text_parts(item, signals);
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_intent_term_is_error_code(term: &str) -> bool {
    if term.eq_ignore_ascii_case("error")
        || runtime_smart_context_numeric_error_term(term, "exit_code_")
        || runtime_smart_context_numeric_error_term(term, "status_code_")
    {
        return true;
    }
    let rest = term
        .strip_prefix('E')
        .or_else(|| term.strip_prefix("TS"))
        .or_else(|| term.strip_prefix('F'));
    rest.is_some_and(|value| value.len() >= 3 && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn runtime_smart_context_numeric_error_term(term: &str, prefix: &str) -> bool {
    term.strip_prefix(prefix)
        .is_some_and(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn runtime_smart_context_intent_term_is_path(term: &str) -> bool {
    term.contains('/')
        || term.rsplit_once('.').is_some_and(|(_, extension)| {
            matches!(
                extension,
                "c" | "cc"
                    | "cpp"
                    | "css"
                    | "go"
                    | "h"
                    | "hpp"
                    | "html"
                    | "js"
                    | "jsx"
                    | "json"
                    | "md"
                    | "py"
                    | "rs"
                    | "toml"
                    | "ts"
                    | "tsx"
                    | "yaml"
                    | "yml"
            )
        })
}

fn runtime_smart_context_intent_term_is_symbol(term: &str) -> bool {
    if term.contains("::") || term.contains('#') {
        return true;
    }
    if term.contains('.') && !runtime_smart_context_intent_term_is_path(term) {
        return term
            .split('.')
            .all(runtime_smart_context_intent_symbol_segment);
    }
    runtime_smart_context_intent_symbol_segment(term)
        && (term.starts_with("test_")
            || term.ends_with("_test")
            || term.contains('_')
            || runtime_smart_context_intent_symbol_has_camel_shape(term))
}

fn runtime_smart_context_intent_symbol_segment(term: &str) -> bool {
    let mut chars = term.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn runtime_smart_context_intent_symbol_has_camel_shape(term: &str) -> bool {
    let mut previous_lowercase = false;
    for ch in term.chars() {
        if previous_lowercase && ch.is_ascii_uppercase() {
            return true;
        }
        previous_lowercase = ch.is_ascii_lowercase();
    }
    false
}

fn runtime_smart_context_error_code_terms_from_intent_text(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut terms = Vec::new();
    runtime_smart_context_push_numeric_error_terms(&lower, "exit code", "exit_code_", &mut terms);
    runtime_smart_context_push_numeric_error_terms(&lower, "exit_code", "exit_code_", &mut terms);
    runtime_smart_context_push_numeric_error_terms(
        &lower,
        "status code",
        "status_code_",
        &mut terms,
    );
    runtime_smart_context_push_numeric_error_terms(
        &lower,
        "status_code",
        "status_code_",
        &mut terms,
    );
    terms
}

fn runtime_smart_context_push_numeric_error_terms(
    text: &str,
    marker: &str,
    prefix: &str,
    terms: &mut Vec<String>,
) {
    let mut remaining = text;
    while let Some(index) = remaining.find(marker) {
        let after_marker = &remaining[index + marker.len()..];
        let after_separator = after_marker
            .trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == ':' || ch == '=');
        let digits = after_separator
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if !digits.is_empty() {
            let term = format!("{prefix}{digits}");
            if !terms.iter().any(|existing| existing == &term) {
                terms.push(term);
            }
        }
        remaining = after_marker;
    }
}

fn runtime_smart_context_command_kinds_from_intent_text(text: &str) -> Vec<&'static str> {
    let lower = text.to_ascii_lowercase();
    let mut kinds = Vec::new();
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(
            &lower,
            &["cargo test", "cargo nextest", "nextest run"],
        ),
        "cargo-test",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(
            &lower,
            &["cargo check", "cargo build", "cargo clippy"],
        ),
        "cargo-build",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(
            &lower,
            &["npm test", "pnpm test", "yarn test", "vitest", "jest"],
        ),
        "npm-test",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(&lower, &["git diff", "git show"]),
        "diff",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(&lower, &["pytest", "python -m pytest"]),
        "python",
    );
    kinds
}

fn runtime_smart_context_text_contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn runtime_smart_context_push_command_kind_if(
    kinds: &mut Vec<&'static str>,
    condition: bool,
    kind: &'static str,
) {
    if condition && !kinds.contains(&kind) {
        kinds.push(kind);
    }
}

fn runtime_smart_context_command_kind_hint_label(
    hint: prodex_context::CommandOutputKind,
) -> &'static str {
    match hint {
        prodex_context::CommandOutputKind::Auto => "auto",
        prodex_context::CommandOutputKind::GitStatus => "git-status",
        prodex_context::CommandOutputKind::GitDiff => "git-diff",
        prodex_context::CommandOutputKind::RustDiagnostics => "rust-diagnostics",
        prodex_context::CommandOutputKind::Diagnostics => "diagnostics",
        prodex_context::CommandOutputKind::GitLog => "git-log",
        prodex_context::CommandOutputKind::Search => "search",
        prodex_context::CommandOutputKind::FileList => "file-list",
        prodex_context::CommandOutputKind::LogStream => "log-stream",
        prodex_context::CommandOutputKind::NoisySuccess => "noisy-success",
        prodex_context::CommandOutputKind::Plain => "plain",
    }
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

fn runtime_smart_context_exact_header(request: &RuntimeProxyRequest) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context")
        .is_some_and(|value| value.eq_ignore_ascii_case("exact"))
}

fn runtime_smart_context_missing_artifact_refs(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
) -> Vec<String> {
    let ref_ids = runtime_smart_context_collect_rehydratable_artifact_ref_ids(value);
    if ref_ids.is_empty() {
        return Vec::new();
    }
    with_runtime_smart_context_artifacts(shared, |store| {
        ref_ids
            .into_iter()
            .filter(|id| !store.contains(id))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

fn runtime_smart_context_auto_rehydrate_plan(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    token_budget: usize,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> runtime_proxy_crate::SmartContextRehydratePlan {
    let mut refs_by_id = BTreeMap::<String, usize>::new();
    let mut available_ids = BTreeSet::<String>::new();
    for reference in runtime_smart_context_collect_rehydratable_artifact_refs(value) {
        if store.contains(&reference.id) {
            available_ids.insert(reference.id.clone());
        }
        let token_cost =
            runtime_smart_context_rehydrate_ref_token_cost(&reference, store).unwrap_or(0);
        refs_by_id
            .entry(reference.id)
            .and_modify(|cost| *cost = cost.saturating_add(token_cost))
            .or_insert(token_cost);
    }

    runtime_proxy_crate::smart_context_auto_rehydrate_plan(
        refs_by_id.into_iter().map(|(id, token_cost)| {
            runtime_proxy_crate::SmartContextRehydrateRef {
                id,
                token_cost,
                required: true,
            }
        }),
        available_ids,
        token_budget,
        tier,
    )
}

fn runtime_smart_context_rehydrate_ref_token_cost(
    reference: &RuntimeSmartContextArtifactReference,
    store: &RuntimeSmartContextArtifactStore,
) -> Option<usize> {
    let artifact_text = store.get_text(&reference.id)?;
    let rehydrated =
        runtime_smart_context_rehydrated_artifact_reference_text(&artifact_text, reference)?;
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(rehydrated.len())
        .try_into()
        .ok()
}

fn runtime_smart_context_deferred_rehydrate_refs(
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
) -> Vec<String> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { id, .. } => Some(id.clone()),
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { .. } => None,
        })
        .collect()
}

#[cfg(test)]
fn runtime_smart_context_rehydrate_value(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let plan = runtime_smart_context_auto_rehydrate_plan(
        value,
        store,
        usize::MAX,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
    );
    runtime_smart_context_rehydrate_value_with_plan(value, store, &plan, stats);
}

fn runtime_smart_context_rehydrate_value_with_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let rehydrate_ids = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { id, .. } => {
                Some(id.clone())
            }
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    runtime_smart_context_rehydrate_value_for_ids(value, store, &rehydrate_ids, &aliases, stats);
}

fn runtime_smart_context_rehydrate_value_for_ids(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    rehydrate_ids: &BTreeSet<String>,
    aliases: &BTreeMap<String, String>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text) => {
            let mut next = text.clone();
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                if rehydrate_ids.contains(&reference.id)
                    && let Some(artifact_text) = store.get_text(&reference.id)
                    && let Some(rehydrated_text) =
                        runtime_smart_context_rehydrated_artifact_reference_text(
                            &artifact_text,
                            &reference,
                        )
                {
                    let legacy_marker = format!("prodex-artifact:{}", reference.id);
                    let short_marker = runtime_smart_context_artifact_ref(&reference.id);
                    if reference.line_range.is_none()
                        && runtime_smart_context_text_is_artifact_marker_summary(
                            &next,
                            &reference.id,
                        )
                    {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                        break;
                    } else if next.contains(&reference.marker) {
                        next = next.replace(&reference.marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&legacy_marker) {
                        next = next.replace(&legacy_marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&short_marker) {
                        next = next.replace(&short_marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.trim() == reference.id {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                    }
                }
            }
            *text = next;
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_rehydrate_value_for_ids(
                    item,
                    store,
                    rehydrate_ids,
                    aliases,
                    stats,
                );
            }
        }
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_rehydrate_value_for_ids(
                    item,
                    store,
                    rehydrate_ids,
                    aliases,
                    stats,
                );
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_text_is_artifact_marker_summary(text: &str, id: &str) -> bool {
    let Some(first_line) = text.trim_start().lines().next() else {
        return false;
    };
    let legacy = (first_line.starts_with("prodex-sc artifact ")
        || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&format!("prodex-artifact:{id}"));
    let short = (first_line.starts_with("psc art ")
        || first_line.starts_with("psc rep ")
        || first_line.starts_with("psc repeat ")
        || first_line.starts_with("psc co ")
        || first_line.starts_with("psc cmdout ")
        || first_line.starts_with("prodex-sc artifact ")
        || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&runtime_smart_context_artifact_ref(id));
    legacy || short
}

fn runtime_smart_context_rehydrated_artifact_text(
    artifact_text: &str,
    line_range: Option<RuntimeSmartContextLineRange>,
) -> Option<String> {
    let Some(line_range) = line_range else {
        return Some(artifact_text.to_string());
    };
    let lines = artifact_text.lines().collect::<Vec<_>>();
    if line_range.start > lines.len() {
        return None;
    }
    let end = line_range.end.min(lines.len());
    Some(lines[line_range.start - 1..end].join("\n"))
}

fn runtime_smart_context_rehydrated_artifact_reference_text(
    artifact_text: &str,
    reference: &RuntimeSmartContextArtifactReference,
) -> Option<String> {
    if reference.line_ranges.is_empty() {
        return runtime_smart_context_rehydrated_artifact_text(artifact_text, reference.line_range);
    }
    let mut parts = Vec::new();
    for range in &reference.line_ranges {
        parts.push(runtime_smart_context_rehydrated_artifact_text(
            artifact_text,
            Some(*range),
        )?);
    }
    Some(parts.join("\n"))
}

#[cfg(test)]
fn runtime_smart_context_selective_rehydrate_semantic_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    runtime_smart_context_selective_rehydrate_semantic_ranges_with_budget(
        value,
        store,
        exactness_guard,
        terms,
        usize::MAX,
        stats,
    )
}

fn runtime_smart_context_selective_rehydrate_budget_aware_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    token_budget: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return 0;
    }

    let mut remaining_tokens = token_budget;
    let strong_terms = runtime_smart_context_selective_rehydrate_terms_strong(terms);
    let mut count = if strong_terms {
        runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
            value,
            store,
            terms,
            &mut remaining_tokens,
            stats,
        )
    } else {
        0
    };

    let deferred_ids = runtime_smart_context_token_budget_deferred_rehydrate_ids(plan);
    if strong_terms && !deferred_ids.is_empty() && remaining_tokens > 0 {
        count = count.saturating_add(
            runtime_smart_context_selective_rehydrate_deferred_read_plan(
                value,
                store,
                terms,
                &deferred_ids,
                &mut remaining_tokens,
                stats,
            ),
        );
    }
    count
}

#[cfg(test)]
fn runtime_smart_context_selective_rehydrate_semantic_ranges_with_budget(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    token_budget: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow
        || runtime_smart_context_selective_rehydrate_terms_empty(terms)
    {
        return 0;
    }
    let mut remaining_tokens = token_budget;
    runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
        value,
        store,
        terms,
        &mut remaining_tokens,
        stats,
    )
}

fn runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
                text,
                store,
                terms,
                remaining_tokens,
                stats,
            )
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
                    item,
                    store,
                    terms,
                    remaining_tokens,
                    stats,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(
                    runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
                        item,
                        store,
                        terms,
                        remaining_tokens,
                        stats,
                    ),
                );
            }
            count
        }
        _ => 0,
    }
}

fn runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if *remaining_tokens == 0 {
        return 0;
    }
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some(line_index) = store.line_index(&id) else {
            continue;
        };
        let Some((appendix, range_count, token_cost)) =
            runtime_smart_context_matching_semantic_range_appendix_with_budget(
                &id,
                line_index,
                &next,
                terms,
                *remaining_tokens,
            )
        else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
        runtime_smart_context_consume_rehydrate_budget(remaining_tokens, token_cost);
        if *remaining_tokens == 0 {
            break;
        }
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

fn runtime_smart_context_token_budget_deferred_rehydrate_ids(
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
) -> BTreeSet<String> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Defer {
                id,
                reason:
                    runtime_proxy_crate::SmartContextRehydrateDeferReason::TokenBudgetExceeded
                    | runtime_proxy_crate::SmartContextRehydrateDeferReason::MinimalBudgetTier,
            } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn runtime_smart_context_selective_rehydrate_deferred_read_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    deferred_ids: &BTreeSet<String>,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if *remaining_tokens == 0 || deferred_ids.is_empty() {
        return 0;
    }
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_selective_rehydrate_deferred_read_plan_in_text(
                text,
                store,
                terms,
                deferred_ids,
                remaining_tokens,
                stats,
            )
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_selective_rehydrate_deferred_read_plan(
                    item,
                    store,
                    terms,
                    deferred_ids,
                    remaining_tokens,
                    stats,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(
                    runtime_smart_context_selective_rehydrate_deferred_read_plan(
                        item,
                        store,
                        terms,
                        deferred_ids,
                        remaining_tokens,
                        stats,
                    ),
                );
            }
            count
        }
        _ => 0,
    }
}

fn runtime_smart_context_selective_rehydrate_deferred_read_plan_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    deferred_ids: &BTreeSet<String>,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if *remaining_tokens == 0 {
        return 0;
    }
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .filter(|reference| reference.line_ranges.is_empty() && reference.line_range.is_none())
        .map(|reference| reference.id)
        .filter(|id| deferred_ids.contains(id))
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some((appendix, range_count, token_cost)) =
            runtime_smart_context_deferred_read_plan_appendix(
                &id,
                store,
                terms,
                &next,
                *remaining_tokens,
            )
        else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
        runtime_smart_context_consume_rehydrate_budget(remaining_tokens, token_cost);
        if *remaining_tokens == 0 {
            break;
        }
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

fn runtime_smart_context_deferred_read_plan_appendix(
    artifact_id: &str,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    current_text: &str,
    token_budget: usize,
) -> Option<(String, usize, usize)> {
    let line_index = store.line_index(artifact_id)?;
    let artifact_text = store.get_text(artifact_id);
    let mut ranges = Vec::<RuntimeSmartContextScoredExactAppendixRange>::new();

    for range in line_index
        .symbol_ranges
        .iter()
        .chain(line_index.test_failure_ranges.iter())
        .chain(line_index.error_ranges.iter())
        .chain(line_index.file_location_ranges.iter())
        .chain(line_index.diff_hunk_ranges.iter())
    {
        if !runtime_smart_context_artifact_semantic_range_valid(range)
            || !runtime_smart_context_semantic_range_matches_terms(range, terms)
        {
            continue;
        }
        runtime_smart_context_push_scored_exact_range(
            &mut ranges,
            current_text,
            RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            },
            runtime_smart_context_semantic_range_score_with_command(
                range,
                terms,
                line_index.command_kind.as_deref(),
            )
            .saturating_add(200),
        );
    }

    for range in &line_index.critical_ranges {
        if !runtime_smart_context_artifact_line_index_range_valid(range) {
            continue;
        }
        runtime_smart_context_push_scored_exact_range(
            &mut ranges,
            current_text,
            RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            },
            runtime_smart_context_critical_exact_appendix_score(
                &RuntimeSmartContextExactAppendixRange {
                    reference: String::new(),
                    body: range.text.clone(),
                },
            )
            .saturating_add(100),
        );
    }

    if let Some(artifact_text) = artifact_text.as_deref() {
        ranges.extend(runtime_smart_context_import_read_plan_ranges(
            artifact_id,
            artifact_text,
            current_text,
            terms,
        ));
    }

    if ranges.is_empty() {
        return None;
    }
    let exact_ranges = ranges
        .iter()
        .map(|range| range.range.clone())
        .collect::<Vec<_>>();
    runtime_smart_context_render_budgeted_scored_exact_appendix(
        SMART_CONTEXT_LABEL_REHYDRATE_PLAN_EXACT,
        exact_ranges,
        SMART_CONTEXT_BUDGET_AWARE_REHYDRATE_MAX_RANGES,
        token_budget,
        |range| {
            ranges
                .iter()
                .find(|candidate| candidate.range.eq(range))
                .map_or(0, |candidate| candidate.score)
        },
    )
}

fn runtime_smart_context_push_scored_exact_range(
    ranges: &mut Vec<RuntimeSmartContextScoredExactAppendixRange>,
    current_text: &str,
    range: RuntimeSmartContextExactAppendixRange,
    score: usize,
) {
    if range.body.trim().is_empty()
        || current_text.contains(&range.reference)
        || current_text.contains(&range.body)
        || ranges.iter().any(|candidate| candidate.range == range)
    {
        return;
    }
    ranges.push(RuntimeSmartContextScoredExactAppendixRange { range, score });
}

fn runtime_smart_context_import_read_plan_ranges(
    artifact_id: &str,
    artifact_text: &str,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> Vec<RuntimeSmartContextScoredExactAppendixRange> {
    let lines = artifact_text.lines().collect::<Vec<_>>();
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(SMART_CONTEXT_BUDGET_AWARE_IMPORT_SCAN_MAX_LINES)
    {
        let line_number = index + 1;
        if runtime_smart_context_line_is_import(line) {
            start.get_or_insert(line_number);
            end = line_number;
            continue;
        }
        if let Some(range_start) = start.take() {
            runtime_smart_context_push_import_read_plan_range(
                artifact_id,
                &lines,
                range_start,
                end,
                current_text,
                terms,
                &mut ranges,
            );
            if ranges.len() >= SMART_CONTEXT_BUDGET_AWARE_IMPORT_MAX_RANGES {
                return ranges;
            }
        }
        if !line.trim().is_empty()
            && !line.trim_start().starts_with("//")
            && !line.trim_start().starts_with('#')
        {
            continue;
        }
    }
    if let Some(range_start) = start {
        runtime_smart_context_push_import_read_plan_range(
            artifact_id,
            &lines,
            range_start,
            end,
            current_text,
            terms,
            &mut ranges,
        );
    }
    ranges
}

fn runtime_smart_context_push_import_read_plan_range(
    artifact_id: &str,
    lines: &[&str],
    start: usize,
    end: usize,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    ranges: &mut Vec<RuntimeSmartContextScoredExactAppendixRange>,
) {
    if ranges.len() >= SMART_CONTEXT_BUDGET_AWARE_IMPORT_MAX_RANGES {
        return;
    }
    let Some(body) = runtime_smart_context_line_excerpt(lines, start, end) else {
        return;
    };
    let mut score = 40usize;
    for term in terms
        .file_paths
        .iter()
        .chain(terms.test_symbols.iter())
        .chain(terms.error_codes.iter())
    {
        if body.contains(term) {
            score = score.saturating_add(40);
        }
    }
    runtime_smart_context_push_scored_exact_range(
        ranges,
        current_text,
        RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, start, end),
            body,
        },
        score,
    );
}

fn runtime_smart_context_line_is_import(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use ")
        || trimmed.starts_with("pub use ")
        || trimmed.starts_with("extern crate ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ") && trimmed.contains(" import ")
}

fn runtime_smart_context_matching_semantic_range_appendix_with_budget(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    token_budget: usize,
) -> Option<(String, usize, usize)> {
    let ranges = runtime_smart_context_matching_semantic_ranges(
        artifact_id,
        line_index,
        current_text,
        terms,
    );
    if ranges.is_empty() {
        return None;
    }

    let exact_ranges = ranges
        .iter()
        .map(|range| RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, range.start, range.end),
            body: range.text.clone(),
        })
        .collect::<Vec<_>>();
    let scored_ranges = ranges
        .iter()
        .zip(exact_ranges.iter())
        .map(
            |(range, exact)| RuntimeSmartContextScoredExactAppendixRange {
                range: exact.clone(),
                score: runtime_smart_context_semantic_range_score_with_command(
                    range,
                    terms,
                    line_index.command_kind.as_deref(),
                ),
            },
        )
        .collect::<Vec<_>>();

    runtime_smart_context_render_budgeted_scored_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        exact_ranges,
        runtime_smart_context_semantic_rehydrate_range_cap(terms),
        token_budget,
        |range| {
            scored_ranges
                .iter()
                .find(|candidate| candidate.range.eq(range))
                .map_or(0, |candidate| candidate.score)
        },
    )
}

fn runtime_smart_context_matching_semantic_ranges<'a>(
    artifact_id: &str,
    line_index: &'a RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> Vec<&'a RuntimeSmartContextArtifactSemanticLineRange> {
    let mut ranges = Vec::new();
    let mut seen = BTreeSet::<(usize, usize, String)>::new();
    for range in line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .chain(line_index.test_failure_ranges.iter())
        .chain(line_index.error_ranges.iter())
        .chain(line_index.symbol_ranges.iter())
    {
        if !runtime_smart_context_artifact_semantic_range_valid(range)
            || !runtime_smart_context_semantic_range_matches_terms(range, terms)
            || current_text.contains(&runtime_smart_context_artifact_line_ref(
                artifact_id,
                range.start,
                range.end,
            ))
            || current_text.contains(&format!(
                "prodex-artifact:{artifact_id}#L{}-L{}",
                range.start, range.end
            ))
            || current_text.contains(&range.text)
        {
            continue;
        }
        if seen.insert((range.start, range.end, range.content_hash.clone())) {
            ranges.push(range);
        }
    }
    ranges.sort_by_key(|range| {
        (
            std::cmp::Reverse(runtime_smart_context_semantic_range_score_with_command(
                range,
                terms,
                line_index.command_kind.as_deref(),
            )),
            range.start,
            range.end,
        )
    });
    ranges
}

fn runtime_smart_context_semantic_range_score_with_command(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    command_kind: Option<&str>,
) -> usize {
    let mut score = prodex_context::count_critical_signals(&range.text)
        .total()
        .saturating_mul(20);
    if range
        .code
        .as_deref()
        .is_some_and(|code| runtime_smart_context_error_code_matches_terms(code, terms))
    {
        score = score.saturating_add(120);
    }
    if range
        .symbol
        .as_deref()
        .is_some_and(|symbol| runtime_smart_context_symbol_matches_terms(symbol, terms))
    {
        score = score.saturating_add(110);
    }
    if range
        .path
        .as_deref()
        .is_some_and(|path| runtime_smart_context_path_matches_terms(path, terms))
    {
        score = score.saturating_add(90);
    }
    if command_kind.is_some_and(|kind| terms.command_kinds.contains(kind)) {
        score = score.saturating_add(match range.label.as_deref() {
            Some("test_failure" | "error") => 70,
            Some("diff_hunk") => 50,
            Some("file_location") => 30,
            _ => 20,
        });
    }
    if terms
        .diff_hunks
        .iter()
        .any(|term| runtime_smart_context_diff_hunk_range_matches_term(range, term))
    {
        score = score.saturating_add(80);
    }
    score.saturating_add(10_000usize.saturating_sub(range.byte_len.min(10_000)) / 100)
}

fn runtime_smart_context_symbol_matches_terms(
    symbol: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .test_symbols
        .iter()
        .any(|term| runtime_smart_context_symbol_matches_term(symbol, term))
}

fn runtime_smart_context_symbol_matches_term(symbol: &str, term: &str) -> bool {
    let symbol = symbol.trim_end_matches("()");
    let term = term.trim_end_matches("()");
    symbol == term
        || term
            .rsplit("::")
            .next()
            .is_some_and(|suffix| suffix == symbol)
        || symbol
            .rsplit("::")
            .next()
            .is_some_and(|suffix| suffix == term)
        || term
            .rsplit('#')
            .next()
            .is_some_and(|suffix| suffix == symbol)
        || symbol
            .rsplit('#')
            .next()
            .is_some_and(|suffix| suffix == term)
        || term
            .rsplit('.')
            .next()
            .is_some_and(|suffix| suffix == symbol)
        || symbol
            .rsplit('.')
            .next()
            .is_some_and(|suffix| suffix == term)
}

fn runtime_smart_context_path_matches_terms(
    path: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .file_paths
        .iter()
        .any(|term| runtime_smart_context_path_matches_term(path, term))
}

fn runtime_smart_context_path_matches_term(path: &str, term: &str) -> bool {
    let Some(path) = runtime_smart_context_normalized_intent_path(path) else {
        return false;
    };
    let Some(term) = runtime_smart_context_normalized_intent_path(term) else {
        return false;
    };
    path == term
        || path.ends_with(&format!("/{term}"))
        || term.ends_with(&format!("/{path}"))
        || !term.contains('/') && path.rsplit('/').next() == Some(term.as_str())
}

fn runtime_smart_context_normalized_intent_path(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    let path = normalized
        .as_str()
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '`' | '/' | ':' | ',' | ';'))
        .to_string();
    (!path.is_empty()).then_some(path)
}

fn runtime_smart_context_error_code_matches_terms(
    code: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .error_codes
        .iter()
        .any(|term| code.eq_ignore_ascii_case(term))
}

fn runtime_smart_context_semantic_rehydrate_range_cap(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> usize {
    if runtime_smart_context_selective_rehydrate_terms_narrow(terms) {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES
    } else {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES
    }
}

fn runtime_smart_context_selective_rehydrate_terms_narrow(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    let term_count = terms
        .file_paths
        .len()
        .saturating_add(terms.error_codes.len())
        .saturating_add(terms.test_symbols.len())
        .saturating_add(terms.diff_hunks.len());
    term_count == 1
}

fn runtime_smart_context_selective_rehydrate_terms_empty(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms.file_paths.is_empty()
        && terms.error_codes.is_empty()
        && terms.test_symbols.is_empty()
        && terms.command_kinds.is_empty()
        && terms.diff_hunks.is_empty()
}

fn runtime_smart_context_selective_rehydrate_terms_strong(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    !terms.file_paths.is_empty()
        || !terms.error_codes.is_empty()
        || !terms.test_symbols.is_empty()
        || !terms.diff_hunks.is_empty()
}

fn runtime_smart_context_semantic_range_matches_terms(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    range
        .path
        .as_deref()
        .is_some_and(|path| runtime_smart_context_path_matches_terms(path, terms))
        || range
            .code
            .as_deref()
            .is_some_and(|code| runtime_smart_context_error_code_matches_terms(code, terms))
        || range
            .symbol
            .as_deref()
            .is_some_and(|symbol| runtime_smart_context_symbol_matches_terms(symbol, terms))
        || terms
            .diff_hunks
            .iter()
            .any(|term| runtime_smart_context_diff_hunk_range_matches_term(range, term))
}

fn runtime_smart_context_diff_hunk_range_matches_term(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    term: &RuntimeSmartContextSelectiveDiffHunkTerm,
) -> bool {
    range.label.as_deref() == Some("diff_hunk")
        && term
            .path
            .as_deref()
            .map(|path| range.path.as_deref() == Some(path))
            .unwrap_or(true)
        && term
            .old_start
            .map(|old_start| range.old_start == Some(old_start))
            .unwrap_or(true)
        && term
            .new_start
            .map(|new_start| range.new_start == Some(new_start))
            .unwrap_or(true)
}

fn runtime_smart_context_artifact_semantic_range_valid(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}

fn runtime_smart_context_artifact_marker_line(
    kind: &str,
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let kind = match kind {
        "artifact" => "art",
        other => other,
    };
    format!(
        "psc {kind} {reference} b={} lines=#Lx-Ly",
        artifact.byte_len
    )
}

fn runtime_smart_context_artifact_line_ref(id: &str, start: usize, end: usize) -> String {
    format!("{}#L{start}-L{end}", runtime_smart_context_artifact_ref(id))
}

fn runtime_smart_context_artifact_ref(id: &str) -> String {
    let short_id = id.strip_prefix("sc:").unwrap_or(id);
    format!("{SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX}{short_id}")
}

#[cfg(test)]
fn runtime_smart_context_apply_artifact_aliases_to_generated_texts(
    value: &mut serde_json::Value,
) -> bool {
    let counts = runtime_smart_context_generated_artifact_ref_counts(value);
    let aliases = runtime_smart_context_artifact_alias_plan(counts, None);
    if aliases.is_empty() {
        return false;
    }
    let replacement_count =
        runtime_smart_context_replace_generated_artifact_refs_with_aliases(value, &aliases);
    if replacement_count == 0 {
        return false;
    }
    runtime_smart_context_insert_artifact_alias_legend(value, &aliases)
}

fn runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
) -> bool {
    let counts = runtime_smart_context_generated_artifact_ref_counts(value);
    let previous_aliases = state.artifact_aliases.clone();
    let previous_next_index = state.next_artifact_alias_index;
    let aliases = runtime_smart_context_artifact_alias_plan(counts, Some(state));
    if aliases.is_empty() {
        return false;
    }
    let replacement_count =
        runtime_smart_context_replace_generated_artifact_refs_with_aliases(value, &aliases);
    if replacement_count == 0 {
        state.artifact_aliases = previous_aliases;
        state.next_artifact_alias_index = previous_next_index;
        return false;
    }
    if !runtime_smart_context_insert_artifact_alias_legend(value, &aliases) {
        state.artifact_aliases = previous_aliases;
        state.next_artifact_alias_index = previous_next_index;
        return false;
    }
    true
}

fn runtime_smart_context_generated_artifact_ref_counts(
    value: &serde_json::Value,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    runtime_smart_context_generated_artifact_ref_counts_from_value(value, &mut counts);
    counts
}

fn runtime_smart_context_generated_artifact_ref_counts_from_value(
    value: &serde_json::Value,
    counts: &mut BTreeMap<String, usize>,
) {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let aliases = BTreeMap::new();
            let ids = runtime_smart_context_artifact_ref_occurrences_from_text(text, &aliases)
                .into_iter()
                .map(|reference| reference.id)
                .collect::<BTreeSet<_>>();
            for id in ids {
                let reference = runtime_smart_context_artifact_ref(&id);
                let count = text.matches(&reference).count();
                if count > 0 {
                    *counts.entry(id).or_default() += count;
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_generated_artifact_ref_counts_from_value(item, counts);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_generated_artifact_ref_counts_from_value(item, counts);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_artifact_alias_plan(
    counts: BTreeMap<String, usize>,
    mut state: Option<&mut RuntimeSmartContextProxyState>,
) -> Vec<RuntimeSmartContextArtifactAlias> {
    let mut candidates = counts
        .into_iter()
        .filter_map(|(id, count)| {
            (count > 1).then(|| {
                let reference = runtime_smart_context_artifact_ref(&id);
                let potential = reference.len().saturating_mul(count);
                (id, count, reference, potential)
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.3.cmp(&left.3).then_with(|| left.0.cmp(&right.0)));

    let mut aliases = Vec::new();
    for (id, count, reference, _) in candidates {
        let alias = if let Some(state) = state.as_deref_mut() {
            runtime_smart_context_stable_artifact_alias_candidate(state, &id, &aliases)
        } else {
            format!("@{}", aliases.len())
        };
        if reference.len() <= alias.len() {
            continue;
        }
        let replacement_savings = reference
            .len()
            .saturating_sub(alias.len())
            .saturating_mul(count);
        let definition_len = alias
            .len()
            .saturating_add(1)
            .saturating_add(reference.len());
        let legend_cost = if aliases.is_empty() {
            SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX
                .len()
                .saturating_add(definition_len)
                .saturating_add(1)
        } else {
            definition_len.saturating_add(1)
        };
        if replacement_savings <= legend_cost {
            continue;
        }
        if let Some(state) = state.as_deref_mut() {
            runtime_smart_context_commit_stable_artifact_alias(state, &id, &alias);
        }
        aliases.push(RuntimeSmartContextArtifactAlias { id, alias });
    }
    aliases
}

fn runtime_smart_context_stable_artifact_alias_candidate(
    state: &mut RuntimeSmartContextProxyState,
    id: &str,
    planned_aliases: &[RuntimeSmartContextArtifactAlias],
) -> String {
    if let Some(alias) = state.artifact_aliases.get(id) {
        return alias.clone();
    }
    let mut index = state.next_artifact_alias_index;
    loop {
        let alias = format!("@{index}");
        index = index.saturating_add(1);
        if state
            .artifact_aliases
            .values()
            .all(|existing| existing != &alias)
            && planned_aliases.iter().all(|planned| planned.alias != alias)
        {
            return alias;
        }
    }
}

fn runtime_smart_context_commit_stable_artifact_alias(
    state: &mut RuntimeSmartContextProxyState,
    id: &str,
    alias: &str,
) {
    if state
        .artifact_aliases
        .get(id)
        .is_some_and(|existing| existing == alias)
    {
        return;
    }
    state
        .artifact_aliases
        .insert(id.to_string(), alias.to_string());
    if let Some(index) = runtime_smart_context_artifact_alias_index(alias) {
        state.next_artifact_alias_index = state.next_artifact_alias_index.max(index + 1);
    }
}

fn runtime_smart_context_artifact_alias_index(alias: &str) -> Option<usize> {
    alias.strip_prefix('@')?.parse::<usize>().ok()
}

fn runtime_smart_context_persisted_artifact_aliases(
    state: &RuntimeSmartContextProxyState,
) -> Vec<RuntimeSmartContextPersistedArtifactAlias> {
    let mut aliases = state
        .artifact_aliases
        .iter()
        .filter(|(id, alias)| {
            runtime_smart_context_artifact_id_valid(id)
                && runtime_smart_context_artifact_alias_valid(alias)
        })
        .map(|(id, alias)| RuntimeSmartContextPersistedArtifactAlias {
            id: id.clone(),
            alias: alias.clone(),
        })
        .collect::<Vec<_>>();
    aliases.sort_by_key(|entry| {
        (
            runtime_smart_context_artifact_alias_index(&entry.alias).unwrap_or(usize::MAX),
            entry.id.clone(),
        )
    });
    aliases.truncate(SMART_CONTEXT_PERSISTED_ARTIFACT_ALIAS_LIMIT);
    aliases
}

fn runtime_smart_context_artifact_alias_state_from_persisted(
    persisted: Vec<RuntimeSmartContextPersistedArtifactAlias>,
) -> (BTreeMap<String, String>, usize) {
    let aliases = runtime_smart_context_merge_persisted_artifact_aliases(Vec::new(), persisted);
    let mut map = BTreeMap::new();
    let mut next_index = 0usize;
    for entry in aliases {
        if let Some(index) = runtime_smart_context_artifact_alias_index(&entry.alias) {
            next_index = next_index.max(index + 1);
        }
        map.insert(entry.id, entry.alias);
    }
    (map, next_index)
}

fn runtime_smart_context_merge_persisted_artifact_aliases(
    existing: Vec<RuntimeSmartContextPersistedArtifactAlias>,
    incoming: Vec<RuntimeSmartContextPersistedArtifactAlias>,
) -> Vec<RuntimeSmartContextPersistedArtifactAlias> {
    let mut by_id = BTreeMap::<String, String>::new();
    let mut alias_owner = BTreeMap::<String, String>::new();
    for entry in existing.into_iter().chain(incoming) {
        if !runtime_smart_context_artifact_id_valid(&entry.id)
            || !runtime_smart_context_artifact_alias_valid(&entry.alias)
        {
            continue;
        }
        if let Some(owner) = alias_owner.get(&entry.alias)
            && owner != &entry.id
        {
            continue;
        }
        if let Some(previous_alias) = by_id.insert(entry.id.clone(), entry.alias.clone()) {
            alias_owner.remove(&previous_alias);
        }
        alias_owner.insert(entry.alias, entry.id);
    }
    let mut aliases = by_id
        .into_iter()
        .map(|(id, alias)| RuntimeSmartContextPersistedArtifactAlias { id, alias })
        .collect::<Vec<_>>();
    aliases.sort_by_key(|entry| {
        (
            runtime_smart_context_artifact_alias_index(&entry.alias).unwrap_or(usize::MAX),
            entry.id.clone(),
        )
    });
    aliases.truncate(SMART_CONTEXT_PERSISTED_ARTIFACT_ALIAS_LIMIT);
    aliases
}

fn runtime_smart_context_artifact_id_valid(id: &str) -> bool {
    id.strip_prefix("sc:")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn runtime_smart_context_replace_generated_artifact_refs_with_aliases(
    value: &mut serde_json::Value,
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> usize {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let mut next_lines = Vec::new();
            let mut replacements = 0usize;
            for line in text.lines() {
                if runtime_smart_context_generated_control_line(line) {
                    next_lines.push(line.to_string());
                    continue;
                }
                let mut next_line = line.to_string();
                for alias in aliases {
                    let reference = runtime_smart_context_artifact_ref(&alias.id);
                    let count = next_line.matches(&reference).count();
                    if count == 0 {
                        continue;
                    }
                    next_line = next_line.replace(&reference, &alias.alias);
                    replacements = replacements.saturating_add(count);
                }
                next_lines.push(next_line);
            }
            if replacements > 0 {
                *text = next_lines.join("\n");
            }
            replacements
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_replace_generated_artifact_refs_with_aliases(item, aliases)
            })
            .sum(),
        serde_json::Value::Object(object) => object
            .values_mut()
            .map(|item| {
                runtime_smart_context_replace_generated_artifact_refs_with_aliases(item, aliases)
            })
            .sum(),
        _ => 0,
    }
}

fn runtime_smart_context_generated_control_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY)
        || trimmed.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY)
        || trimmed.starts_with("psc art ")
        || trimmed.starts_with("psc rep ")
        || trimmed.starts_with("psc repeat ")
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX_LEGACY)
        || trimmed.starts_with("prodex-sc artifact ")
        || trimmed.starts_with("prodex-sc repeat ")
}

fn runtime_smart_context_insert_artifact_alias_legend(
    value: &mut serde_json::Value,
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> bool {
    let legend = runtime_smart_context_artifact_alias_legend(aliases);
    runtime_smart_context_insert_artifact_alias_legend_in_value(value, &legend)
}

fn runtime_smart_context_artifact_alias_legend(
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> String {
    let defs = aliases
        .iter()
        .map(|alias| {
            format!(
                "{}={}",
                alias.alias,
                runtime_smart_context_artifact_ref(&alias.id)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX}{defs}")
}

fn runtime_smart_context_insert_artifact_alias_legend_in_value(
    value: &mut serde_json::Value,
    legend: &str,
) -> bool {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            *text = runtime_smart_context_insert_artifact_alias_legend_in_text(text, legend);
            true
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(|item| runtime_smart_context_insert_artifact_alias_legend_in_value(item, legend)),
        serde_json::Value::Object(object) => object
            .values_mut()
            .any(|item| runtime_smart_context_insert_artifact_alias_legend_in_value(item, legend)),
        _ => false,
    }
}

fn runtime_smart_context_insert_artifact_alias_legend_in_text(text: &str, legend: &str) -> String {
    let duplicate_legend = if legend.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX) {
        text.contains(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX)
            || text.contains(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY)
    } else if legend.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX) {
        text.contains(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX)
            || text.contains(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY)
    } else {
        text.contains(legend)
    };
    if duplicate_legend {
        return text.to_string();
    }
    if let Some((first, rest)) = text.split_once('\n') {
        format!("{first}\n{legend}\n{rest}")
    } else {
        format!("{text}\n{legend}")
    }
}

fn runtime_smart_context_apply_path_aliases_to_generated_texts(
    value: &mut serde_json::Value,
) -> bool {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let Some(next) = runtime_smart_context_path_aliased_generated_text(text) else {
                return false;
            };
            *text = next;
            true
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(runtime_smart_context_apply_path_aliases_to_generated_texts),
        serde_json::Value::Object(object) => object
            .values_mut()
            .any(runtime_smart_context_apply_path_aliases_to_generated_texts),
        _ => false,
    }
}

fn runtime_smart_context_path_aliased_generated_text(text: &str) -> Option<String> {
    let aliases = runtime_smart_context_path_aliases(text);
    if aliases.is_empty() {
        return None;
    }
    let mut candidate = text.to_string();
    for (alias, prefix) in &aliases {
        candidate = candidate.replace(prefix, alias);
    }
    if candidate == text || candidate.len() >= text.len() {
        return None;
    }
    let legend = aliases
        .iter()
        .map(|(alias, prefix)| format!("{alias}={prefix}"))
        .collect::<Vec<_>>()
        .join(" ");
    candidate = runtime_smart_context_insert_artifact_alias_legend_in_text(
        &candidate,
        &format!("{SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX}{legend}"),
    );
    if candidate.len() < text.len()
        && prodex_context::critical_signal_self_check(text, &candidate).passed()
    {
        Some(candidate)
    } else {
        None
    }
}

fn runtime_smart_context_path_aliases(text: &str) -> Vec<(String, String)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() && !matches!(ch, '/' | '_' | '-' | '.')
        });
        if let Some(prefix) = runtime_smart_context_repeated_path_prefix(token) {
            *counts.entry(prefix).or_default() += 1;
        }
    }
    let mut candidates = counts
        .into_iter()
        .filter(|(prefix, count)| *count >= 2 && prefix.len() > 8)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .len()
            .saturating_mul(right.1)
            .cmp(&left.0.len().saturating_mul(left.1))
    });
    candidates
        .into_iter()
        .take(4)
        .enumerate()
        .filter_map(|(index, (prefix, count))| {
            let alias = if index == 0 {
                "$R".to_string()
            } else {
                format!("$P{index}")
            };
            let saved = prefix
                .len()
                .saturating_sub(alias.len())
                .saturating_mul(count);
            let cost = alias.len().saturating_add(prefix.len()).saturating_add(2);
            (saved > cost).then_some((alias, prefix))
        })
        .collect()
}

fn runtime_smart_context_repeated_path_prefix(token: &str) -> Option<String> {
    if !token.starts_with('/') {
        return None;
    }
    for marker in [
        "/crates/",
        "/src/",
        "/tests/",
        "/test/",
        "/dist/",
        "/target/",
        "/apps/",
        "/packages/",
    ] {
        if let Some(index) = token.find(marker) {
            let prefix = &token[..index];
            if prefix.matches('/').count() >= 2 {
                return Some(prefix.to_string());
            }
        }
    }
    token
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .filter(|prefix| prefix.matches('/').count() >= 3)
        .map(str::to_string)
}

fn runtime_smart_context_text_is_generated_summary_or_manifest(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("prodex-sc artifact ")
        || trimmed.starts_with("prodex-sc repeat ")
        || trimmed.starts_with("psc art ")
        || trimmed.starts_with("psc rep ")
        || trimmed.starts_with("psc repeat ")
        || trimmed.starts_with("psc co ")
        || trimmed.starts_with("psc cmdout ")
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX_LEGACY)
        || trimmed.starts_with("psc m ")
        || trimmed.starts_with("psc manifest ")
}

fn runtime_smart_context_append_artifact_manifest_delta_if_useful(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
    stats: &RuntimeSmartContextTransformStats,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    if !runtime_smart_context_artifact_manifest_useful(stats) {
        return false;
    }
    if !runtime_smart_context_artifact_manifest_delta_eligible(state, intent_signals) {
        return false;
    }
    if runtime_smart_context_explicit_artifact_refs_fully_resolved_without_manifest_request(
        state,
        intent_signals,
    ) {
        return false;
    }
    let mut entries = state
        .artifacts
        .artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_filter_manifest_entries(value, intent_signals, &mut entries);
    let ids = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    if ids.is_empty() || ids == state.last_artifact_manifest_ids {
        return false;
    }
    let detailed_manifest = runtime_smart_context_manifest_requested(intent_signals);
    let unchanged_count = ids.intersection(&state.last_artifact_manifest_ids).count();
    let manifest_entries = if detailed_manifest {
        entries
    } else {
        entries
            .into_iter()
            .filter(|entry| !state.last_artifact_manifest_ids.contains(&entry.id))
            .collect::<Vec<_>>()
    };
    let Some(manifest) = runtime_smart_context_artifact_manifest_delta_from_entries(
        manifest_entries,
        detailed_manifest,
        unchanged_count,
    ) else {
        return false;
    };
    if !runtime_smart_context_append_input_manifest(value, manifest) {
        return false;
    }
    state.last_artifact_manifest_ids = ids;
    state.last_artifact_manifest_emitted_at = Some(Instant::now());
    true
}

fn runtime_smart_context_explicit_artifact_refs_fully_resolved_without_manifest_request(
    state: &RuntimeSmartContextProxyState,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    !runtime_smart_context_manifest_requested(intent_signals)
        && !intent_signals.artifact_refs.is_empty()
        && intent_signals
            .artifact_refs
            .iter()
            .all(|reference| state.artifacts.contains(&reference.id))
}

fn runtime_smart_context_artifact_manifest_delta_eligible(
    state: &RuntimeSmartContextProxyState,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    if runtime_smart_context_manifest_requested(intent_signals) {
        return true;
    }
    if intent_signals.artifact_refs.is_empty()
        && runtime_smart_context_selective_rehydrate_terms_empty(&intent_signals.semantic_terms)
        && intent_signals.command_kind_hints.is_empty()
    {
        return false;
    }
    state
        .last_artifact_manifest_emitted_at
        .is_none_or(|emitted_at| {
            emitted_at.elapsed()
                >= Duration::from_millis(SMART_CONTEXT_ARTIFACT_MANIFEST_COOLDOWN_MS)
        })
}

#[cfg(test)]
fn runtime_smart_context_append_artifact_manifest_if_useful(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    if !runtime_smart_context_artifact_manifest_useful(stats) {
        return false;
    }
    let Some(manifest) =
        runtime_smart_context_artifact_manifest_for_value(value, store, &Default::default())
    else {
        return false;
    };
    runtime_smart_context_append_input_manifest(value, manifest)
}

fn runtime_smart_context_artifact_manifest_useful(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.artifacts_stored > 0
        || stats.tool_outputs_condensed > 0
        || stats.cross_turn_duplicate_texts > 0
        || stats.repeat_tool_output_refs > 0
}

#[cfg(test)]
fn runtime_smart_context_artifact_manifest(
    store: &RuntimeSmartContextArtifactStore,
) -> Option<String> {
    let entries = store.artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_artifact_manifest_from_entries(entries, true)
}

#[cfg(test)]
fn runtime_smart_context_artifact_manifest_for_value(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> Option<String> {
    let mut entries = store.artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_filter_manifest_entries(value, intent_signals, &mut entries);
    runtime_smart_context_artifact_manifest_from_entries(
        entries,
        runtime_smart_context_manifest_requested(intent_signals),
    )
}

fn runtime_smart_context_filter_manifest_entries(
    value: &serde_json::Value,
    intent_signals: &RuntimeSmartContextIntentSignals,
    entries: &mut Vec<RuntimeSmartContextArtifactManifestEntry>,
) {
    if !runtime_smart_context_manifest_requested(intent_signals) {
        let visible_ids = runtime_smart_context_collect_artifact_refs(value)
            .into_iter()
            .map(|reference| reference.id)
            .collect::<BTreeSet<_>>();
        entries.retain(|entry| !visible_ids.contains(&entry.id));
    }
    runtime_smart_context_sort_manifest_entries_for_intent(entries, intent_signals);
}

fn runtime_smart_context_manifest_requested(
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    intent_signals.intent_terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "artifact"
                | "artifacts"
                | "manifest"
                | "reference"
                | "references"
                | "refs"
                | "rehydrate"
        )
    })
}

fn runtime_smart_context_sort_manifest_entries_for_intent(
    entries: &mut Vec<RuntimeSmartContextArtifactManifestEntry>,
    intent_signals: &RuntimeSmartContextIntentSignals,
) {
    if runtime_smart_context_selective_rehydrate_terms_empty(&intent_signals.semantic_terms) {
        return;
    }
    let original = std::mem::take(entries);
    let (mut matching, rest): (Vec<_>, Vec<_>) = original.into_iter().partition(|entry| {
        runtime_smart_context_manifest_entry_matches_intent(entry, intent_signals)
    });
    matching.extend(rest);
    *entries = matching;
}

fn runtime_smart_context_manifest_entry_matches_intent(
    entry: &RuntimeSmartContextArtifactManifestEntry,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    entry.file_location_range_count > 0 && !intent_signals.semantic_terms.file_paths.is_empty()
        || entry.error_range_count > 0 && !intent_signals.semantic_terms.error_codes.is_empty()
        || entry.test_failure_range_count > 0
            && !intent_signals.semantic_terms.test_symbols.is_empty()
        || entry.diff_hunk_range_count > 0 && !intent_signals.semantic_terms.diff_hunks.is_empty()
}

#[cfg(test)]
fn runtime_smart_context_artifact_manifest_from_entries(
    entries: Vec<RuntimeSmartContextArtifactManifestEntry>,
    detailed: bool,
) -> Option<String> {
    runtime_smart_context_artifact_manifest_delta_from_entries(entries, detailed, 0)
}

fn runtime_smart_context_artifact_manifest_delta_from_entries(
    entries: Vec<RuntimeSmartContextArtifactManifestEntry>,
    detailed: bool,
    unchanged_count: usize,
) -> Option<String> {
    if entries.is_empty() {
        if unchanged_count == 0 {
            return None;
        }
        return Some(format!("psc m refs same={unchanged_count}"));
    }

    let mut header = "psc m refs".to_string();
    if unchanged_count > 0 {
        header.push_str(&format!(" same={unchanged_count}"));
    }
    let mut lines = vec![header];
    let mut rendered_len = lines[0].len();
    for entry in entries {
        let semantic_count = entry
            .file_location_range_count
            .saturating_add(entry.diff_hunk_range_count)
            .saturating_add(entry.test_failure_range_count)
            .saturating_add(entry.error_range_count);
        let reference = runtime_smart_context_artifact_ref(&entry.id);
        let mut parts = vec![format!("- {reference}"), format!("b={}", entry.byte_len)];
        if detailed && entry.critical_range_count > 0 {
            parts.push(format!("cr={}", entry.critical_range_count));
        }
        if detailed && semantic_count > 0 {
            parts.push(format!("sr={semantic_count}"));
        }
        if detailed && entry.file_location_range_count > 0 {
            parts.push(format!("f={}", entry.file_location_range_count));
        }
        if detailed && entry.diff_hunk_range_count > 0 {
            parts.push(format!("d={}", entry.diff_hunk_range_count));
        }
        if detailed && entry.test_failure_range_count > 0 {
            parts.push(format!("t={}", entry.test_failure_range_count));
        }
        if detailed && entry.error_range_count > 0 {
            parts.push(format!("e={}", entry.error_range_count));
        }
        if detailed && let Some(kind) = entry.command_kind.as_deref() {
            parts.push(format!("k={kind}"));
        }
        let line = parts.join(" ");
        if rendered_len.saturating_add(line.len()).saturating_add(1)
            > SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_CHARS
        {
            lines.push("[psc m trunc]".to_string());
            break;
        }
        rendered_len = rendered_len.saturating_add(line.len()).saturating_add(1);
        lines.push(line);
    }
    Some(lines.join("\n"))
}

fn runtime_smart_context_append_input_manifest(
    value: &mut serde_json::Value,
    manifest: String,
) -> bool {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    input.push(serde_json::json!({
        "type": "message",
        "role": "user",
        "content": manifest,
    }));
    true
}

fn runtime_smart_context_dedupe_input_text(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let mut seen = BTreeMap::<String, usize>::new();
    for (index, item) in input.iter_mut().enumerate() {
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        runtime_smart_context_dedupe_value_text(item, index, &mut seen, stats);
    }
    runtime_smart_context_replace_cross_turn_duplicate_refs(value, store, exactness_guard, stats);
}

fn runtime_smart_context_dedupe_value_text(
    value: &mut serde_json::Value,
    item_index: usize,
    seen: &mut BTreeMap<String, usize>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    match value {
        serde_json::Value::String(text) => {
            if text.len() < SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES {
                return;
            }
            let hash = runtime_proxy_crate::smart_context_hash_text(text);
            if let Some(first_index) = seen.get(&hash) {
                *text = format!("[psc dup input[{first_index}]]");
                stats.duplicate_texts += 1;
            } else {
                seen.insert(hash.clone(), item_index);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values_mut() {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_replace_cross_turn_duplicate_refs(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let items = runtime_smart_context_collect_large_rehydratable_text_items(value);
    if items.is_empty() {
        return;
    }
    let artifacts = runtime_smart_context_available_artifacts_for_text_items(items.iter(), store);
    let plan = runtime_proxy_crate::smart_context_cross_turn_duplicate_ref_plan(
        items,
        artifacts,
        SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES,
        exactness_guard,
    );
    let replacements = plan
        .actions
        .into_iter()
        .filter_map(|action| {
            let runtime_proxy_crate::SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
                id,
                artifact,
                content_hash: _,
                byte_len,
            } = action
            else {
                return None;
            };
            Some((
                id,
                format!(
                    "[psc rep {} b={}]",
                    runtime_smart_context_artifact_ref(&artifact.id),
                    byte_len
                ),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    if replacements.is_empty() {
        return;
    }
    stats.cross_turn_duplicate_texts +=
        runtime_smart_context_apply_text_replacements(value, &replacements);
}

fn runtime_smart_context_available_artifacts_for_text_items<'a>(
    items: impl IntoIterator<Item = &'a runtime_proxy_crate::SmartContextConversationItem>,
    store: &RuntimeSmartContextArtifactStore,
) -> Vec<runtime_proxy_crate::SmartContextArtifactRef> {
    items
        .into_iter()
        .filter_map(|item| {
            let content_hash = runtime_proxy_crate::smart_context_hash_text(&item.text);
            let artifact_text = store.get_text(&content_hash)?;
            (artifact_text == item.text).then_some(runtime_proxy_crate::SmartContextArtifactRef {
                id: content_hash.clone(),
                byte_len: item.text.len(),
                content_hash,
            })
        })
        .collect()
}

fn runtime_smart_context_collect_large_rehydratable_text_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextConversationItem> {
    let mut items = Vec::new();
    let Some(object) = value.as_object() else {
        return items;
    };
    let mut keys = object.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if runtime_smart_context_static_prompt_field_key(&key) {
            continue;
        }
        if let Some(item) = object.get(&key) {
            runtime_smart_context_collect_large_text_items_from_value(key, item, &mut items);
        }
    }
    items
}

fn runtime_smart_context_collect_large_text_items_from_value(
    id: String,
    value: &serde_json::Value,
    items: &mut Vec<runtime_proxy_crate::SmartContextConversationItem>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text)
            if text.len() >= SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES
                && !text.contains("prodex-artifact:")
                && !text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
                && !text.contains("prodex smart context artifact")
                && !text.contains("prodex-sc ") =>
        {
            items.push(runtime_proxy_crate::SmartContextConversationItem {
                id,
                text: text.clone(),
            });
        }
        serde_json::Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                runtime_smart_context_collect_large_text_items_from_value(
                    format!("{id}[{index}]"),
                    item,
                    items,
                );
            }
        }
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(item) = object.get(&key) {
                    if runtime_smart_context_static_prompt_field_key(&key) {
                        continue;
                    }
                    runtime_smart_context_collect_large_text_items_from_value(
                        format!("{id}.{key}"),
                        item,
                        items,
                    );
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_apply_text_replacements(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    let mut keys = object.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if runtime_smart_context_static_prompt_field_key(&key) {
            continue;
        }
        if let Some(item) = object.get_mut(&key) {
            replaced +=
                runtime_smart_context_apply_text_replacements_to_value(item, key, replacements);
        }
    }
    replaced
}

fn runtime_smart_context_apply_text_replacements_to_value(
    value: &mut serde_json::Value,
    id: String,
    replacements: &BTreeMap<String, String>,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            if let Some(replacement) = replacements.get(&id) {
                *text = replacement.clone();
                1
            } else {
                0
            }
        }
        serde_json::Value::Array(values) => values
            .iter_mut()
            .enumerate()
            .map(|(index, item)| {
                runtime_smart_context_apply_text_replacements_to_value(
                    item,
                    format!("{id}[{index}]"),
                    replacements,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.into_iter()
                .filter_map(|key| {
                    if runtime_smart_context_static_prompt_field_key(&key) {
                        return None;
                    }
                    object.get_mut(&key).map(|item| {
                        runtime_smart_context_apply_text_replacements_to_value(
                            item,
                            format!("{id}.{key}"),
                            replacements,
                        )
                    })
                })
                .sum()
        }
        _ => 0,
    }
}

fn runtime_smart_context_likely_blob_or_noise(text: &str) -> bool {
    prodex_context::is_context_blob_noise(text)
}

fn runtime_smart_context_critical_signal_self_check(
    before: &[u8],
    after: &[u8],
) -> prodex_context::CriticalSignalSelfCheck {
    let before = String::from_utf8_lossy(before);
    let after = String::from_utf8_lossy(after);
    prodex_context::critical_signal_self_check(&before, &after)
}

fn runtime_smart_context_regression_self_check(
    before: &[u8],
    after: &[u8],
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
) -> runtime_proxy_crate::SmartContextRegressionSelfCheck {
    let before_text = String::from_utf8_lossy(before);
    let after_text = String::from_utf8_lossy(after);
    runtime_proxy_crate::smart_context_regression_self_check(
        runtime_proxy_crate::SmartContextRegressionSelfCheckInput {
            exactness_guard,
            before_hash: runtime_proxy_crate::smart_context_hash_text(&before_text),
            after_hash: runtime_proxy_crate::smart_context_hash_text(&after_text),
            before_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(before.len()),
            after_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(after.len()),
            before_critical_signal_count: prodex_context::count_critical_signals(&before_text)
                .total(),
            after_critical_signal_count: prodex_context::count_critical_signals(&after_text)
                .total(),
            missing_rehydrate_refs,
        },
    )
}

fn runtime_smart_context_fallback_exact_reason(
    regression_check: &runtime_proxy_crate::SmartContextRegressionSelfCheck,
    critical_signal_check: prodex_context::CriticalSignalSelfCheck,
    stats: &RuntimeSmartContextTransformStats,
) -> Option<&'static str> {
    if critical_signal_check.has_loss() {
        return Some("critical_signal_loss");
    }
    if runtime_smart_context_rewrite_is_rehydrate_only(stats) {
        return None;
    }
    if regression_check.decision
        == runtime_proxy_crate::SmartContextRegressionSelfCheckDecision::FallbackExact
    {
        return Some(runtime_smart_context_regression_reason_label(
            &regression_check.reasons,
        ));
    }
    None
}

fn runtime_smart_context_rewrite_is_rehydrate_only(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs > 0
        && stats.tool_outputs_condensed == 0
        && stats.duplicate_texts == 0
        && stats.cross_turn_duplicate_texts == 0
        && stats.repeat_tool_output_refs == 0
        && stats.static_context_deltas == 0
}

fn runtime_smart_context_regression_reason_label(
    reasons: &[runtime_proxy_crate::SmartContextRegressionSelfCheckReason],
) -> &'static str {
    if reasons
        .iter()
        .any(|reason| matches!(reason, runtime_proxy_crate::SmartContextRegressionSelfCheckReason::CriticalSignalDropped))
    {
        "critical_signal_loss"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::MissingRehydrateRefs
        )
    }) {
        "missing_rehydrate_refs"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged
        )
    }) {
        "exactness_required"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::EmptyAfterPayload
        )
    }) {
        "empty_after_payload"
    } else {
        "token_budget_did_not_improve"
    }
}

fn runtime_smart_context_minified_json_body(
    value: &serde_json::Value,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_value_body(original_body, value)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

fn runtime_smart_context_minified_json_body_from_original(original_body: &[u8]) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_body(original_body)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

fn runtime_smart_context_validated_minified_json_body(
    body: Vec<u8>,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    if body.len() >= original_body.len() {
        return None;
    }
    let before = String::from_utf8_lossy(original_body);
    let after = String::from_utf8_lossy(&body);
    prodex_context::critical_signal_self_check(&before, &after)
        .passed()
        .then_some(body)
}

#[cfg(test)]
fn runtime_smart_context_should_pass_through_after_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs == 0
        && stats.static_context_deltas == 0
        && body_bytes_after >= body_bytes_before
}

fn runtime_smart_context_tier_label(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> &'static str {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => "exact",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => "large",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => "condensed",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => "minimal",
    }
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/smart_context.rs"]
mod tests;
