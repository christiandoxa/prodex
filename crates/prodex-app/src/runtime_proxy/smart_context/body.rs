use super::{
    RuntimeProxyRequest, RuntimeRotationProxyShared, RuntimeRouteKind,
    RuntimeSmartContextBudgetInput, RuntimeSmartContextLogInput,
    RuntimeSmartContextRewriteSafetyObservation, RuntimeSmartContextTransformOutcome,
    RuntimeSmartContextTransformStats, RuntimeSmartContextTransport,
    observe_runtime_smart_context_rewrite_safety, persist_runtime_smart_context_artifacts,
    persist_runtime_smart_context_token_calibration_metadata, runtime_request_previous_response_id,
    runtime_request_session_id, runtime_request_turn_state,
    runtime_smart_context_affinity_pressure_rewrite_allowed,
    runtime_smart_context_affinity_pressure_rewrite_guard,
    runtime_smart_context_append_artifact_manifest_delta_if_useful,
    runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state,
    runtime_smart_context_apply_path_aliases_to_generated_texts,
    runtime_smart_context_apply_repo_state_micro_cache,
    runtime_smart_context_apply_static_context_chunk_dedupe,
    runtime_smart_context_apply_static_context_cross_field_dedupe,
    runtime_smart_context_apply_static_context_delta,
    runtime_smart_context_apply_static_context_persistent_section_dedupe,
    runtime_smart_context_apply_static_context_section_dedupe,
    runtime_smart_context_auto_rehydrate_plan, runtime_smart_context_budget,
    runtime_smart_context_collect_intent_signals,
    runtime_smart_context_condense_historical_tool_call_arguments,
    runtime_smart_context_condense_tool_outputs, runtime_smart_context_critical_signal_self_check,
    runtime_smart_context_dedupe_input_text, runtime_smart_context_deferred_rehydrate_refs,
    runtime_smart_context_exact_header, runtime_smart_context_fallback_exact_reason,
    runtime_smart_context_log, runtime_smart_context_minified_json_body,
    runtime_smart_context_minified_json_body_from_original,
    runtime_smart_context_missing_artifact_refs, runtime_smart_context_observe_static_context,
    runtime_smart_context_reason_labels, runtime_smart_context_regression_self_check,
    runtime_smart_context_rehydrate_value_with_plan, runtime_smart_context_rewrite_self_check,
    runtime_smart_context_saved_tokens,
    runtime_smart_context_selective_rehydrate_budget_aware_ranges,
    runtime_smart_context_tier_label, runtime_smart_context_try_surgical_rehydrate_critical_ranges,
    runtime_smart_context_unsupported_json_shape_reason, with_runtime_smart_context_proxy_state,
};
use std::borrow::Cow;

