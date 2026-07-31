use super::{
    RuntimeProxyRequest, RuntimeRotationProxyShared, RuntimeRouteKind,
    RuntimeSmartContextBudgetInput, RuntimeSmartContextLogInput, RuntimeSmartContextPrepareError,
    RuntimeSmartContextRewriteSafetyObservation, RuntimeSmartContextTransformOutcome,
    RuntimeSmartContextTransformStats, RuntimeSmartContextTransport,
    SMART_CONTEXT_REWRITE_DEADLINE_MS, commit_runtime_smart_context_proxy_state_for_scope,
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
    runtime_smart_context_has_duplicate_input_text,
    runtime_smart_context_inline_reference_round_trip_is_exact, runtime_smart_context_log,
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
    let Some(mut prepared) = prepare_runtime_smart_context_body_input(
        request_id,
        request,
        shared,
        route_kind,
        transport,
        profile_name,
    )?
    else {
        return Ok(Cow::Borrowed(&request.body));
    };

    if prepared.exactness.decision
        == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact
        && !prepared.affinity_pressure_rewrite
    {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &prepared.scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(prepared.budget.tier),
            decision: "require_exact",
            reasons: &runtime_smart_context_reason_labels(&prepared.exactness.reasons),
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &prepared.budget,
            token_count_after: &prepared.request_token_count,
            self_check: "pass_through_exact",
        });
        return Ok(Cow::Borrowed(&request.body));
    }

    let outcome = runtime_smart_context_transform_body(
        RuntimeSmartContextBodyTransformInput {
            budget: &prepared.budget,
        },
        &mut prepared.planned_state,
        &mut prepared.value,
    );
    if prepared.value == prepared.original_value {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &prepared.scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(prepared.budget.tier),
            decision: "pass_through",
            reasons: prepared.rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: outcome.stats,
            budget: &prepared.budget,
            token_count_after: &prepared.request_token_count,
            self_check: "noop",
        });
        return Ok(Cow::Borrowed(&request.body));
    }

    finish_runtime_smart_context_body(RuntimeSmartContextBodyFinishContext {
        request_id,
        request,
        shared,
        route_kind,
        transport,
        rollout,
        started_at,
        prepared,
        outcome,
    })
}

struct RuntimeSmartContextBodyInput {
    value: serde_json::Value,
    original_value: serde_json::Value,
    model_name: Option<String>,
    request_token_count: runtime_proxy_crate::SmartContextTokenCount,
    scope: runtime_proxy_crate::ContextScopeId,
    state_generation: u64,
    planned_state: super::RuntimeSmartContextProxyState,
    missing_rehydrate_refs: Vec<String>,
    exactness: runtime_proxy_crate::SmartContextExactnessGuard,
    transform_exactness: runtime_proxy_crate::SmartContextExactnessGuard,
    budget: super::RuntimeSmartContextBudget,
    affinity_pressure_rewrite: bool,
    rewrite_reason_label: &'static str,
}

fn prepare_runtime_smart_context_body_input(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> Result<Option<RuntimeSmartContextBodyInput>, RuntimeSmartContextPrepareError> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        runtime_smart_context_log_prepare_fallback(
            request_id,
            shared,
            route_kind,
            transport,
            profile_name,
            request.body.len(),
            "invalid_json",
        );
        return Ok(None);
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
        return Ok(None);
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
        return Ok(None);
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
        return Ok(None);
    }
    let Some(scope) = runtime_smart_context_scope_id(shared, profile_name) else {
        return Ok(None);
    };
    let original_value = value.clone();
    let Some((state_generation, planned_state)) =
        runtime_smart_context_proxy_state_snapshot_for_scope(shared, &scope)
    else {
        return Ok(None);
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
    Ok(Some(RuntimeSmartContextBodyInput {
        value,
        original_value,
        model_name,
        request_token_count,
        scope,
        state_generation,
        planned_state,
        missing_rehydrate_refs,
        exactness,
        transform_exactness,
        budget,
        affinity_pressure_rewrite,
        rewrite_reason_label: if affinity_pressure_rewrite {
            "affinity_pressure"
        } else {
            "-"
        },
    }))
}

struct RuntimeSmartContextBodyFinishContext<'request, 'shared, 'rollout> {
    request_id: u64,
    request: &'request RuntimeProxyRequest,
    shared: &'shared RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    rollout: &'rollout runtime_proxy_crate::SmartContextRolloutDecision,
    started_at: Instant,
    prepared: RuntimeSmartContextBodyInput,
    outcome: RuntimeSmartContextTransformOutcome,
}

