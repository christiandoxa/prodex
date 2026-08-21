use super::*;

pub(super) fn evaluate(
    provider: ProviderId,
    requirements: &ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
    resolved: &ProviderRequestRequirements,
    entry: Option<&ProviderCatalogEntry>,
) -> Result<ProviderRequestConstraintEvaluation, prodex_mojo_core::MojoError> {
    let input = prodex_mojo_core::provider_constraints::Input {
        policy_enabled: policy.enabled,
        endpoint_supported: endpoint_supported(provider, requirements.endpoint, entry),
        catalog_entry_present: entry.is_some(),
        embeddings_endpoint: requirements.endpoint == ProviderEndpoint::Embeddings,
        missing_feature: entry.and_then(|entry| {
            requirements
                .required_features
                .iter()
                .copied()
                .find(|feature| !entry_supports_feature(provider, entry, *feature))
                .map(feature_tag)
        }),
        reasoning_effort_unsupported: entry
            .is_some_and(|entry| unsupported_reasoning_effort(resolved, entry)),
        estimated_input_tokens: resolved.estimated_input_tokens,
        explicit_output_tokens: resolved.explicit_output_tokens,
        default_output_reserve_tokens: resolved.default_output_reserve_tokens,
        reasoning_reserve_tokens: resolved.reasoning_reserve_tokens,
        max_output_tokens: entry.and_then(|entry| entry.max_output_tokens),
        context_window_tokens: entry.and_then(|entry| entry.context_window_tokens),
        unknown_context_policy: unknown_context_policy_tag(policy.unknown_context),
        safe_window_tokens: policy.safe_window_tokens,
        oversized_output_policy: oversized_output_policy_tag(policy.oversized_output),
        output_limit_field: resolved.output_limit_field.map(output_limit_field_tag),
    };
    let result = prodex_mojo_core::provider_constraints::evaluate(input)?;
    let decision =
        decision_from_tag(result.decision).ok_or(prodex_mojo_core::MojoError::InvalidOutput)?;
    let missing_feature = match result.missing_feature {
        Some(tag) => Some(feature_from_tag(tag).ok_or(prodex_mojo_core::MojoError::InvalidOutput)?),
        None => None,
    };
    let mut requirements = resolved.clone();
    if let Some(adjusted) = result.adjusted_output_tokens {
        requirements.explicit_output_tokens = Some(adjusted);
    }
    requirements.total_required_tokens = result.total_required_tokens;
    let mut evaluation = super::evaluation(decision, result.eligible, requirements, entry);
    evaluation.missing_feature = missing_feature;
    evaluation.available_context_tokens = result.available_context_tokens;
    evaluation.max_output_tokens = result.max_output_tokens;
    for (tag, warning) in [
        (
            super::ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit,
            1_u64 << 7,
        ),
        (
            super::ProviderRequestConstraintDecision::OutputLimitUnknown,
            1_u64 << 6,
        ),
        (
            super::ProviderRequestConstraintDecision::ContextWindowUnknown,
            1_u64 << 4,
        ),
        (
            super::ProviderRequestConstraintDecision::CatalogEntryUnavailable,
            1_u64 << 3,
        ),
    ] {
        if result.warnings & warning != 0 {
            evaluation.warnings.push(tag);
        }
    }
    if let (Some(field), Some(applied), Some(reason)) = (
        result.adjustment_field,
        result.adjusted_output_tokens,
        result.adjustment_reason,
    ) {
        evaluation.adjustment = Some(ProviderOutputAdjustment {
            field: output_limit_field_from_tag(field)
                .ok_or(prodex_mojo_core::MojoError::InvalidOutput)?,
            requested_tokens: resolved
                .explicit_output_tokens
                .ok_or(prodex_mojo_core::MojoError::InvalidOutput)?,
            applied_tokens: applied,
            reason: decision_from_tag(reason).ok_or(prodex_mojo_core::MojoError::InvalidOutput)?,
        });
    }
    Ok(evaluation)
}

