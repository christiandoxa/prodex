use super::*;
use crate::RuntimeTokenUsage;
use std::collections::BTreeSet;

pub fn smart_context_token_budget_tier(available_tokens: usize) -> SmartContextTokenBudgetTier {
    match available_tokens {
        16_000.. => SmartContextTokenBudgetTier::Exact,
        8_000..=15_999 => SmartContextTokenBudgetTier::Large,
        2_000..=7_999 => SmartContextTokenBudgetTier::Condensed,
        _ => SmartContextTokenBudgetTier::Minimal,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmartContextMemoryCapsule {
    pub id: String,
    pub token_cost: usize,
    pub relevance: f32,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextMemoryCapsuleSelection {
    pub selected_ids: Vec<String>,
    pub omitted_ids: Vec<String>,
    pub used_tokens: usize,
}

pub const SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET: usize = 256;
pub const SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET: usize = 1_024;
pub const SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET: usize = 4_096;

pub fn smart_context_select_memory_capsules_for_policy(
    capsules: impl IntoIterator<Item = SmartContextMemoryCapsule>,
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> SmartContextMemoryCapsuleSelection {
    smart_context_select_memory_capsules(
        capsules,
        smart_context_memory_capsule_token_budget(accounting, policy),
    )
}

pub fn smart_context_memory_capsule_token_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> usize {
    if smart_context_memory_capsule_policy_allows_unbounded_budget(accounting, policy) {
        return usize::MAX;
    }
    if !smart_context_memory_capsule_policy_allows_bounded_budget(accounting, policy) {
        return 0;
    }

    let Some(available_context_tokens) = accounting.available_context_tokens else {
        return 0;
    };

    let mode_budget = match policy.mode {
        SmartContextBudgetMode::MinimalRefsOnly => {
            SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
        }
        SmartContextBudgetMode::ArtifactCondensed => {
            SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
        }
        SmartContextBudgetMode::LargeLossless => SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET,
        SmartContextBudgetMode::ExactPassThrough => match policy.tier {
            SmartContextTokenBudgetTier::Exact | SmartContextTokenBudgetTier::Large => {
                SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET
            }
            SmartContextTokenBudgetTier::Condensed => {
                SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            SmartContextTokenBudgetTier::Minimal => {
                SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
            }
        },
    };

    mode_budget
        .min(smart_context_u64_saturating_usize(
            policy.max_rehydrate_tokens,
        ))
        .min(smart_context_u64_saturating_usize(available_context_tokens))
}

pub fn smart_context_select_memory_capsules(
    capsules: impl IntoIterator<Item = SmartContextMemoryCapsule>,
    token_budget: usize,
) -> SmartContextMemoryCapsuleSelection {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for capsule in capsules {
        if capsule.required {
            required.push(capsule);
        } else {
            optional.push(capsule);
        }
    }

    required.sort_by(|left, right| left.id.cmp(&right.id));
    optional.sort_by(smart_context_capsule_order);

    let mut selected_ids = Vec::new();
    let mut omitted_ids = Vec::new();
    let mut used_tokens = 0usize;

    for capsule in required.into_iter().chain(optional) {
        if used_tokens.saturating_add(capsule.token_cost) <= token_budget {
            used_tokens += capsule.token_cost;
            selected_ids.push(capsule.id);
        } else {
            omitted_ids.push(capsule.id);
        }
    }

    SmartContextMemoryCapsuleSelection {
        selected_ids,
        omitted_ids,
        used_tokens,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRehydrateRef {
    pub id: String,
    pub token_cost: usize,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextRehydrateAction {
    Rehydrate {
        id: String,
        token_cost: usize,
    },
    Defer {
        id: String,
        reason: SmartContextRehydrateDeferReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextRehydrateDeferReason {
    MissingArtifact,
    TokenBudgetExceeded,
    MinimalBudgetTier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRehydratePlan {
    pub actions: Vec<SmartContextRehydrateAction>,
    pub used_tokens: usize,
}

pub fn smart_context_auto_rehydrate_plan(
    refs: impl IntoIterator<Item = SmartContextRehydrateRef>,
    available_artifact_ids: impl IntoIterator<Item = String>,
    token_budget: usize,
    tier: SmartContextTokenBudgetTier,
) -> SmartContextRehydratePlan {
    let available = available_artifact_ids.into_iter().collect::<BTreeSet<_>>();
    let mut refs = refs.into_iter().collect::<Vec<_>>();
    refs.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| left.token_cost.cmp(&right.token_cost))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut actions = Vec::new();
    let mut used_tokens = 0usize;
    for item in refs {
        if !available.contains(&item.id) {
            actions.push(SmartContextRehydrateAction::Defer {
                id: item.id,
                reason: SmartContextRehydrateDeferReason::MissingArtifact,
            });
        } else if tier == SmartContextTokenBudgetTier::Minimal && !item.required {
            actions.push(SmartContextRehydrateAction::Defer {
                id: item.id,
                reason: SmartContextRehydrateDeferReason::MinimalBudgetTier,
            });
        } else if used_tokens.saturating_add(item.token_cost) <= token_budget {
            used_tokens += item.token_cost;
            actions.push(SmartContextRehydrateAction::Rehydrate {
                id: item.id,
                token_cost: item.token_cost,
            });
        } else {
            actions.push(SmartContextRehydrateAction::Defer {
                id: item.id,
                reason: SmartContextRehydrateDeferReason::TokenBudgetExceeded,
            });
        }
    }

    SmartContextRehydratePlan {
        actions,
        used_tokens,
    }
}

pub const SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN: u64 = 4;
pub(super) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_NUMERATOR: u64 = 9;
pub(super) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR: u64 = 8;
pub(super) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS: u64 = 64;
pub(super) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextTokenAccountingSource {
    CurrentRequestTokens,
    CurrentRequestBodyEstimate,
    ObservedHistory,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextTokenAccountingRisk {
    UnknownTokenWindow,
    ZeroContextWindow,
    ReservedOutputConsumesWindow,
    UnknownCurrentRequestAccounting,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextObservedTokenAccountingInput {
    pub model_context_window_tokens: Option<u64>,
    pub reserved_output_tokens: u64,
    pub current_input_tokens: u64,
    pub current_request_body_bytes: usize,
    pub current_request_estimated_tokens: Option<u64>,
    pub observed_usage: Vec<RuntimeTokenUsage>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SmartContextTokenCalibrationBucketKey {
    pub route: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
    pub transport: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTokenCalibrationSample {
    pub bucket_key: Option<SmartContextTokenCalibrationBucketKey>,
    pub usage: RuntimeTokenUsage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextObservedTokenAccountingCalibrationInput {
    pub accounting: SmartContextObservedTokenAccountingInput,
    pub calibration_bucket_key: Option<SmartContextTokenCalibrationBucketKey>,
    pub calibration_samples: Vec<SmartContextTokenCalibrationSample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextObservedTokenAccounting {
    pub model_context_window_tokens: Option<u64>,
    pub observed_turns: usize,
    pub observed_input_tokens: u64,
    pub observed_cached_input_tokens: u64,
    pub observed_uncached_input_tokens: u64,
    pub observed_output_tokens: u64,
    pub observed_reasoning_tokens: u64,
    pub observed_total_tokens: u64,
    pub observed_context_tokens: u64,
    pub last_input_tokens: u64,
    pub last_accounted_input_tokens: u64,
    pub last_observed_context_tokens: u64,
    pub current_request_body_bytes: usize,
    pub estimated_current_request_tokens: u64,
    pub current_request_accounted_tokens: u64,
    pub effective_input_tokens: u64,
    pub effective_input_source: SmartContextTokenAccountingSource,
    pub reserved_output_tokens: u64,
    pub available_context_tokens: Option<u64>,
    pub accounting_risks: Vec<SmartContextTokenAccountingRisk>,
}

pub fn smart_context_observed_token_accounting(
    input: SmartContextObservedTokenAccountingInput,
) -> SmartContextObservedTokenAccounting {
    smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: input,
            calibration_bucket_key: None,
            calibration_samples: Vec::new(),
        },
    )
}

pub fn smart_context_observed_token_accounting_with_calibration(
    input: SmartContextObservedTokenAccountingCalibrationInput,
) -> SmartContextObservedTokenAccounting {
    let SmartContextObservedTokenAccountingCalibrationInput {
        accounting: input,
        calibration_bucket_key,
        calibration_samples,
    } = input;
    let mut observed_input_tokens = 0u64;
    let mut observed_cached_input_tokens = 0u64;
    let mut observed_output_tokens = 0u64;
    let mut observed_reasoning_tokens = 0u64;
    let mut last_input_tokens = 0u64;
    let mut last_accounted_input_tokens = 0u64;
    let mut last_observed_context_tokens = 0u64;

    for usage in &input.observed_usage {
        observed_input_tokens = observed_input_tokens.saturating_add(usage.input_tokens);
        observed_cached_input_tokens =
            observed_cached_input_tokens.saturating_add(usage.cached_input_tokens);
        observed_output_tokens = observed_output_tokens.saturating_add(usage.output_tokens);
        observed_reasoning_tokens =
            observed_reasoning_tokens.saturating_add(usage.reasoning_tokens);
        last_input_tokens = usage.input_tokens;
        last_accounted_input_tokens = smart_context_accounted_input_tokens(*usage).unwrap_or(0);
        last_observed_context_tokens =
            smart_context_observed_usage_context_tokens(*usage).unwrap_or(0);
    }

    let observed_uncached_input_tokens =
        observed_input_tokens.saturating_sub(observed_cached_input_tokens);
    let observed_total_tokens = observed_input_tokens.saturating_add(observed_output_tokens);
    let observed_context_tokens = observed_total_tokens.saturating_add(observed_reasoning_tokens);
    let baseline_estimated_current_request_tokens =
        input.current_request_estimated_tokens.unwrap_or_else(|| {
            smart_context_estimate_tokens_from_body_bytes(input.current_request_body_bytes)
        });
    let estimated_current_request_tokens = smart_context_observed_calibrated_request_estimate(
        input.current_request_body_bytes,
        baseline_estimated_current_request_tokens,
        &input.observed_usage,
        calibration_bucket_key.as_ref(),
        &calibration_samples,
    );
    let current_request_accounted_tokens = input
        .current_input_tokens
        .max(estimated_current_request_tokens);
    let effective_input_tokens = current_request_accounted_tokens.max(last_accounted_input_tokens);
    let effective_input_source = smart_context_effective_input_source(
        input.current_input_tokens,
        estimated_current_request_tokens,
        current_request_accounted_tokens,
        last_accounted_input_tokens,
        effective_input_tokens,
    );
    let available_context_tokens = input.model_context_window_tokens.map(|window| {
        window
            .saturating_sub(effective_input_tokens)
            .saturating_sub(input.reserved_output_tokens)
    });
    let accounting_risks = smart_context_token_accounting_risks(
        input.model_context_window_tokens,
        input.reserved_output_tokens,
        effective_input_source,
    );

    SmartContextObservedTokenAccounting {
        model_context_window_tokens: input.model_context_window_tokens,
        observed_turns: input.observed_usage.len(),
        observed_input_tokens,
        observed_cached_input_tokens,
        observed_uncached_input_tokens,
        observed_output_tokens,
        observed_reasoning_tokens,
        observed_total_tokens,
        observed_context_tokens,
        last_input_tokens,
        last_accounted_input_tokens,
        last_observed_context_tokens,
        current_request_body_bytes: input.current_request_body_bytes,
        estimated_current_request_tokens,
        current_request_accounted_tokens,
        effective_input_tokens,
        effective_input_source,
        reserved_output_tokens: input.reserved_output_tokens,
        available_context_tokens,
        accounting_risks,
    }
}

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: usize) -> u64 {
    let body_bytes = u64::try_from(body_bytes).unwrap_or(u64::MAX);
    body_bytes.saturating_add(SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN - 1)
        / SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN
}

pub(super) fn smart_context_observed_calibrated_request_estimate(
    body_bytes: usize,
    baseline_estimate: u64,
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> u64 {
    if baseline_estimate == 0 {
        return 0;
    }
    let Some(recent_accounted_input) = smart_context_recent_accounted_input_calibration(
        observed_usage,
        calibration_bucket_key,
        calibration_samples,
    ) else {
        return baseline_estimate;
    };
    let raw_floor = smart_context_estimate_tokens_from_body_bytes(body_bytes)
        .saturating_add(1)
        .saturating_div(2)
        .max(1);
    let raw_floor_with_margin =
        raw_floor.saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS);
    let observed_with_margin = recent_accounted_input
        .saturating_mul(SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_NUMERATOR)
        .saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR - 1)
        / SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR;
    let observed_with_margin =
        observed_with_margin.saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS);
    baseline_estimate.min(observed_with_margin.max(raw_floor_with_margin))
}

pub(super) fn smart_context_recent_accounted_input_calibration(
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> Option<u64> {
    if !calibration_samples.is_empty() {
        return smart_context_recent_accounted_input_calibration_for_bucket(
            calibration_bucket_key,
            calibration_samples,
        );
    }

    observed_usage
        .iter()
        .rev()
        .filter_map(|usage| smart_context_accounted_input_tokens(*usage))
        .take(SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT)
        .max()
}

pub(super) fn smart_context_recent_accounted_input_calibration_for_bucket(
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> Option<u64> {
    if let Some(calibration) = smart_context_recent_accounted_input_calibration_matching(
        calibration_samples,
        |sample_bucket_key| sample_bucket_key == calibration_bucket_key,
    ) {
        return Some(calibration);
    }

    let calibration_bucket_key = calibration_bucket_key?;

    for tier in [
        SmartContextTokenCalibrationFallbackTier::Model,
        SmartContextTokenCalibrationFallbackTier::ProfileRoute,
        SmartContextTokenCalibrationFallbackTier::RouteTransportGlobal,
    ] {
        if let Some(calibration) = smart_context_recent_accounted_input_calibration_matching(
            calibration_samples,
            |sample_bucket_key| {
                smart_context_token_calibration_bucket_fallback_matches(
                    calibration_bucket_key,
                    sample_bucket_key,
                    tier,
                )
            },
        ) {
            return Some(calibration);
        }
    }

    None
}

pub(super) fn smart_context_recent_accounted_input_calibration_matching(
    calibration_samples: &[SmartContextTokenCalibrationSample],
    mut bucket_matches: impl FnMut(Option<&SmartContextTokenCalibrationBucketKey>) -> bool,
) -> Option<u64> {
    calibration_samples
        .iter()
        .rev()
        .filter(|sample| bucket_matches(sample.bucket_key.as_ref()))
        .filter_map(|sample| smart_context_accounted_input_tokens(sample.usage))
        .take(SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT)
        .max()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SmartContextTokenCalibrationFallbackTier {
    Model,
    ProfileRoute,
    RouteTransportGlobal,
}

pub(super) fn smart_context_token_calibration_bucket_fallback_matches(
    target: &SmartContextTokenCalibrationBucketKey,
    sample: Option<&SmartContextTokenCalibrationBucketKey>,
    tier: SmartContextTokenCalibrationFallbackTier,
) -> bool {
    match tier {
        SmartContextTokenCalibrationFallbackTier::Model => sample.is_some_and(|sample| {
            smart_context_token_calibration_model_matches(&target.model, &sample.model)
        }),
        SmartContextTokenCalibrationFallbackTier::ProfileRoute => sample.is_some_and(|sample| {
            smart_context_token_calibration_field_matches(&target.profile, &sample.profile)
                && smart_context_token_calibration_field_matches(&target.route, &sample.route)
        }),
        SmartContextTokenCalibrationFallbackTier::RouteTransportGlobal => sample
            .map(|sample| {
                (smart_context_token_calibration_field_matches(&target.route, &sample.route)
                    && smart_context_token_calibration_field_matches(
                        &target.transport,
                        &sample.transport,
                    ))
                    || smart_context_token_calibration_sample_is_global_compatible(target, sample)
            })
            .unwrap_or(true),
    }
}

pub(super) fn smart_context_token_calibration_field_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    target.as_deref().is_some_and(non_empty) && target == sample
}

pub(super) fn smart_context_token_calibration_model_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    let Some(target) = smart_context_token_calibration_normalized_model(target.as_deref()) else {
        return false;
    };
    let Some(sample) = smart_context_token_calibration_normalized_model(sample.as_deref()) else {
        return false;
    };
    target == sample
        || smart_context_token_calibration_model_family(&target)
            == smart_context_token_calibration_model_family(&sample)
}

pub(super) fn smart_context_token_calibration_sample_is_global_compatible(
    target: &SmartContextTokenCalibrationBucketKey,
    sample: &SmartContextTokenCalibrationBucketKey,
) -> bool {
    smart_context_token_calibration_optional_field_compatible(&target.route, &sample.route)
        && smart_context_token_calibration_optional_model_compatible(&target.model, &sample.model)
        && smart_context_token_calibration_optional_field_compatible(
            &target.profile,
            &sample.profile,
        )
        && smart_context_token_calibration_optional_field_compatible(
            &target.transport,
            &sample.transport,
        )
}

pub(super) fn smart_context_token_calibration_optional_field_compatible(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    sample
        .as_deref()
        .is_none_or(|sample| target.as_deref() == Some(sample))
}

pub(super) fn smart_context_token_calibration_optional_model_compatible(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    sample
        .as_ref()
        .is_none_or(|_| smart_context_token_calibration_model_matches(target, sample))
}

pub(super) fn smart_context_token_calibration_normalized_model(
    value: Option<&str>,
) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let mut normalized = String::with_capacity(value.len());
    let mut previous_separator = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        let ch = match ch {
            '_' | ' ' => '-',
            ch => ch,
        };
        if ch == '-' {
            if !previous_separator {
                normalized.push(ch);
                previous_separator = true;
            }
        } else {
            normalized.push(ch);
            previous_separator = false;
        }
    }
    let normalized = normalized.trim_matches('-');
    (!normalized.is_empty()).then(|| normalized.to_string())
}

pub(super) fn smart_context_token_calibration_model_family(model: &str) -> &str {
    let model = model
        .strip_suffix("-latest")
        .or_else(|| model.strip_suffix("-preview"))
        .unwrap_or(model);

    if let Some(family) = model
        .strip_suffix("-mini")
        .or_else(|| model.strip_suffix("-nano"))
        .or_else(|| model.strip_suffix("-codex"))
    {
        return family;
    }

    if model.len() >= 11 {
        let suffix_start = model.len() - 11;
        let suffix = &model[suffix_start..];
        if suffix.as_bytes()[0] == b'-'
            && suffix.as_bytes()[1..5].iter().all(u8::is_ascii_digit)
            && suffix.as_bytes()[5] == b'-'
            && suffix.as_bytes()[6..8].iter().all(u8::is_ascii_digit)
            && suffix.as_bytes()[8] == b'-'
            && suffix.as_bytes()[9..11].iter().all(u8::is_ascii_digit)
        {
            return &model[..suffix_start];
        }
    }

    model
}

pub(super) fn smart_context_accounted_input_tokens(usage: RuntimeTokenUsage) -> Option<u64> {
    let accounted = if usage.input_tokens == 0 {
        usage.cached_input_tokens
    } else {
        usage.input_tokens
    };
    (accounted > 0).then_some(accounted)
}

pub fn smart_context_estimate_tokens_from_body(body: &[u8]) -> u64 {
    let Ok(text) = std::str::from_utf8(body) else {
        return smart_context_estimate_tokens_from_body_bytes(body.len());
    };
    smart_context_estimate_tokens_from_text(text)
        .max(smart_context_estimate_tokens_from_body_bytes(body.len()).saturating_div(2))
}

pub(super) fn smart_context_estimate_tokens_from_text(text: &str) -> u64 {
    let mut tokens = 0u64;
    let mut run = String::new();
    let mut run_kind = SmartContextEstimatorRunKind::Other;
    let mut structural = 0u64;
    let mut separators = 0u64;

    for ch in text.chars() {
        let kind = smart_context_estimator_run_kind(ch);
        if matches!(
            kind,
            SmartContextEstimatorRunKind::Word | SmartContextEstimatorRunKind::Number
        ) {
            if kind != run_kind {
                tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
                run.clear();
                run_kind = kind;
            }
            run.push(ch);
            continue;
        }

        tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
        run.clear();
        run_kind = SmartContextEstimatorRunKind::Other;

        if ch.is_whitespace() {
            if ch == '\n' {
                separators = separators.saturating_add(1);
            }
        } else if matches!(
            ch,
            '{' | '}' | '[' | ']' | ':' | ',' | '"' | '\'' | '`' | '(' | ')' | '<' | '>'
        ) {
            structural = structural.saturating_add(1);
        } else {
            tokens = tokens.saturating_add(1);
        }
    }

    tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
    tokens
        .saturating_add(structural.saturating_add(3) / 4)
        .saturating_add(separators.saturating_add(7) / 8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SmartContextEstimatorRunKind {
    Word,
    Number,
    Other,
}

pub(super) fn smart_context_estimator_run_kind(ch: char) -> SmartContextEstimatorRunKind {
    if ch.is_ascii_alphabetic() || ch == '_' || ch == '-' {
        SmartContextEstimatorRunKind::Word
    } else if ch.is_ascii_digit() {
        SmartContextEstimatorRunKind::Number
    } else {
        SmartContextEstimatorRunKind::Other
    }
}

pub(super) fn smart_context_estimate_run_tokens(
    run: &str,
    kind: SmartContextEstimatorRunKind,
) -> u64 {
    if run.is_empty() {
        return 0;
    }
    let chars = u64::try_from(run.chars().count()).unwrap_or(u64::MAX);
    match kind {
        SmartContextEstimatorRunKind::Word => chars.saturating_add(3) / 4,
        SmartContextEstimatorRunKind::Number => chars.saturating_add(2) / 3,
        SmartContextEstimatorRunKind::Other => chars,
    }
    .max(1)
}

pub fn smart_context_observed_usage_context_tokens(usage: RuntimeTokenUsage) -> Option<u64> {
    let observed = usage
        .input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.reasoning_tokens);
    let observed = if observed == 0 {
        usage.cached_input_tokens
    } else {
        observed
    };
    (observed > 0).then_some(observed)
}

pub fn smart_context_token_budget_tier_from_accounting(
    accounting: &SmartContextObservedTokenAccounting,
) -> SmartContextTokenBudgetTier {
    accounting
        .available_context_tokens
        .map(smart_context_u64_budget_tier)
        .unwrap_or(SmartContextTokenBudgetTier::Exact)
}

pub fn smart_context_accounting_safe_for_adaptive_policy(
    accounting: &SmartContextObservedTokenAccounting,
) -> bool {
    accounting.accounting_risks.is_empty()
}