fn finish_runtime_smart_context_body<'a>(
    context: RuntimeSmartContextBodyFinishContext<'a, '_, '_>,
) -> Result<Cow<'a, [u8]>, RuntimeSmartContextPrepareError> {
    let RuntimeSmartContextBodyFinishContext {
        request_id,
        request,
        shared,
        route_kind,
        transport,
        rollout,
        started_at,
        mut prepared,
        outcome,
    } = context;
    let mut stats = outcome.stats;
    let Ok(body) = serde_json::to_vec(&prepared.value) else {
        return Ok(Cow::Borrowed(&request.body));
    };
    let body_token_count = runtime_proxy_crate::smart_context_count_serialized_request(
        &body,
        prepared.model_name.as_deref(),
    );
    let self_check =
        runtime_smart_context_rewrite_self_check(request.body.len(), body.len(), &stats);
    let mut unresolved_rehydrate_refs = prepared.missing_rehydrate_refs;
    let mut expanded =
        runtime_smart_context_expand_inline_references(&prepared.original_value, &prepared.value);
    let exact_inline_round_trip = stats.duplicate_texts > 0
        && stats.rehydrated_refs == 0
        && expanded.as_mut().is_some_and(|expanded| {
            runtime_smart_context_inline_reference_round_trip_is_exact(
                &prepared.original_value,
                expanded,
            )
        });
    if expanded.is_none() && stats.duplicate_texts > 0 {
        unresolved_rehydrate_refs.push("invalid_inline_reference".to_string());
    }
    let quality_body = (!exact_inline_round_trip)
        .then(|| {
            expanded
                .as_ref()
                .and_then(|value| serde_json::to_vec(value).ok())
        })
        .flatten();
    let critical_signal_check = if exact_inline_round_trip {
        let counts =
            prodex_context::count_critical_signals(&String::from_utf8_lossy(&request.body));
        prodex_context::CriticalSignalSelfCheck {
            before: counts,
            after: counts,
            lost: Default::default(),
            gained: Default::default(),
        }
    } else {
        runtime_smart_context_critical_signal_self_check(
            &request.body,
            quality_body.as_deref().unwrap_or(&body),
        )
    };
    let regression_check = runtime_smart_context_regression_self_check(
        &request.body,
        &body,
        &prepared.request_token_count,
        &body_token_count,
        critical_signal_check,
        prepared.transform_exactness.clone(),
        unresolved_rehydrate_refs.clone(),
    );
    if let Some(fallback_reason) = runtime_smart_context_fallback_exact_reason(
        &regression_check,
        critical_signal_check,
        &stats,
    ) {
        stats.full_request_fallback_count = stats.full_request_fallback_count.saturating_add(1);
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &prepared.scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(prepared.budget.tier),
            decision: "self_check_passthrough",
            reasons: prepared.rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &prepared.budget,
            token_count_after: &prepared.request_token_count,
            self_check: fallback_reason,
        });
        return Ok(Cow::Borrowed(&request.body));
    }
    if started_at.elapsed() > Duration::from_millis(SMART_CONTEXT_REWRITE_DEADLINE_MS) {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &prepared.scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(prepared.budget.tier),
            decision: "deadline_passthrough",
            reasons: "deadline_exceeded",
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &prepared.budget,
            token_count_after: &prepared.request_token_count,
            self_check: "pass_through_exact",
        });
        return Ok(Cow::Borrowed(&request.body));
    }
    if rollout.computes_shadow() {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            scope: &prepared.scope,
            rollout,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(prepared.budget.tier),
            decision: "shadow_rewrite",
            reasons: prepared.rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: body.len(),
            stats,
            budget: &prepared.budget,
            token_count_after: &body_token_count,
            self_check,
        });
        return Ok(Cow::Borrowed(&request.body));
    }
    observe_runtime_smart_context_rewrite_safety_with_state(
        &mut prepared.planned_state,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: regression_check.saved_tokens,
        },
    );
    if !commit_runtime_smart_context_proxy_state_for_scope(
        shared,
        &prepared.scope,
        prepared.state_generation,
        prepared.planned_state,
    ) {
        return Ok(Cow::Borrowed(&request.body));
    }
    runtime_smart_context_log(RuntimeSmartContextLogInput {
        request_id,
        shared,
        scope: &prepared.scope,
        rollout,
        route_kind,
        transport,
        tier: runtime_smart_context_tier_label(prepared.budget.tier),
        decision: "rewritten",
        reasons: prepared.rewrite_reason_label,
        body_bytes_before: request.body.len(),
        body_bytes_after: body.len(),
        stats,
        budget: &prepared.budget,
        token_count_after: &body_token_count,
        self_check,
    });
    Ok(Cow::Owned(body))
}
