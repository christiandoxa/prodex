use super::*;

pub(super) struct RuntimeSmartContextBudgetInput<'a> {
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) body: &'a [u8],
    pub(super) route_kind: RuntimeRouteKind,
    pub(super) transport: RuntimeSmartContextTransport,
    pub(super) profile_name: Option<&'a str>,
    pub(super) exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    pub(super) missing_rehydrate_refs: Vec<String>,
    pub(super) static_context_changed: bool,
}

pub(super) fn runtime_smart_context_budget(
    input: RuntimeSmartContextBudgetInput<'_>,
) -> RuntimeSmartContextBudget {
    let model_name = runtime_smart_context_model_name_from_body(input.body);
    let bucket_key = runtime_smart_context_token_calibration_bucket_key_with_model(
        input.route_kind,
        input.transport,
        input.profile_name,
        model_name.as_deref(),
    );
    let (
        global_history,
        bucket_history,
        calibration_samples,
        configured_context_window_tokens,
        recent_rewrite_safety,
        rewrite_telemetry_samples,
    ) = runtime_smart_context_budget_inputs(input.shared, &bucket_key);
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
                current_request_body_bytes: input.body.len(),
                current_request_estimated_tokens: Some(
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body(input.body),
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
            exactness_guard: input.exactness_guard,
            accounting,
            recent_rewrite_safety,
            static_context_changed: input.static_context_changed,
            missing_rehydrate_refs: input.missing_rehydrate_refs,
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

#[cfg(test)]
pub(super) fn runtime_smart_context_token_calibration_bucket_key(
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

pub(super) fn runtime_smart_context_token_calibration_bucket_key_with_model(
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

pub(super) fn runtime_smart_context_budget_inputs(
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

pub(super) fn runtime_smart_context_recent_rewrite_safety(
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
