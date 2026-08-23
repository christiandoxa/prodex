use super::*;
use prodex_mojo_core::provider_constraints as mojo_constraints;

pub(super) fn evaluate(
    provider: ProviderId,
    requirements: &ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
    resolved: &ProviderRequestRequirements,
    entry: Option<&ProviderCatalogEntry>,
) -> Result<ProviderRequestConstraintEvaluation, prodex_mojo_core::MojoError> {
    let input = mojo_constraints::Input {
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
                .map(feature_to_mojo)
        }),
        reasoning_effort_unsupported: entry
            .is_some_and(|entry| unsupported_reasoning_effort(resolved, entry)),
        estimated_input_tokens: resolved.estimated_input_tokens,
        explicit_output_tokens: resolved.explicit_output_tokens,
        default_output_reserve_tokens: resolved.default_output_reserve_tokens,
        reasoning_reserve_tokens: resolved.reasoning_reserve_tokens,
        max_output_tokens: entry.and_then(|entry| entry.max_output_tokens),
        context_window_tokens: entry.and_then(|entry| entry.context_window_tokens),
        unknown_context_policy: unknown_context_policy_to_mojo(policy.unknown_context),
        safe_window_tokens: policy.safe_window_tokens,
        oversized_output_policy: oversized_output_policy_to_mojo(policy.oversized_output),
        output_limit_field: resolved.output_limit_field.map(output_limit_field_to_mojo),
    };
    let result = mojo_constraints::evaluate(input)?;
    let mut requirements = resolved.clone();
    if let Some(adjusted) = result.adjusted_output_tokens {
        requirements.explicit_output_tokens = Some(adjusted);
    }
    requirements.total_required_tokens = result.total_required_tokens;
    let mut evaluation = super::evaluation(
        decision_from_mojo(result.decision),
        result.eligible,
        requirements,
        entry,
    );
    evaluation.missing_feature = result.missing_feature.map(feature_from_mojo);
    evaluation.available_context_tokens = result.available_context_tokens;
    evaluation.max_output_tokens = result.max_output_tokens;
    for (tag, warning) in [
        (
            ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit,
            mojo_constraints::PROVIDER_CONSTRAINT_WARNING_OUTPUT_EXCEEDS_LIMIT,
        ),
        (
            ProviderRequestConstraintDecision::OutputLimitUnknown,
            mojo_constraints::PROVIDER_CONSTRAINT_WARNING_OUTPUT_UNKNOWN,
        ),
        (
            ProviderRequestConstraintDecision::ContextWindowUnknown,
            mojo_constraints::PROVIDER_CONSTRAINT_WARNING_CONTEXT_UNKNOWN,
        ),
        (
            ProviderRequestConstraintDecision::CatalogEntryUnavailable,
            mojo_constraints::PROVIDER_CONSTRAINT_WARNING_CATALOG_UNAVAILABLE,
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
            field: output_limit_field_from_mojo(field),
            requested_tokens: resolved
                .explicit_output_tokens
                .ok_or(prodex_mojo_core::MojoError::InvalidOutput)?,
            applied_tokens: applied,
            reason: decision_from_mojo(reason),
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

fn feature_to_mojo(feature: ProviderRequestFeature) -> mojo_constraints::Feature {
    match feature {
        ProviderRequestFeature::Tools => mojo_constraints::Feature::Tools,
        ProviderRequestFeature::JsonSchema => mojo_constraints::Feature::JsonSchema,
        ProviderRequestFeature::Vision => mojo_constraints::Feature::Vision,
        ProviderRequestFeature::Audio => mojo_constraints::Feature::Audio,
        ProviderRequestFeature::WebSearch => mojo_constraints::Feature::WebSearch,
        ProviderRequestFeature::Reasoning => mojo_constraints::Feature::Reasoning,
        ProviderRequestFeature::Streaming => mojo_constraints::Feature::Streaming,
        ProviderRequestFeature::Compact => mojo_constraints::Feature::Compact,
        ProviderRequestFeature::Websocket => mojo_constraints::Feature::Websocket,
    }
}

fn feature_from_mojo(feature: mojo_constraints::Feature) -> ProviderRequestFeature {
    match feature {
        mojo_constraints::Feature::Tools => ProviderRequestFeature::Tools,
        mojo_constraints::Feature::JsonSchema => ProviderRequestFeature::JsonSchema,
        mojo_constraints::Feature::Vision => ProviderRequestFeature::Vision,
        mojo_constraints::Feature::Audio => ProviderRequestFeature::Audio,
        mojo_constraints::Feature::WebSearch => ProviderRequestFeature::WebSearch,
        mojo_constraints::Feature::Reasoning => ProviderRequestFeature::Reasoning,
        mojo_constraints::Feature::Streaming => ProviderRequestFeature::Streaming,
        mojo_constraints::Feature::Compact => ProviderRequestFeature::Compact,
        mojo_constraints::Feature::Websocket => ProviderRequestFeature::Websocket,
    }
}

fn output_limit_field_to_mojo(
    field: ProviderOutputLimitField,
) -> mojo_constraints::OutputLimitField {
    match field {
        ProviderOutputLimitField::MaxOutputTokens => mojo_constraints::OutputLimitField::MaxOutput,
        ProviderOutputLimitField::MaxCompletionTokens => {
            mojo_constraints::OutputLimitField::MaxCompletion
        }
        ProviderOutputLimitField::MaxTokens => mojo_constraints::OutputLimitField::MaxTokens,
    }
}

fn output_limit_field_from_mojo(
    field: mojo_constraints::OutputLimitField,
) -> ProviderOutputLimitField {
    match field {
        mojo_constraints::OutputLimitField::MaxOutput => ProviderOutputLimitField::MaxOutputTokens,
        mojo_constraints::OutputLimitField::MaxCompletion => {
            ProviderOutputLimitField::MaxCompletionTokens
        }
        mojo_constraints::OutputLimitField::MaxTokens => ProviderOutputLimitField::MaxTokens,
    }
}

fn unknown_context_policy_to_mojo(
    policy: ProviderUnknownContextPolicy,
) -> mojo_constraints::UnknownContextPolicy {
    match policy {
        ProviderUnknownContextPolicy::Allow => mojo_constraints::UnknownContextPolicy::Allow,
        ProviderUnknownContextPolicy::SafeWindow => {
            mojo_constraints::UnknownContextPolicy::SafeWindow
        }
        ProviderUnknownContextPolicy::Reject => mojo_constraints::UnknownContextPolicy::Reject,
    }
}

fn oversized_output_policy_to_mojo(
    policy: ProviderOversizedOutputPolicy,
) -> mojo_constraints::OversizedOutputPolicy {
    match policy {
        ProviderOversizedOutputPolicy::Passthrough => {
            mojo_constraints::OversizedOutputPolicy::Passthrough
        }
        ProviderOversizedOutputPolicy::Reject => mojo_constraints::OversizedOutputPolicy::Reject,
        ProviderOversizedOutputPolicy::ClampWithNotice => {
            mojo_constraints::OversizedOutputPolicy::Clamp
        }
    }
}

fn decision_from_mojo(decision: mojo_constraints::Decision) -> ProviderRequestConstraintDecision {
    match decision {
        mojo_constraints::Decision::Compatible => ProviderRequestConstraintDecision::Compatible,
        mojo_constraints::Decision::EndpointUnsupported => {
            ProviderRequestConstraintDecision::EndpointUnsupported
        }
        mojo_constraints::Decision::RequiredCapabilityMissing => {
            ProviderRequestConstraintDecision::RequiredCapabilityMissing
        }
        mojo_constraints::Decision::CatalogUnavailable => {
            ProviderRequestConstraintDecision::CatalogEntryUnavailable
        }
        mojo_constraints::Decision::ContextUnknown => {
            ProviderRequestConstraintDecision::ContextWindowUnknown
        }
        mojo_constraints::Decision::ContextExceeded => {
            ProviderRequestConstraintDecision::ContextWindowExceeded
        }
        mojo_constraints::Decision::OutputUnknown => {
            ProviderRequestConstraintDecision::OutputLimitUnknown
        }
        mojo_constraints::Decision::OutputExceedsLimit => {
            ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit
        }
        mojo_constraints::Decision::ReasoningUnsupported => {
            ProviderRequestConstraintDecision::ReasoningReserveUnsupported
        }
        mojo_constraints::Decision::ReasoningExcessive => {
            ProviderRequestConstraintDecision::ReasoningReserveExcessive
        }
        mojo_constraints::Decision::MalformedLimits => {
            ProviderRequestConstraintDecision::MalformedRequestLimits
        }
        mojo_constraints::Decision::OutputClamped => {
            ProviderRequestConstraintDecision::OutputLimitClamped
        }
    }
}