pub(super) fn prepare_runtime_smart_context_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> Cow<'a, [u8]> {
    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared,
        body: &request.body,
        route_kind,
        transport,
        profile_name,
        exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: "invalid_json",
            decision: "pass_through",
            reasons: "-",
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            self_check: "pass_through",
        });
        return Cow::Borrowed(&request.body);
    };
    if let Some(reason) = runtime_smart_context_unsupported_json_shape_reason(&value) {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(budget.tier),
            decision: "unsupported_json_shape",
            reasons: reason,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            self_check: "pass_through",
        });
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
    let mut budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared,
        body: &request.body,
        route_kind,
        transport,
        profile_name,
        exactness_guard: exactness.clone(),
        missing_rehydrate_refs: missing_rehydrate_refs.clone(),
        static_context_changed: static_observation.changed,
    });
    let affinity_pressure_rewrite =
        runtime_smart_context_affinity_pressure_rewrite_allowed(&exactness, &budget);
    let transform_exactness = if affinity_pressure_rewrite {
        runtime_smart_context_affinity_pressure_rewrite_guard(&exactness)
    } else {
        exactness.clone()
    };
    if affinity_pressure_rewrite {
        budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
            shared,
            body: &request.body,
            route_kind,
            transport,
            profile_name,
            exactness_guard: transform_exactness.clone(),
            missing_rehydrate_refs: missing_rehydrate_refs.clone(),
            static_context_changed: static_observation.changed,
        });
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
            runtime_smart_context_log(RuntimeSmartContextLogInput {
                request_id,
                shared,
                route_kind,
                transport,
                tier: runtime_smart_context_tier_label(tier),
                decision: "require_exact",
                reasons: &runtime_smart_context_reason_labels(&exactness.reasons),
                body_bytes_before: request.body.len(),
                body_bytes_after: body.len(),
                stats: RuntimeSmartContextTransformStats::default(),
                budget: &budget,
                self_check: "ok_minified",
            });
            return Cow::Owned(body);
        }
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "require_exact",
            reasons: &runtime_smart_context_reason_labels(&exactness.reasons),
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            self_check: "pass_through_exact",
        });
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
            runtime_smart_context_log(RuntimeSmartContextLogInput {
                request_id,
                shared,
                route_kind,
                transport,
                tier: runtime_smart_context_tier_label(tier),
                decision: "artifact_store_unavailable",
                reasons: rewrite_reason_label,
                body_bytes_before: request.body.len(),
                body_bytes_after: body.len(),
                stats: RuntimeSmartContextTransformStats::default(),
                budget: &budget,
                self_check: "ok_minified",
            });
            return Cow::Owned(body);
        }
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "artifact_store_unavailable",
            reasons: rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            self_check: "pass_through",
        });
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
    let mut stats = outcome.stats.clone();
    if stats.artifacts_stored > 0 {
        persist_runtime_smart_context_artifacts(shared);
    }

    if stats == RuntimeSmartContextTransformStats::default() && !generated_aliases_used {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(RuntimeSmartContextLogInput {
                request_id,
                shared,
                route_kind,
                transport,
                tier: runtime_smart_context_tier_label(tier),
                decision: "minified",
                reasons: rewrite_reason_label,
                body_bytes_before: request.body.len(),
                body_bytes_after: body.len(),
                stats,
                budget: &budget,
                self_check: "ok_minified",
            });
            return Cow::Owned(body);
        }
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "pass_through",
            reasons: rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &budget,
            self_check: "noop",
        });
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
        && let Some((repaired_body, mut repaired_stats)) =
            runtime_smart_context_try_surgical_rehydrate_critical_ranges(
                &value,
                shared,
                &request.body,
                &transform_exactness,
                &unresolved_rehydrate_refs,
                &stats,
            )
    {
        repaired_stats.segment_rollback_count =
            repaired_stats.segment_rollback_count.saturating_add(1);
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
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "rewritten",
            reasons: if affinity_pressure_rewrite {
                "affinity_pressure,surgical_rehydrate"
            } else {
                "surgical_rehydrate"
            },
            body_bytes_before: request.body.len(),
            body_bytes_after: repaired_body.len(),
            stats: repaired_stats,
            budget: &budget,
            self_check: "ok_surgical_rehydrate",
        });
        return Cow::Owned(repaired_body);
    }
    if let Some(fallback_reason) = runtime_smart_context_fallback_exact_reason(
        &regression_check,
        critical_signal_check,
        &stats,
    ) {
        stats.full_request_fallback_count = stats.full_request_fallback_count.saturating_add(1);
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: false,
                saved_tokens: 0,
            },
        );
        if let Some(body) = runtime_smart_context_minified_json_body_from_original(&request.body) {
            runtime_smart_context_log(RuntimeSmartContextLogInput {
                request_id,
                shared,
                route_kind,
                transport,
                tier: runtime_smart_context_tier_label(tier),
                decision: "self_check_passthrough",
                reasons: rewrite_reason_label,
                body_bytes_before: request.body.len(),
                body_bytes_after: body.len(),
                stats,
                budget: &budget,
                self_check: fallback_reason,
            });
            return Cow::Owned(body);
        }
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id,
            shared,
            route_kind,
            transport,
            tier: runtime_smart_context_tier_label(tier),
            decision: "self_check_passthrough",
            reasons: rewrite_reason_label,
            body_bytes_before: request.body.len(),
            body_bytes_after: request.body.len(),
            stats,
            budget: &budget,
            self_check: fallback_reason,
        });
        return Cow::Borrowed(&request.body);
    }
    observe_runtime_smart_context_rewrite_safety(
        shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: regression_check.saved_tokens,
        },
    );
    runtime_smart_context_log(RuntimeSmartContextLogInput {
        request_id,
        shared,
        route_kind,
        transport,
        tier: runtime_smart_context_tier_label(tier),
        decision: "rewritten",
        reasons: rewrite_reason_label,
        body_bytes_before: request.body.len(),
        body_bytes_after: body.len(),
        stats,
        budget: &budget,
        self_check,
    });
    Cow::Owned(body)
}
