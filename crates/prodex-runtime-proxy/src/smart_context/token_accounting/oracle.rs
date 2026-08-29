use super::super::*;
use super::{SmartContextTokenAccountingDecision, observed};
#[cfg(any(not(feature = "mojo"), test))]
use super::{calibration, estimation};

pub(super) fn smart_context_observed_token_accounting_from_decision(
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

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_observed_token_accounting_rust(
    input: SmartContextObservedTokenAccountingCalibrationInput,
) -> SmartContextObservedTokenAccounting {
    let SmartContextObservedTokenAccountingCalibrationInput {
        accounting: input,
        calibration_bucket_key,
        calibration_samples,
    } = input;
    let usage_totals = observed::smart_context_observed_usage_totals_rust(&input.observed_usage);
    let baseline_estimated_current_request_tokens =
        input.current_request_estimated_tokens.unwrap_or_else(|| {
            estimation::smart_context_estimate_tokens_from_body_bytes_rust(
                input.current_request_body_bytes,
            )
        });
    let estimated_current_request_tokens =
        calibration::smart_context_observed_calibrated_request_estimate_rust(
            input.current_request_body_bytes,
            baseline_estimated_current_request_tokens,
            &input.observed_usage,
            calibration_bucket_key.as_ref(),
            &calibration_samples,
        );
    let current_request_accounted_tokens = input
        .current_input_tokens
        .max(estimated_current_request_tokens);
    let effective_input_tokens =
        current_request_accounted_tokens.max(usage_totals.last_accounted_input_tokens);
    let effective_input_source = smart_context_effective_input_source(
        input.current_input_tokens,
        estimated_current_request_tokens,
        current_request_accounted_tokens,
        usage_totals.last_accounted_input_tokens,
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
    let pressure = smart_context_pressure_snapshot_rust(SmartContextPressureSnapshotInput {
        model_context_window_tokens: input.model_context_window_tokens,
        reserved_output_tokens: input.reserved_output_tokens,
        effective_input_tokens,
        available_context_tokens,
        effective_input_source,
        accounting_risks: &accounting_risks,
    });
    smart_context_observed_token_accounting_from_decision(
        input,
        usage_totals,
        estimated_current_request_tokens,
        SmartContextTokenAccountingDecision {
            observed_uncached_input_tokens: usage_totals
                .input_tokens
                .saturating_sub(usage_totals.cached_input_tokens),
            observed_total_tokens: usage_totals
                .input_tokens
                .saturating_add(usage_totals.output_tokens),
            observed_context_tokens: usage_totals
                .input_tokens
                .saturating_add(usage_totals.output_tokens)
                .saturating_add(usage_totals.reasoning_tokens),
            current_request_accounted_tokens,
            effective_input_tokens,
            effective_input_source,
            available_context_tokens,
            accounting_risks,
        },
        pressure,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_pressure_snapshot_rust(
    input: SmartContextPressureSnapshotInput<'_>,
) -> SmartContextPressureSnapshot {
    let effective_usable_context_tokens = input
        .model_context_window_tokens
        .and_then(|window| window.checked_sub(input.reserved_output_tokens));
    let pressure_basis_points = effective_usable_context_tokens.and_then(|usable| {
        (usable > 0).then(|| {
            input
                .effective_input_tokens
                .saturating_mul(10_000)
                .checked_div(usable)
                .unwrap_or(u64::MAX)
                .min(u32::MAX as u64) as u32
        })
    });
    let pressure_band = smart_context_pressure_band(pressure_basis_points);
    let estimator_confidence =
        smart_context_estimator_confidence(input.effective_input_source, input.accounting_risks);

    SmartContextPressureSnapshot {
        model_context_window_tokens: input.model_context_window_tokens,
        reserved_output_tokens: input.reserved_output_tokens,
        effective_usable_context_tokens,
        effective_used_tokens: input.effective_input_tokens,
        pressure_basis_points,
        pressure_band,
        absolute_safety_floor_tokens: smart_context_absolute_safety_floor_tokens(
            input.model_context_window_tokens,
            input.reserved_output_tokens,
        ),
        available_context_tokens: input.available_context_tokens,
        estimator_confidence,
    }
}
