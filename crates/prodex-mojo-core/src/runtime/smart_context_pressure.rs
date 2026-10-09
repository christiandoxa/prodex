use super::{SmartContextPressureSnapshot, prodex_smart_context_pressure_snapshot};

pub fn smart_context_pressure_snapshot(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_tokens: u64,
    effective_input_source: i64,
    unknown_token_window: bool,
    zero_context_window: bool,
    reserved_output_consumes_window: bool,
) -> Result<SmartContextPressureSnapshot, crate::MojoError> {
    if !(0..=3).contains(&effective_input_source) {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut effective_usable_context_tokens = 0;
    let mut effective_usable_has_value = 0;
    let mut pressure_basis_points = 0;
    let mut pressure_has_value = 0;
    let mut pressure_band = 0;
    let mut absolute_safety_floor_tokens = 0;
    let mut estimator_confidence = 0;
    let status = unsafe {
        prodex_smart_context_pressure_snapshot(
            model_context_window_tokens.unwrap_or(0),
            i64::from(model_context_window_tokens.is_some()),
            reserved_output_tokens,
            effective_input_tokens,
            effective_input_source,
            i64::from(unknown_token_window),
            i64::from(zero_context_window),
            i64::from(reserved_output_consumes_window),
            &mut effective_usable_context_tokens,
            &mut effective_usable_has_value,
            &mut pressure_basis_points,
            &mut pressure_has_value,
            &mut pressure_band,
            &mut absolute_safety_floor_tokens,
            &mut estimator_confidence,
        )
    };
    if status != 0
        || !matches!(effective_usable_has_value, 0 | 1)
        || !matches!(pressure_has_value, 0 | 1)
        || !matches!(pressure_band, 0..=5)
        || !matches!(estimator_confidence, 0..=2)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextPressureSnapshot {
        effective_usable_context_tokens: (effective_usable_has_value == 1)
            .then_some(effective_usable_context_tokens),
        effective_used_tokens: effective_input_tokens,
        pressure_basis_points: (pressure_has_value == 1)
            .then_some(pressure_basis_points.min(u64::from(u32::MAX)) as u32),
        pressure_band,
        absolute_safety_floor_tokens,
        estimator_confidence,
    })
}
