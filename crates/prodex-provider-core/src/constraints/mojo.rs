use super::*;
use prodex_mojo_core::provider_constraints as mojo_constraints;

pub(super) fn resolve_requirement_input(
    input: mojo_constraints::RequirementResolutionInput,
) -> Result<mojo_constraints::RequirementResolution, prodex_mojo_core::MojoError> {
    mojo_constraints::resolve_requirement_input(input)
}

pub(super) fn resolve_requirements(
    mut requirements: ProviderRequestRequirements,
    entry: &ProviderCatalogEntry,
) -> ProviderRequestRequirements {
    let mut reasoning_reserve_by_effort = [None; 9];
    if let Some(reserves) = entry.reasoning_reserve_tokens.as_ref() {
        for (effort, value) in reserves {
            reasoning_reserve_by_effort[usize::try_from(reasoning_effort_to_mojo(*effort))
                .expect("Mojo reasoning effort tag fits")] = Some(*value);
        }
    }
    let resolution = resolve_requirement_input(mojo_constraints::RequirementResolutionInput {
        explicit_output_present: requirements.explicit_output_tokens.is_some(),
        default_output_reserve_tokens: entry.default_output_reserve_tokens,
        requested_reasoning_effort: requirements.reasoning_effort.map(reasoning_effort_to_mojo),
        default_reasoning_effort: entry.default_reasoning_effort.map(reasoning_effort_to_mojo),
        reasoning_reserve_tokens: requirements.reasoning_reserve_tokens,
        reasoning_reserve_by_effort,
    })
    .expect("Mojo provider requirement resolution returned invalid output");
    requirements.default_output_reserve_tokens = resolution.default_output_reserve_tokens;
    requirements.reasoning_effort = resolution.reasoning_effort.map(|effort| match effort {
        0 => ProviderReasoningEffort::None,
        1 => ProviderReasoningEffort::Minimal,
        2 => ProviderReasoningEffort::Low,
        3 => ProviderReasoningEffort::Medium,
        4 => ProviderReasoningEffort::High,
        5 => ProviderReasoningEffort::XHigh,
        6 => ProviderReasoningEffort::Max,
        7 => ProviderReasoningEffort::Ultra,
        _ => ProviderReasoningEffort::Unknown,
    });
    requirements.reasoning_reserve_tokens = resolution.reasoning_reserve_tokens;
    requirements
}

pub(super) fn evaluate(
    provider: ProviderId,
    requirements: &ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
    resolved: &ProviderRequestRequirements,
    entry: Option<&ProviderCatalogEntry>,
) -> Result<ProviderRequestConstraintEvaluation, prodex_mojo_core::MojoError> {
    let preclassification =
        mojo_constraints::preclassify(mojo_constraints::PreclassificationInput {
            endpoint_kind: endpoint_kind(requirements.endpoint),
            provider_endpoint_supported: !matches!(
                provider_adapter(provider).capability_status(requirements.endpoint),
                ProviderCapabilityStatus::Unsupported
            ),
            catalog_entry_present: entry.is_some(),
            provider_streaming_supported: provider_adapter(provider).supports_streaming(),
            supported_endpoint_mask: entry.map_or(0, endpoint_mask),
            feature_mask: entry.map_or(0, |entry| feature_mask(provider, entry)),
            required_features: requirements
                .required_features
                .iter()
                .copied()
                .map(feature_to_mojo)
                .collect(),
            reasoning_effort: resolved.reasoning_effort.map(reasoning_effort_to_mojo),
            supported_reasoning_efforts: entry.and_then(|entry| {
                entry
                    .supported_reasoning_efforts
                    .as_ref()
                    .map(|efforts| reasoning_effort_mask(efforts))
            }),
        })?;
    let input = mojo_constraints::Input {
        policy_enabled: policy.enabled,
        endpoint_supported: preclassification.endpoint_supported,
        catalog_entry_present: entry.is_some(),
        embeddings_endpoint: requirements.endpoint == ProviderEndpoint::Embeddings,
        missing_feature: preclassification.missing_feature,
        reasoning_effort_unsupported: preclassification.reasoning_effort_unsupported,
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

pub(super) fn reasoning_effort_to_mojo(effort: ProviderReasoningEffort) -> i64 {
    match effort {
        ProviderReasoningEffort::None => 0,
        ProviderReasoningEffort::Minimal => 1,
        ProviderReasoningEffort::Low => 2,
        ProviderReasoningEffort::Medium => 3,
        ProviderReasoningEffort::High => 4,
        ProviderReasoningEffort::XHigh => 5,
        ProviderReasoningEffort::Max => 6,
        ProviderReasoningEffort::Ultra => 7,
        ProviderReasoningEffort::Unknown => 8,
    }
}

fn endpoint_kind(endpoint: ProviderEndpoint) -> i64 {
    match endpoint {
        ProviderEndpoint::Responses => 0,
        ProviderEndpoint::ResponsesCompact => 1,
        ProviderEndpoint::ChatCompletions => 2,
        ProviderEndpoint::Messages => 3,
        ProviderEndpoint::Models => 4,
        ProviderEndpoint::Embeddings => 5,
        ProviderEndpoint::Images => 6,
        ProviderEndpoint::Audio => 7,
        ProviderEndpoint::Batches => 8,
        ProviderEndpoint::Rerank => 9,
        ProviderEndpoint::A2a => 10,
    }
}

fn endpoint_mask(entry: &ProviderCatalogEntry) -> u64 {
    entry
        .supported_endpoints
        .iter()
        .map(|endpoint| endpoint_kind(*endpoint))
        .fold(0, |mask, endpoint| mask | (1_u64 << endpoint))
}

fn feature_mask(provider: ProviderId, entry: &ProviderCatalogEntry) -> u64 {
    let flags = [
        entry.feature_flags.tools,
        entry.feature_flags.json_schema,
        entry.feature_flags.vision,
        entry.feature_flags.audio,
        entry.feature_flags.web_search,
        entry.feature_flags.reasoning,
        provider_adapter(provider).supports_streaming(),
        entry_supports_endpoint_mask(entry, ProviderEndpoint::ResponsesCompact),
        false,
    ];
    flags
        .into_iter()
        .enumerate()
        .fold(0, |mask, (index, supported)| {
            mask | u64::from(supported) << index
        })
}

fn entry_supports_endpoint_mask(entry: &ProviderCatalogEntry, endpoint: ProviderEndpoint) -> bool {
    let kind = endpoint_kind(endpoint);
    if endpoint == ProviderEndpoint::ResponsesCompact {
        (endpoint_mask(entry) & 1) != 0 || (endpoint_mask(entry) & (1 << kind)) != 0
    } else {
        endpoint_mask(entry) & (1 << kind) != 0
    }
}

fn reasoning_effort_mask(efforts: &[ProviderReasoningEffort]) -> u64 {
    efforts.iter().fold(0, |mask, effort| {
        mask | (1_u64 << reasoning_effort_to_mojo(*effort))
    })
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
