//! Pure model-aware request requirement and constraint evaluation.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderCatalogEntry, ProviderEndpoint,
    ProviderId, estimate_request_input_tokens_value, provider_adapter, provider_catalog_entry,
};

#[path = "constraints/features.rs"]
mod features;
use self::features::inferred_features;

pub const PROVIDER_REQUEST_SAFE_WINDOW_TOKENS_DEFAULT: u64 = 128_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRequestFeature {
    Tools,
    JsonSchema,
    Vision,
    Audio,
    WebSearch,
    Reasoning,
    Streaming,
    Compact,
    Websocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Unknown,
}

impl ProviderReasoningEffort {
    pub(super) fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "minimal" => Self::Minimal,
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "xhigh" => Self::XHigh,
            _ => Self::Unknown,
        }
    }

    pub(super) fn reserves_tokens(self) -> bool {
        !matches!(self, Self::None | Self::Minimal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutputLimitField {
    MaxOutputTokens,
    MaxCompletionTokens,
    MaxTokens,
}

impl ProviderOutputLimitField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaxOutputTokens => "max_output_tokens",
            Self::MaxCompletionTokens => "max_completion_tokens",
            Self::MaxTokens => "max_tokens",
        }
    }
}

const PROVIDER_OUTPUT_LIMIT_FIELDS: [(ProviderOutputLimitField, &str); 3] = [
    (
        ProviderOutputLimitField::MaxOutputTokens,
        "max_output_tokens",
    ),
    (ProviderOutputLimitField::MaxTokens, "max_tokens"),
    (
        ProviderOutputLimitField::MaxCompletionTokens,
        "max_completion_tokens",
    ),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUnknownContextPolicy {
    #[default]
    Allow,
    SafeWindow,
    Reject,
}

impl ProviderUnknownContextPolicy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "allow" => Some(Self::Allow),
            "safe_window" => Some(Self::SafeWindow),
            "reject" => Some(Self::Reject),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOversizedOutputPolicy {
    #[default]
    Passthrough,
    Reject,
    ClampWithNotice,
}

impl ProviderOversizedOutputPolicy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "passthrough" => Some(Self::Passthrough),
            "reject" => Some(Self::Reject),
            "clamp_with_notice" => Some(Self::ClampWithNotice),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderRequestConstraintPolicy {
    pub enabled: bool,
    pub unknown_context: ProviderUnknownContextPolicy,
    pub safe_window_tokens: u64,
    pub oversized_output: ProviderOversizedOutputPolicy,
}

impl Default for ProviderRequestConstraintPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            unknown_context: ProviderUnknownContextPolicy::Allow,
            safe_window_tokens: PROVIDER_REQUEST_SAFE_WINDOW_TOKENS_DEFAULT,
            oversized_output: ProviderOversizedOutputPolicy::Passthrough,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRequestConstraintDecision {
    Compatible,
    EndpointUnsupported,
    RequiredCapabilityMissing,
    CatalogEntryUnavailable,
    ContextWindowUnknown,
    ContextWindowExceeded,
    OutputLimitUnknown,
    RequestedOutputExceedsModelLimit,
    ReasoningReserveUnsupported,
    ReasoningReserveExcessive,
    MalformedRequestLimits,
    OutputLimitClamped,
    AffinityOwnerUnavailable,
}

impl ProviderRequestConstraintDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::EndpointUnsupported => "endpoint_unsupported",
            Self::RequiredCapabilityMissing => "required_capability_missing",
            Self::CatalogEntryUnavailable => "catalog_entry_unavailable",
            Self::ContextWindowUnknown => "context_window_unknown",
            Self::ContextWindowExceeded => "context_window_exceeded",
            Self::OutputLimitUnknown => "output_limit_unknown",
            Self::RequestedOutputExceedsModelLimit => "requested_output_exceeds_model_limit",
            Self::ReasoningReserveUnsupported => "reasoning_reserve_unsupported",
            Self::ReasoningReserveExcessive => "reasoning_reserve_excessive",
            Self::MalformedRequestLimits => "malformed_request_limits",
            Self::OutputLimitClamped => "output_limit_clamped",
            Self::AffinityOwnerUnavailable => "affinity_owner_unavailable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequestRequirements {
    pub endpoint: ProviderEndpoint,
    pub requested_model: String,
    pub resolved_upstream_model: Option<String>,
    pub estimated_input_tokens: u64,
    pub explicit_output_tokens: Option<u64>,
    pub output_limit_field: Option<ProviderOutputLimitField>,
    pub default_output_reserve_tokens: Option<u64>,
    pub reasoning_effort: Option<ProviderReasoningEffort>,
    pub reasoning_reserve_tokens: Option<u64>,
    pub total_required_tokens: u64,
    pub required_features: Vec<ProviderRequestFeature>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderOutputAdjustment {
    pub field: ProviderOutputLimitField,
    pub requested_tokens: u64,
    pub applied_tokens: u64,
    pub reason: ProviderRequestConstraintDecision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequestConstraintEvaluation {
    pub decision: ProviderRequestConstraintDecision,
    pub eligible: bool,
    pub requirements: ProviderRequestRequirements,
    pub missing_feature: Option<ProviderRequestFeature>,
    pub available_context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub adjustment: Option<ProviderOutputAdjustment>,
    pub warnings: Vec<ProviderRequestConstraintDecision>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRequestLimitErrorKind {
    InvalidJson,
    InvalidRoot,
    InvalidOutputValue,
    ConflictingOutputFields,
    InvalidReasoningBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequestLimitError {
    pub kind: ProviderRequestLimitErrorKind,
    pub field: Option<ProviderOutputLimitField>,
}

impl fmt::Display for ProviderRequestLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("request token limits are malformed")
    }
}

impl std::error::Error for ProviderRequestLimitError {}

pub fn provider_request_requirements(
    body: &[u8],
    endpoint: ProviderEndpoint,
    requested_model: &str,
    additional_features: &[ProviderRequestFeature],
) -> Result<ProviderRequestRequirements, ProviderRequestLimitError> {
    let value = serde_json::from_slice::<serde_json::Value>(body).map_err(|_| {
        ProviderRequestLimitError {
            kind: ProviderRequestLimitErrorKind::InvalidJson,
            field: None,
        }
    })?;
    provider_request_requirements_from_value(&value, endpoint, requested_model, additional_features)
}

pub fn provider_request_requirements_from_value(
    value: &serde_json::Value,
    endpoint: ProviderEndpoint,
    requested_model: &str,
    additional_features: &[ProviderRequestFeature],
) -> Result<ProviderRequestRequirements, ProviderRequestLimitError> {
    let object = value.as_object().ok_or(ProviderRequestLimitError {
        kind: ProviderRequestLimitErrorKind::InvalidRoot,
        field: None,
    })?;
    let (output_limit_field, explicit_output_tokens) = provider_requested_output_tokens(value)?;
    let reasoning_effort = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(serde_json::Value::as_str)
        .map(ProviderReasoningEffort::parse);
    let reasoning_reserve_tokens = object
        .get("thinking")
        .and_then(serde_json::Value::as_object)
        .and_then(|thinking| thinking.get("budget_tokens"))
        .map(|value| {
            value
                .as_u64()
                .filter(|tokens| *tokens > 0)
                .ok_or(ProviderRequestLimitError {
                    kind: ProviderRequestLimitErrorKind::InvalidReasoningBudget,
                    field: None,
                })
        })
        .transpose()?;
    let mut required_features = inferred_features(value, endpoint);
    required_features.extend(additional_features.iter().copied());
    let required_features = required_features.into_iter().collect::<Vec<_>>();
    let estimated_input_tokens = estimate_request_input_tokens_value(value)
        .unwrap_or(1)
        .max(1);
    let total_required_tokens = estimated_input_tokens
        .saturating_add(explicit_output_tokens.unwrap_or_default())
        .saturating_add(reasoning_reserve_tokens.unwrap_or_default());
    Ok(ProviderRequestRequirements {
        endpoint,
        requested_model: requested_model.trim().to_string(),
        resolved_upstream_model: None,
        estimated_input_tokens,
        explicit_output_tokens,
        output_limit_field,
        default_output_reserve_tokens: None,
        reasoning_effort,
        reasoning_reserve_tokens,
        total_required_tokens,
        required_features,
    })
}

pub fn provider_requested_output_tokens(
    value: &serde_json::Value,
) -> Result<(Option<ProviderOutputLimitField>, Option<u64>), ProviderRequestLimitError> {
    let object = value.as_object().ok_or(ProviderRequestLimitError {
        kind: ProviderRequestLimitErrorKind::InvalidRoot,
        field: None,
    })?;
    let mut found = None;
    for (field, name) in PROVIDER_OUTPUT_LIMIT_FIELDS {
        let Some(value) = object.get(name) else {
            continue;
        };
        if found.is_some() {
            return Err(ProviderRequestLimitError {
                kind: ProviderRequestLimitErrorKind::ConflictingOutputFields,
                field: Some(field),
            });
        }
        let tokens =
            value
                .as_u64()
                .filter(|tokens| *tokens > 0)
                .ok_or(ProviderRequestLimitError {
                    kind: ProviderRequestLimitErrorKind::InvalidOutputValue,
                    field: Some(field),
                })?;
        found = Some((field, tokens));
    }
    Ok(found
        .map(|(field, tokens)| (Some(field), Some(tokens)))
        .unwrap_or((None, None)))
}

pub fn provider_requested_output_tokens_compat(value: &serde_json::Value) -> Option<u64> {
    let object = value.as_object()?;
    PROVIDER_OUTPUT_LIMIT_FIELDS
        .iter()
        .find_map(|(_, name)| object.get(*name).and_then(serde_json::Value::as_u64))
}

pub fn evaluate_provider_request_constraints(
    provider: ProviderId,
    resolved_model: &str,
    requirements: &ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
) -> ProviderRequestConstraintEvaluation {
    evaluate_provider_request_constraints_with_catalog_entry(
        provider,
        resolved_model,
        requirements,
        policy,
        provider_catalog_entry(provider, resolved_model),
    )
}

pub fn evaluate_provider_request_constraints_with_catalog_entry(
    provider: ProviderId,
    resolved_model: &str,
    requirements: &ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
    entry: Option<&ProviderCatalogEntry>,
) -> ProviderRequestConstraintEvaluation {
    let mut resolved = requirements.clone();
    resolved.resolved_upstream_model = Some(
        entry
            .map(|entry| entry.id.clone())
            .unwrap_or_else(|| resolved_model.trim().to_string()),
    );
    if let Some(entry) = entry {
        if resolved.explicit_output_tokens.is_none() {
            resolved.default_output_reserve_tokens = entry.default_output_reserve_tokens;
        }
        let effort = resolved.reasoning_effort.or(entry.default_reasoning_effort);
        resolved.reasoning_effort = effort;
        if resolved.reasoning_reserve_tokens.is_none() {
            resolved.reasoning_reserve_tokens = effort
                .filter(|effort| effort.reserves_tokens())
                .and_then(|effort| {
                    entry
                        .reasoning_reserve_tokens
                        .as_ref()
                        .and_then(|reserves| reserves.get(&effort).copied())
                });
        }
    }
    recalculate_total(&mut resolved);

    if !policy.enabled {
        return evaluation(
            ProviderRequestConstraintDecision::Compatible,
            true,
            resolved,
            entry,
        );
    }
    if matches!(
        provider_adapter(provider).capability_status(requirements.endpoint),
        ProviderCapabilityStatus::Unsupported
    ) {
        return evaluation(
            ProviderRequestConstraintDecision::EndpointUnsupported,
            false,
            resolved,
            entry,
        );
    }
    if requirements.endpoint == ProviderEndpoint::Embeddings && entry.is_none() {
        return evaluation(
            ProviderRequestConstraintDecision::EndpointUnsupported,
            false,
            resolved,
            None,
        );
    }
    let Some(entry) = entry else {
        return unknown_catalog_evaluation(resolved, policy);
    };
    if !entry_supports_endpoint(entry, requirements.endpoint) {
        return evaluation(
            ProviderRequestConstraintDecision::EndpointUnsupported,
            false,
            resolved,
            Some(entry),
        );
    }
    if let Some(feature) = requirements
        .required_features
        .iter()
        .copied()
        .find(|feature| !entry_supports_feature(provider, entry, *feature))
    {
        let decision = if feature == ProviderRequestFeature::Reasoning {
            ProviderRequestConstraintDecision::ReasoningReserveUnsupported
        } else {
            ProviderRequestConstraintDecision::RequiredCapabilityMissing
        };
        let mut result = evaluation(decision, false, resolved, Some(entry));
        result.missing_feature = Some(feature);
        return result;
    }
    if resolved.reasoning_effort == Some(ProviderReasoningEffort::Unknown)
        || resolved.reasoning_effort.is_some_and(|effort| {
            effort.reserves_tokens()
                && entry
                    .supported_reasoning_efforts
                    .as_ref()
                    .is_some_and(|efforts| !efforts.contains(&effort))
        })
    {
        return evaluation(
            ProviderRequestConstraintDecision::ReasoningReserveUnsupported,
            false,
            resolved,
            Some(entry),
        );
    }

    let mut result = evaluation(
        ProviderRequestConstraintDecision::Compatible,
        true,
        resolved,
        Some(entry),
    );
    if let Some(requested) = result.requirements.explicit_output_tokens {
        match entry.max_output_tokens {
            Some(limit) if requested > limit => match policy.oversized_output {
                ProviderOversizedOutputPolicy::Passthrough => {
                    result.decision =
                        ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit;
                    result.warnings.push(result.decision);
                }
                ProviderOversizedOutputPolicy::Reject => {
                    result.decision =
                        ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit;
                    result.eligible = false;
                    return result;
                }
                ProviderOversizedOutputPolicy::ClampWithNotice => {
                    result.decision = ProviderRequestConstraintDecision::OutputLimitClamped;
                    result.adjustment = Some(ProviderOutputAdjustment {
                        field: result
                            .requirements
                            .output_limit_field
                            .unwrap_or(ProviderOutputLimitField::MaxOutputTokens),
                        requested_tokens: requested,
                        applied_tokens: limit,
                        reason: ProviderRequestConstraintDecision::OutputLimitClamped,
                    });
                    result.requirements.explicit_output_tokens = Some(limit);
                    recalculate_total(&mut result.requirements);
                }
            },
            None => {
                result.decision = ProviderRequestConstraintDecision::OutputLimitUnknown;
                result.warnings.push(result.decision);
            }
            Some(_) => {}
        }
    }

    let reasoning = result
        .requirements
        .reasoning_reserve_tokens
        .unwrap_or_default();
    match entry.context_window_tokens {
        Some(context) if result.requirements.total_required_tokens > context => {
            result.decision = if reasoning > 0
                && result
                    .requirements
                    .total_required_tokens
                    .saturating_sub(reasoning)
                    <= context
            {
                ProviderRequestConstraintDecision::ReasoningReserveExcessive
            } else {
                ProviderRequestConstraintDecision::ContextWindowExceeded
            };
            result.eligible = false;
        }
        Some(_) => {}
        None => match policy.unknown_context {
            ProviderUnknownContextPolicy::Allow => {
                result.decision = ProviderRequestConstraintDecision::ContextWindowUnknown;
                result.warnings.push(result.decision);
            }
            ProviderUnknownContextPolicy::Reject => {
                result.decision = ProviderRequestConstraintDecision::ContextWindowUnknown;
                result.eligible = false;
            }
            ProviderUnknownContextPolicy::SafeWindow => {
                result.available_context_tokens = Some(policy.safe_window_tokens);
                if result.requirements.total_required_tokens > policy.safe_window_tokens {
                    result.decision = ProviderRequestConstraintDecision::ContextWindowExceeded;
                    result.eligible = false;
                } else {
                    result.decision = ProviderRequestConstraintDecision::ContextWindowUnknown;
                    result.warnings.push(result.decision);
                }
            }
        },
    }
    result
}

fn recalculate_total(requirements: &mut ProviderRequestRequirements) {
    requirements.total_required_tokens = requirements
        .estimated_input_tokens
        .saturating_add(
            requirements
                .explicit_output_tokens
                .or(requirements.default_output_reserve_tokens)
                .unwrap_or_default(),
        )
        .saturating_add(requirements.reasoning_reserve_tokens.unwrap_or_default());
}

fn evaluation(
    decision: ProviderRequestConstraintDecision,
    eligible: bool,
    requirements: ProviderRequestRequirements,
    entry: Option<&ProviderCatalogEntry>,
) -> ProviderRequestConstraintEvaluation {
    ProviderRequestConstraintEvaluation {
        decision,
        eligible,
        requirements,
        missing_feature: None,
        available_context_tokens: entry.and_then(|entry| entry.context_window_tokens),
        max_output_tokens: entry.and_then(|entry| entry.max_output_tokens),
        adjustment: None,
        warnings: Vec::new(),
    }
}

fn unknown_catalog_evaluation(
    requirements: ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
) -> ProviderRequestConstraintEvaluation {
    let mut result = evaluation(
        ProviderRequestConstraintDecision::CatalogEntryUnavailable,
        policy.unknown_context != ProviderUnknownContextPolicy::Reject,
        requirements,
        None,
    );
    if policy.unknown_context == ProviderUnknownContextPolicy::SafeWindow {
        result.available_context_tokens = Some(policy.safe_window_tokens);
        if result.requirements.total_required_tokens > policy.safe_window_tokens {
            result.decision = ProviderRequestConstraintDecision::ContextWindowExceeded;
            result.eligible = false;
        }
    }
    if result.eligible {
        result
            .warnings
            .push(ProviderRequestConstraintDecision::CatalogEntryUnavailable);
    }
    result
}

fn entry_supports_feature(
    provider: ProviderId,
    entry: &ProviderCatalogEntry,
    feature: ProviderRequestFeature,
) -> bool {
    match feature {
        ProviderRequestFeature::Tools => entry.feature_flags.tools,
        ProviderRequestFeature::JsonSchema => entry.feature_flags.json_schema,
        ProviderRequestFeature::Vision => entry.feature_flags.vision,
        ProviderRequestFeature::Audio => entry.feature_flags.audio,
        ProviderRequestFeature::WebSearch => entry.feature_flags.web_search,
        ProviderRequestFeature::Reasoning => entry.feature_flags.reasoning,
        ProviderRequestFeature::Streaming => provider_adapter(provider).supports_streaming(),
        ProviderRequestFeature::Compact => {
            entry_supports_endpoint(entry, ProviderEndpoint::ResponsesCompact)
        }
        ProviderRequestFeature::Websocket => false,
    }
}

fn entry_supports_endpoint(entry: &ProviderCatalogEntry, endpoint: ProviderEndpoint) -> bool {
    match endpoint {
        ProviderEndpoint::ResponsesCompact => entry.supported_endpoints.iter().any(|supported| {
            matches!(
                supported,
                ProviderEndpoint::Responses | ProviderEndpoint::ResponsesCompact
            )
        }),
        ProviderEndpoint::Embeddings => entry
            .supported_endpoints
            .contains(&ProviderEndpoint::Embeddings),
        _ => entry.supported_endpoints.contains(&endpoint),
    }
}

#[cfg(test)]
#[path = "../tests/src/constraints.rs"]
mod tests;