fn endpoint_supported(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    entry: Option<&ProviderCatalogEntry>,
) -> bool {
    !matches!(
        provider_adapter(provider).capability_status(endpoint),
        ProviderCapabilityStatus::Unsupported
    ) && entry.is_none_or(|entry| entry_supports_endpoint(entry, endpoint))
}

fn feature_tag(feature: ProviderRequestFeature) -> i64 {
    match feature {
        ProviderRequestFeature::Tools => 0,
        ProviderRequestFeature::JsonSchema => 1,
        ProviderRequestFeature::Vision => 2,
        ProviderRequestFeature::Audio => 3,
        ProviderRequestFeature::WebSearch => 4,
        ProviderRequestFeature::Reasoning => 5,
        ProviderRequestFeature::Streaming => 6,
        ProviderRequestFeature::Compact => 7,
        ProviderRequestFeature::Websocket => 8,
    }
}

fn feature_from_tag(tag: i64) -> Option<ProviderRequestFeature> {
    Some(match tag {
        0 => ProviderRequestFeature::Tools,
        1 => ProviderRequestFeature::JsonSchema,
        2 => ProviderRequestFeature::Vision,
        3 => ProviderRequestFeature::Audio,
        4 => ProviderRequestFeature::WebSearch,
        5 => ProviderRequestFeature::Reasoning,
        6 => ProviderRequestFeature::Streaming,
        7 => ProviderRequestFeature::Compact,
        8 => ProviderRequestFeature::Websocket,
        _ => return None,
    })
}

fn output_limit_field_tag(field: ProviderOutputLimitField) -> i64 {
    match field {
        ProviderOutputLimitField::MaxOutputTokens => 0,
        ProviderOutputLimitField::MaxCompletionTokens => 1,
        ProviderOutputLimitField::MaxTokens => 2,
    }
}

fn output_limit_field_from_tag(tag: i64) -> Option<ProviderOutputLimitField> {
    Some(match tag {
        0 => ProviderOutputLimitField::MaxOutputTokens,
        1 => ProviderOutputLimitField::MaxCompletionTokens,
        2 => ProviderOutputLimitField::MaxTokens,
        _ => return None,
    })
}

fn unknown_context_policy_tag(policy: ProviderUnknownContextPolicy) -> i64 {
    match policy {
        ProviderUnknownContextPolicy::Allow => 0,
        ProviderUnknownContextPolicy::SafeWindow => 1,
        ProviderUnknownContextPolicy::Reject => 2,
    }
}

fn oversized_output_policy_tag(policy: ProviderOversizedOutputPolicy) -> i64 {
    match policy {
        ProviderOversizedOutputPolicy::Passthrough => 0,
        ProviderOversizedOutputPolicy::Reject => 1,
        ProviderOversizedOutputPolicy::ClampWithNotice => 2,
    }
}

fn decision_from_tag(tag: i64) -> Option<ProviderRequestConstraintDecision> {
    Some(match tag {
        0 => ProviderRequestConstraintDecision::Compatible,
        1 => ProviderRequestConstraintDecision::EndpointUnsupported,
        2 => ProviderRequestConstraintDecision::RequiredCapabilityMissing,
        3 => ProviderRequestConstraintDecision::CatalogEntryUnavailable,
        4 => ProviderRequestConstraintDecision::ContextWindowUnknown,
        5 => ProviderRequestConstraintDecision::ContextWindowExceeded,
        6 => ProviderRequestConstraintDecision::OutputLimitUnknown,
        7 => ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit,
        8 => ProviderRequestConstraintDecision::ReasoningReserveUnsupported,
        9 => ProviderRequestConstraintDecision::ReasoningReserveExcessive,
        10 => ProviderRequestConstraintDecision::MalformedRequestLimits,
        11 => ProviderRequestConstraintDecision::OutputLimitClamped,
        12 => ProviderRequestConstraintDecision::AffinityOwnerUnavailable,
        _ => return None,
    })
}
