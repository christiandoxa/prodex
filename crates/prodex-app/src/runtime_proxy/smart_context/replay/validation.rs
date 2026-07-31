pub(super) struct RuntimeSmartContextReplayTurnChecks<'a> {
    pub(super) scenario: &'a runtime_proxy_crate::SmartContextReplayScenarioInput,
    pub(super) turn: &'a runtime_proxy_crate::SmartContextReplayTurnInput,
    pub(super) exact_byte_identity: bool,
    pub(super) exact_state_mutations: u64,
    pub(super) optimized_state_mutations: u64,
    pub(super) valid_json: bool,
    pub(super) required_text_preserved: bool,
    pub(super) protocol_fields_preserved: bool,
    pub(super) unresolved_artifact_references: &'a [String],
    pub(super) rewrite_applied: bool,
    pub(super) blocked_before_upstream: bool,
    pub(super) net_saved_tokens: i64,
    pub(super) exact_input_tokens: u64,
    pub(super) optimized_input_tokens: u64,
}

pub(super) fn runtime_smart_context_replay_turn_failures(
    checks: RuntimeSmartContextReplayTurnChecks<'_>,
) -> Vec<String> {
    let RuntimeSmartContextReplayTurnChecks {
        scenario,
        turn,
        exact_byte_identity,
        exact_state_mutations,
        optimized_state_mutations,
        valid_json,
        required_text_preserved,
        protocol_fields_preserved,
        unresolved_artifact_references,
        rewrite_applied,
        blocked_before_upstream,
        net_saved_tokens,
        exact_input_tokens,
        optimized_input_tokens,
    } = checks;
    let mut failures =
        runtime_smart_context_replay_invariant_failures(RuntimeSmartContextReplayInvariantChecks {
            scenario,
            exact_byte_identity,
            exact_state_mutations,
            optimized_state_mutations,
            valid_json,
            required_text_preserved,
            protocol_fields_preserved,
            unresolved_artifact_references,
        });
    if let Some(failure) = runtime_smart_context_replay_expected_failure(
        turn.expected_outcome,
        rewrite_applied,
        blocked_before_upstream,
    ) {
        failures.push(failure.to_string());
    }
    if rewrite_applied && net_saved_tokens <= 0 {
        failures.push("rewrite_not_token_positive".to_string());
    }
    if optimized_input_tokens > exact_input_tokens {
        failures.push("aggregate_input_tokens_increased".to_string());
    }
    failures
}

struct RuntimeSmartContextReplayInvariantChecks<'a> {
    scenario: &'a runtime_proxy_crate::SmartContextReplayScenarioInput,
    exact_byte_identity: bool,
    exact_state_mutations: u64,
    optimized_state_mutations: u64,
    valid_json: bool,
    required_text_preserved: bool,
    protocol_fields_preserved: bool,
    unresolved_artifact_references: &'a [String],
}

fn runtime_smart_context_replay_invariant_failures(
    checks: RuntimeSmartContextReplayInvariantChecks<'_>,
) -> Vec<String> {
    let RuntimeSmartContextReplayInvariantChecks {
        scenario,
        exact_byte_identity,
        exact_state_mutations,
        optimized_state_mutations,
        valid_json,
        required_text_preserved,
        protocol_fields_preserved,
        unresolved_artifact_references,
    } = checks;
    let mut failures = Vec::new();
    if !exact_byte_identity {
        failures.push("exact_body_changed".to_string());
    }
    if exact_state_mutations != 0 {
        failures.push("exact_state_mutated".to_string());
    }
    if matches!(
        scenario.mode,
        runtime_proxy_crate::SmartContextReplayMode::Exact
            | runtime_proxy_crate::SmartContextReplayMode::Shadow
            | runtime_proxy_crate::SmartContextReplayMode::CanaryOut
    ) && optimized_state_mutations != 0
    {
        failures.push("pass_through_mode_state_mutated".to_string());
    }
    if !valid_json {
        failures.push("optimized_body_invalid_json".to_string());
    }
    if !required_text_preserved {
        failures.push("required_text_missing".to_string());
    }
    if !protocol_fields_preserved {
        failures.push("protocol_field_changed".to_string());
    }
    if !unresolved_artifact_references.is_empty() {
        failures.push("unresolved_artifact_reference".to_string());
    }
    failures
}

fn runtime_smart_context_replay_expected_failure(
    expected_outcome: runtime_proxy_crate::SmartContextReplayExpectedOutcome,
    rewrite_applied: bool,
    blocked_before_upstream: bool,
) -> Option<&'static str> {
    match expected_outcome {
        runtime_proxy_crate::SmartContextReplayExpectedOutcome::Rewrite if !rewrite_applied => {
            Some("expected_rewrite_not_applied")
        }
        runtime_proxy_crate::SmartContextReplayExpectedOutcome::PassThrough
            if rewrite_applied || blocked_before_upstream =>
        {
            Some("expected_pass_through_not_observed")
        }
        runtime_proxy_crate::SmartContextReplayExpectedOutcome::MissingArtifactFailure
            if !blocked_before_upstream =>
        {
            Some("expected_missing_artifact_failure_not_observed")
        }
        _ => None,
    }
}

pub(super) fn runtime_smart_context_replay_fallback_reason(
    scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
    blocked_before_upstream: bool,
    rewrite_applied: bool,
) -> Option<&'static str> {
    if blocked_before_upstream {
        return Some("missing_artifact");
    }
    if rewrite_applied {
        return None;
    }
    Some(match scenario.mode {
        runtime_proxy_crate::SmartContextReplayMode::Exact => "explicit_exact",
        runtime_proxy_crate::SmartContextReplayMode::Shadow => "shadow",
        runtime_proxy_crate::SmartContextReplayMode::CanaryOut => "canary_out",
        runtime_proxy_crate::SmartContextReplayMode::Active => match scenario.route {
            runtime_proxy_crate::SmartContextReplayRoute::Responses
            | runtime_proxy_crate::SmartContextReplayRoute::Websocket => "no_op",
            runtime_proxy_crate::SmartContextReplayRoute::Compact
            | runtime_proxy_crate::SmartContextReplayRoute::Standard => "unsupported_route",
        },
    })
}
