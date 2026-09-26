#[cfg(feature = "mojo")]
mod calibration;
mod estimation;
#[cfg(feature = "mojo")]
mod observed;

use super::*;
use crate::RuntimeTokenUsage;
#[cfg(feature = "mojo")]
pub(super) use calibration::*;
pub use estimation::{
    SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN, smart_context_estimate_tokens_from_body,
    smart_context_estimate_tokens_from_body_bytes,
};
#[cfg(feature = "mojo")]
use observed::smart_context_observed_usage_totals;
#[cfg(feature = "mojo")]
use std::collections::BTreeSet;

pub fn smart_context_token_budget_tier(available_tokens: usize) -> SmartContextTokenBudgetTier {
    smart_context_u64_budget_tier(u64::try_from(available_tokens).unwrap_or(u64::MAX))
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

#[cfg(all(test, feature = "mojo"))]
pub(crate) fn smart_context_select_memory_capsules_for_policy(
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
    super::normalization::smart_context_memory_capsule_token_budget_impl(accounting, policy)
}

pub fn smart_context_select_memory_capsules(
    capsules: impl IntoIterator<Item = SmartContextMemoryCapsule>,
    token_budget: usize,
) -> SmartContextMemoryCapsuleSelection {
    super::normalization::smart_context_select_memory_capsules_impl(capsules, token_budget)
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

#[cfg(feature = "mojo")]
pub fn smart_context_auto_rehydrate_plan(
    refs: impl IntoIterator<Item = SmartContextRehydrateRef>,
    available_artifact_ids: impl IntoIterator<Item = String>,
    token_budget: usize,
    tier: SmartContextTokenBudgetTier,
) -> SmartContextRehydratePlan {
    let available = available_artifact_ids.into_iter().collect::<BTreeSet<_>>();
    let refs = refs.into_iter().collect::<Vec<_>>();
    smart_context_auto_rehydrate_plan_mojo(&refs, &available, token_budget, tier)
        .expect("Mojo Smart Context rehydration returned invalid output")
}

#[cfg(feature = "mojo")]
fn smart_context_auto_rehydrate_plan_mojo(
    refs: &[SmartContextRehydrateRef],
    available: &BTreeSet<String>,
    token_budget: usize,
    tier: SmartContextTokenBudgetTier,
) -> Result<SmartContextRehydratePlan, prodex_mojo_core::MojoError> {
    let inputs = refs
        .iter()
        .map(|item| {
            Ok(prodex_mojo_core::rich::ContextPlanItem {
                id: &item.id,
                token_cost: item.token_cost,
                required: item.required,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tier = match tier {
        SmartContextTokenBudgetTier::Minimal => 0,
        SmartContextTokenBudgetTier::Condensed => 1,
        SmartContextTokenBudgetTier::Large => 2,
        SmartContextTokenBudgetTier::Exact => 3,
    };
    let available = available.iter().map(String::as_str).collect::<Vec<_>>();
    let plan = prodex_mojo_core::rich::ContextPlanItem::plan_smart_context_rehydrate(
        &inputs,
        &available,
        token_budget,
        tier,
    )?;
    let actions = plan
        .actions
        .into_iter()
        .map(|action| {
            let item = refs
                .get(action.input_index)
                .ok_or(prodex_mojo_core::MojoError::InvalidOutput)?;
            if action.id != item.id || action.token_cost != item.token_cost {
                return Err(prodex_mojo_core::MojoError::InvalidOutput);
            }
            Ok(match (action.action, action.reason) {
                (1, 0) => SmartContextRehydrateAction::Rehydrate {
                    id: item.id.clone(),
                    token_cost: item.token_cost,
                },
                (0, 1) => SmartContextRehydrateAction::Defer {
                    id: item.id.clone(),
                    reason: SmartContextRehydrateDeferReason::MissingArtifact,
                },
                (0, 3) => SmartContextRehydrateAction::Defer {
                    id: item.id.clone(),
                    reason: SmartContextRehydrateDeferReason::MinimalBudgetTier,
                },
                (0, 2) => SmartContextRehydrateAction::Defer {
                    id: item.id.clone(),
                    reason: SmartContextRehydrateDeferReason::TokenBudgetExceeded,
                },
                _ => return Err(prodex_mojo_core::MojoError::InvalidOutput),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmartContextRehydratePlan {
        actions,
        used_tokens: plan.used_tokens,
    })
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextPressureBand {
    Unknown,
    Low,
    Moderate,
    High,
    Critical,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextEstimatorConfidence {
    High,
    Medium,
    Low,
}

#[cfg(feature = "mojo")]
fn smart_context_token_accounting_risks_from_bits(
    bits: u64,
) -> Vec<SmartContextTokenAccountingRisk> {
    [
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_RISK_UNKNOWN_WINDOW,
            SmartContextTokenAccountingRisk::UnknownTokenWindow,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_RISK_ZERO_WINDOW,
            SmartContextTokenAccountingRisk::ZeroContextWindow,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_RISK_RESERVED_OUTPUT,
            SmartContextTokenAccountingRisk::ReservedOutputConsumesWindow,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_RISK_UNKNOWN_INPUT,
            SmartContextTokenAccountingRisk::UnknownCurrentRequestAccounting,
        ),
    ]
    .into_iter()
    .filter_map(|(bit, risk)| (bits & bit != 0).then_some(risk))
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextPressureSnapshot {
    pub model_context_window_tokens: Option<u64>,
    pub reserved_output_tokens: u64,
    pub effective_usable_context_tokens: Option<u64>,
    pub effective_used_tokens: u64,
    pub pressure_basis_points: Option<u32>,
    pub pressure_band: SmartContextPressureBand,
    pub absolute_safety_floor_tokens: u64,
    pub available_context_tokens: Option<u64>,
    pub estimator_confidence: SmartContextEstimatorConfidence,
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
    pub pressure: SmartContextPressureSnapshot,
}

#[cfg(feature = "mojo")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct SmartContextTokenAccountingDecision {
    observed_uncached_input_tokens: u64,
    observed_total_tokens: u64,
    observed_context_tokens: u64,
    current_request_accounted_tokens: u64,
    effective_input_tokens: u64,
    effective_input_source: SmartContextTokenAccountingSource,
    available_context_tokens: Option<u64>,
    accounting_risks: Vec<SmartContextTokenAccountingRisk>,
}

#[cfg(all(test, feature = "mojo"))]
pub(crate) fn smart_context_observed_token_accounting(
    input: SmartContextObservedTokenAccountingInput,
) -> SmartContextObservedTokenAccounting {
    smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: input,
            calibration_bucket_key: None,
            calibration_samples: Vec::new(),
        },
    )
    .expect("Smart Context accounting requires Mojo")
}

#[cfg(feature = "mojo")]
fn smart_context_observed_token_accounting_from_decision(
    input: SmartContextObservedTokenAccountingInput,
    usage_totals: observed::SmartContextObservedUsageTotals,
    estimated_current_request_tokens: u64,
    decision: SmartContextTokenAccountingDecision,
    pressure: SmartContextPressureSnapshot,
) -> SmartContextObservedTokenAccounting {
    SmartContextObservedTokenAccounting {
        model_context_window_tokens: input.model_context_window_tokens,
        observed_turns: input.observed_usage.len(),
        observed_input_tokens: usage_totals.input_tokens,
        observed_cached_input_tokens: usage_totals.cached_input_tokens,
        observed_uncached_input_tokens: decision.observed_uncached_input_tokens,
        observed_output_tokens: usage_totals.output_tokens,
        observed_reasoning_tokens: usage_totals.reasoning_tokens,
        observed_total_tokens: decision.observed_total_tokens,
        observed_context_tokens: decision.observed_context_tokens,
        last_input_tokens: usage_totals.last_input_tokens,
        last_accounted_input_tokens: usage_totals.last_accounted_input_tokens,
        last_observed_context_tokens: usage_totals.last_observed_context_tokens,
        current_request_body_bytes: input.current_request_body_bytes,
        estimated_current_request_tokens,
        current_request_accounted_tokens: decision.current_request_accounted_tokens,
        effective_input_tokens: decision.effective_input_tokens,
        effective_input_source: decision.effective_input_source,
        reserved_output_tokens: input.reserved_output_tokens,
        available_context_tokens: decision.available_context_tokens,
        accounting_risks: decision.accounting_risks,
        pressure,
    }
}

pub fn smart_context_observed_token_accounting_with_calibration(
    input: SmartContextObservedTokenAccountingCalibrationInput,
) -> Option<SmartContextObservedTokenAccounting> {
    #[cfg(feature = "mojo")]
    {
        let SmartContextObservedTokenAccountingCalibrationInput {
            accounting: input,
            calibration_bucket_key,
            calibration_samples,
        } = input;
        let usage_totals = smart_context_observed_usage_totals(&input.observed_usage);
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
        let decision = prodex_mojo_core::runtime::smart_context_token_accounting(
            prodex_mojo_core::runtime::SmartContextTokenAccountingInput {
                model_context_window_tokens: input.model_context_window_tokens,
                reserved_output_tokens: input.reserved_output_tokens,
                current_input_tokens: input.current_input_tokens,
                estimated_current_request_tokens,
                observed_input_tokens: usage_totals.input_tokens,
                observed_cached_input_tokens: usage_totals.cached_input_tokens,
                observed_output_tokens: usage_totals.output_tokens,
                observed_reasoning_tokens: usage_totals.reasoning_tokens,
                last_accounted_input_tokens: usage_totals.last_accounted_input_tokens,
            },
        )
        .expect("Mojo Smart Context token accounting returned invalid output");
        let effective_input_source = match decision.effective_input_source {
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_SOURCE_CURRENT_TOKENS => {
                SmartContextTokenAccountingSource::CurrentRequestTokens
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_SOURCE_BODY_ESTIMATE => {
                SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_SOURCE_OBSERVED_HISTORY => {
                SmartContextTokenAccountingSource::ObservedHistory
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_ACCOUNTING_SOURCE_UNKNOWN => {
                SmartContextTokenAccountingSource::Unknown
            }
            _ => unreachable!("Mojo Smart Context token accounting source was validated"),
        };
        let accounting_risks = smart_context_token_accounting_risks_from_bits(decision.risk_bits);
        let pressure = smart_context_pressure_snapshot(SmartContextPressureSnapshotInput {
            model_context_window_tokens: input.model_context_window_tokens,
            reserved_output_tokens: input.reserved_output_tokens,
            effective_input_tokens: decision.effective_input_tokens,
            available_context_tokens: decision.available_context_tokens,
            effective_input_source,
            accounting_risks: &accounting_risks,
        });

        Some(smart_context_observed_token_accounting_from_decision(
            input,
            usage_totals,
            estimated_current_request_tokens,
            SmartContextTokenAccountingDecision {
                observed_uncached_input_tokens: decision.observed_uncached_input_tokens,
                observed_total_tokens: decision.observed_total_tokens,
                observed_context_tokens: decision.observed_context_tokens,
                current_request_accounted_tokens: decision.current_request_accounted_tokens,
                effective_input_tokens: decision.effective_input_tokens,
                effective_input_source,
                available_context_tokens: decision.available_context_tokens,
                accounting_risks,
            },
            pressure,
        ))
    }

    #[cfg(not(feature = "mojo"))]
    {
        let _ = input;
        None
    }
}

#[cfg(feature = "mojo")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextPressureSnapshotInput<'a> {
    pub model_context_window_tokens: Option<u64>,
    pub reserved_output_tokens: u64,
    pub effective_input_tokens: u64,
    pub available_context_tokens: Option<u64>,
    pub effective_input_source: SmartContextTokenAccountingSource,
    pub accounting_risks: &'a [SmartContextTokenAccountingRisk],
}

#[cfg(feature = "mojo")]
pub fn smart_context_pressure_snapshot(
    input: SmartContextPressureSnapshotInput<'_>,
) -> SmartContextPressureSnapshot {
    let snapshot = crate::quota::mojo::smart_context_pressure_snapshot(
        input.model_context_window_tokens,
        input.reserved_output_tokens,
        input.effective_input_tokens,
        match input.effective_input_source {
            SmartContextTokenAccountingSource::CurrentRequestTokens => 0,
            SmartContextTokenAccountingSource::CurrentRequestBodyEstimate => 1,
            SmartContextTokenAccountingSource::ObservedHistory => 2,
            SmartContextTokenAccountingSource::Unknown => 3,
        },
        input
            .accounting_risks
            .contains(&SmartContextTokenAccountingRisk::UnknownTokenWindow),
        input
            .accounting_risks
            .contains(&SmartContextTokenAccountingRisk::ZeroContextWindow),
        input
            .accounting_risks
            .contains(&SmartContextTokenAccountingRisk::ReservedOutputConsumesWindow),
    )
    .expect("Mojo Smart Context pressure snapshot returned invalid output");
    let pressure_band = match snapshot.pressure_band {
        0 => Some(SmartContextPressureBand::Unknown),
        1 => Some(SmartContextPressureBand::Low),
        2 => Some(SmartContextPressureBand::Moderate),
        3 => Some(SmartContextPressureBand::High),
        4 => Some(SmartContextPressureBand::Critical),
        5 => Some(SmartContextPressureBand::Exhausted),
        _ => None,
    };
    let estimator_confidence = match snapshot.estimator_confidence {
        0 => Some(SmartContextEstimatorConfidence::High),
        1 => Some(SmartContextEstimatorConfidence::Medium),
        2 => Some(SmartContextEstimatorConfidence::Low),
        _ => None,
    };
    let pressure_band = pressure_band.expect("Mojo Smart Context pressure band is invalid");
    let estimator_confidence =
        estimator_confidence.expect("Mojo Smart Context estimator confidence is invalid");
    SmartContextPressureSnapshot {
        model_context_window_tokens: input.model_context_window_tokens,
        reserved_output_tokens: input.reserved_output_tokens,
        effective_usable_context_tokens: snapshot.effective_usable_context_tokens,
        effective_used_tokens: snapshot.effective_used_tokens,
        pressure_basis_points: snapshot.pressure_basis_points,
        pressure_band,
        absolute_safety_floor_tokens: snapshot.absolute_safety_floor_tokens,
        available_context_tokens: input.available_context_tokens,
        estimator_confidence,
    }
}

pub fn smart_context_observed_usage_context_tokens(usage: RuntimeTokenUsage) -> Option<u64> {
    #[cfg(feature = "mojo")]
    {
        let summary = crate::quota::mojo::smart_context_token_usage_summary(&[usage])
            .expect("Mojo Smart Context usage summary returned invalid output");
        (summary.last_observed_context_tokens > 0).then_some(summary.last_observed_context_tokens)
    }

    #[cfg(not(feature = "mojo"))]
    {
        let _ = usage;
        None
    }
}

pub fn smart_context_accounting_safe_for_adaptive_policy(
    accounting: &SmartContextObservedTokenAccounting,
) -> bool {
    accounting.accounting_risks.is_empty()
}

#[cfg(all(test, not(feature = "mojo")))]
#[test]
fn observed_token_accounting_is_unavailable_without_mojo() {
    assert_eq!(
        smart_context_observed_usage_context_tokens(RuntimeTokenUsage {
            input_tokens: 1,
            ..Default::default()
        }),
        None
    );
    assert!(
        smart_context_observed_token_accounting_with_calibration(
            SmartContextObservedTokenAccountingCalibrationInput::default()
        )
        .is_none()
    );
}
