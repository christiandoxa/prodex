use super::{
    RuntimeProxyRequest, RuntimeRotationProxyShared, RuntimeRouteKind,
    RuntimeSmartContextBudgetInput, RuntimeSmartContextLogInput, RuntimeSmartContextPrepareError,
    RuntimeSmartContextRewriteSafetyObservation, RuntimeSmartContextTransformStats,
    RuntimeSmartContextTransport, SMART_CONTEXT_REWRITE_DEADLINE_MS,
    commit_runtime_smart_context_proxy_state_for_scope,
    observe_runtime_smart_context_rewrite_safety_with_state, runtime_request_previous_response_id,
    runtime_request_session_id, runtime_request_turn_state,
    runtime_smart_context_affinity_pressure_rewrite_allowed,
    runtime_smart_context_affinity_pressure_rewrite_guard,
    runtime_smart_context_append_inline_reference_protocol,
    runtime_smart_context_body_may_contain_artifact_ref, runtime_smart_context_budget_for_parsed,
    runtime_smart_context_collect_rehydratable_artifact_ref_ids,
    runtime_smart_context_critical_signal_self_check,
    runtime_smart_context_dedupe_input_text_within_request, runtime_smart_context_exact_header,
    runtime_smart_context_expand_inline_references, runtime_smart_context_fallback_exact_reason,
    runtime_smart_context_has_duplicate_input_text, runtime_smart_context_log,
    runtime_smart_context_log_prepare_fallback,
    runtime_smart_context_missing_artifact_refs_in_store,
    runtime_smart_context_normalized_model_name,
    runtime_smart_context_proxy_state_snapshot_for_scope, runtime_smart_context_reason_labels,
    runtime_smart_context_regression_self_check, runtime_smart_context_rehydrate_value,
    runtime_smart_context_rewrite_self_check, runtime_smart_context_scope_id,
    runtime_smart_context_tier_label, runtime_smart_context_unsupported_json_shape_reason,
};
use std::borrow::Cow;
use std::time::{Duration, Instant};

mod transform;

use transform::{RuntimeSmartContextBodyTransformInput, runtime_smart_context_transform_body};

