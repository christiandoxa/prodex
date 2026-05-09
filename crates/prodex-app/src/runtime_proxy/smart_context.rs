use super::*;
mod artifact_manifest;
mod artifact_refs;
mod constants;
mod intent;
mod panic_guard;
mod rehydration;
mod repo_state;
mod rewrite_telemetry;
mod rewrite_validation;
mod runtime_rehydrate;
mod static_context;
mod token_calibration;
mod tool_outputs;
mod types;

use artifact_manifest::*;
use artifact_refs::*;
use constants::*;
use intent::*;
use panic_guard::*;
use rehydration::*;
use repo_state::*;
use rewrite_telemetry::*;
use rewrite_validation::*;
use runtime_rehydrate::*;
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