pub(super) fn prepare_runtime_smart_context_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    rollout: &runtime_proxy_crate::SmartContextRolloutDecision,
) -> Result<Cow<'a, [u8]>, RuntimeSmartContextPrepareError> {
    let started_at = Instant::now();
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "invalid_json",
        );
        return Ok(Cow::Borrowed(&request.body));
    };
    if let Some(reason) = runtime_smart_context_unsupported_json_shape_reason(&value) {
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
    if !runtime_smart_context_body_may_contain_artifact_ref(&request.body)
        && !runtime_smart_context_has_duplicate_input_text(&value)
    {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "no_duplicate_candidate",
        );
        return Ok(Cow::Borrowed(&request.body));
    }
    let model_name = runtime_smart_context_normalized_model_name(
        value.get("model").and_then(serde_json::Value::as_str),
    );
    let request_token_count = runtime_proxy_crate::smart_context_count_serialized_request(
        &request.body,
        model_name.as_deref(),
    );
    if !request_token_count.is_proven() {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "unsupported_tokenizer",
        );
        return Ok(Cow::Borrowed(&request.body));
    }
    let Some(scope) = runtime_smart_context_scope_id(shared, profile_name) else {
        return Ok(Cow::Borrowed(&request.body));
    };
    let original_value = value.clone();

    let Some((state_generation, mut planned_state)) =
        runtime_smart_context_proxy_state_snapshot_for_scope(shared, &scope)
    else {
        return Ok(Cow::Borrowed(&request.body));
    };
    let missing_rehydrate_refs = runtime_smart_context_missing_artifact_refs_in_store(
        runtime_smart_context_collect_rehydratable_artifact_ref_ids(&value),
        &planned_state.artifacts,
    );
    if !missing_rehydrate_refs.is_empty() {
        return Err(RuntimeSmartContextPrepareError {
            missing_artifact_count: missing_rehydrate_refs.len(),
        });
    }
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
    let mut budget = runtime_smart_context_budget_for_parsed(
        RuntimeSmartContextBudgetInput {
            shared,
            body: &request.body,
            route_kind,
            transport,
            profile_name,
            exactness_guard: exactness.clone(),
            missing_rehydrate_refs: missing_rehydrate_refs.clone(),
            static_context_changed: false,
        },
        model_name.as_deref(),
        &request_token_count,
    );
    let affinity_pressure_rewrite =
        runtime_smart_context_affinity_pressure_rewrite_allowed(&exactness, &budget);
    let transform_exactness = if affinity_pressure_rewrite {
        runtime_smart_context_affinity_pressure_rewrite_guard(&exactness)
    } else {
        exactness.clone()
    };
    if affinity_pressure_rewrite {
        budget = runtime_smart_context_budget_for_parsed(
            RuntimeSmartContextBudgetInput {
                shared,
                body: &request.body,
                route_kind,
                transport,
                profile_name,
                exactness_guard: transform_exactness.clone(),
                missing_rehydrate_refs: missing_rehydrate_refs.clone(),
                static_context_changed: false,
            },
            model_name.as_deref(),
            &request_token_count,
        );
    }
    let tier = budget.tier;
    let rewrite_reason_label = if affinity_pressure_rewrite {
        "affinity_pressure"
    } else {
        "-"
    };

    if exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact
        && !affinity_pressure_rewrite
    {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "require_exact",
            reasons: &runtime_smart_context_reason_labels(&exactness.reasons),
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            token_count_after: &request_token_count,
            self_check: "pass_through_exact",
        });
        return Ok(Cow::Borrowed(&request.body));
    }

    let outcome = runtime_smart_context_transform_body(
        RuntimeSmartContextBodyTransformInput { budget: &budget },
        &mut planned_state,
        &mut value,
    );
    let mut stats = outcome.stats.clone();
    if value == original_value {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "pass_through",
            reasons: rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &budget,
            token_count_after: &request_token_count,
            self_check: "noop",
        });
        return Ok(Cow::Borrowed(&request.body));
    }

    let Ok(body) = serde_json::to_vec(&value) else {
        return Ok(Cow::Borrowed(&request.body));
    };
    let body_token_count =
        runtime_proxy_crate::smart_context_count_serialized_request(&body, model_name.as_deref());
    let self_check =
        runtime_smart_context_rewrite_self_check(request.body.len(), body.len(), &stats);
    let mut unresolved_rehydrate_refs = missing_rehydrate_refs;
    let quality_body = runtime_smart_context_expand_inline_references(&original_value, &value)
        .and_then(|expanded| serde_json::to_vec(&expanded).ok());
    if quality_body.is_none() && stats.duplicate_texts > 0 {
        unresolved_rehydrate_refs.push("invalid_inline_reference".to_string());
    }
    let quality_body = quality_body.as_deref().unwrap_or(&body);
    let regression_check = runtime_smart_context_regression_self_check(
        &request.body,
        &body,
        quality_body,
        &request_token_count,
        &body_token_count,
        transform_exactness.clone(),
        unresolved_rehydrate_refs.clone(),
    );
    let critical_signal_check =
        runtime_smart_context_critical_signal_self_check(&request.body, quality_body);
    if let Some(fallback_reason) = runtime_smart_context_fallback_exact_reason(
        &regression_check,
        critical_signal_check,
        &stats,
    ) {
        stats.full_request_fallback_count = stats.full_request_fallback_count.saturating_add(1);
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "self_check_passthrough",
            reasons: rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &budget,
            token_count_after: &request_token_count,
            self_check: fallback_reason,
        });
        return Ok(Cow::Borrowed(&request.body));
    }
    if started_at.elapsed() > Duration::from_millis(SMART_CONTEXT_REWRITE_DEADLINE_MS) {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "deadline_passthrough",
            reasons: "deadline_exceeded",
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &budget,
            token_count_after: &request_token_count,
            self_check: "pass_through_exact",
        });
        return Ok(Cow::Borrowed(&request.body));
    }
    if rollout.computes_shadow() {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "shadow_rewrite",
            reasons: rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: body.len(),
            stats,
            budget: &budget,
            token_count_after: &body_token_count,
            self_check,
        });
        return Ok(Cow::Borrowed(&request.body));
    }
    observe_runtime_smart_context_rewrite_safety_with_state(
        &mut planned_state,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: regression_check.saved_tokens,
        },
    );
    if !commit_runtime_smart_context_proxy_state_for_scope(
        shared,
        &scope,
        state_generation,
        planned_state,
    ) {
        return Ok(Cow::Borrowed(&request.body));
    }
    runtime_smart_context_log(RuntimeSmartContextLogInput {
        request_id,
        shared,
        scope: &scope,
        rollout,
        route_kind,
        transport,
        tier: runtime_smart_context_tier_label(tier),
        decision: "rewritten",
        reasons: rewrite_reason_label,
        body_bytes_before: request.body.len(),
        body_bytes_after: body.len(),
        stats,
        budget: &budget,
        token_count_after: &body_token_count,
        self_check,
    });
    Ok(Cow::Owned(body))
}
