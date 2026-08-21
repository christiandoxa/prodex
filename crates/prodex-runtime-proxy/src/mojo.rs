use crate::{
    RuntimeProxyQuotaProfileScore, RuntimeProxyQuotaProfileScoreInput,
    RuntimeProxyQuotaWindowObservation, RuntimeResponseCandidatePlanInput,
    RuntimeResponseCandidatePlanOptions, RuntimeRouteKind, RuntimeSelectionQuotaPressureBand,
    RuntimeSelectionQuotaWindowStatus,
};

pub(crate) fn pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> Result<RuntimeSelectionQuotaPressureBand, prodex_mojo_core::MojoError> {
    let five_hour = five_hour.map(|window| (window.remaining_percent, 1));
    let weekly = weekly.map(|window| (window.remaining_percent, 1));
    let route_kind = match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    };
    match prodex_mojo_core::runtime::pressure_band_for_route(five_hour, weekly, route_kind)? {
        0 => Ok(RuntimeSelectionQuotaPressureBand::Healthy),
        1 => Ok(RuntimeSelectionQuotaPressureBand::Thin),
        2 => Ok(RuntimeSelectionQuotaPressureBand::Critical),
        3 => Ok(RuntimeSelectionQuotaPressureBand::Exhausted),
        4 => Ok(RuntimeSelectionQuotaPressureBand::Unknown),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub(crate) fn window_status(
    remaining_percent: i64,
) -> Result<RuntimeSelectionQuotaWindowStatus, prodex_mojo_core::MojoError> {
    match prodex_mojo_core::quota::window_status(remaining_percent, true) {
        0 => Ok(RuntimeSelectionQuotaWindowStatus::Ready),
        1 => Ok(RuntimeSelectionQuotaWindowStatus::Thin),
        2 => Ok(RuntimeSelectionQuotaWindowStatus::Critical),
        3 => Ok(RuntimeSelectionQuotaWindowStatus::Exhausted),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub(crate) fn pressure_band_from_window_status(
    status: RuntimeSelectionQuotaWindowStatus,
) -> Result<RuntimeSelectionQuotaPressureBand, prodex_mojo_core::MojoError> {
    let code = match status {
        RuntimeSelectionQuotaWindowStatus::Ready => 0,
        RuntimeSelectionQuotaWindowStatus::Thin => 1,
        RuntimeSelectionQuotaWindowStatus::Critical => 2,
        RuntimeSelectionQuotaWindowStatus::Exhausted => 3,
        RuntimeSelectionQuotaWindowStatus::Unknown => 4,
    };
    match prodex_mojo_core::quota::pressure_band(code, code) {
        0 => Ok(RuntimeSelectionQuotaPressureBand::Healthy),
        1 => Ok(RuntimeSelectionQuotaPressureBand::Thin),
        2 => Ok(RuntimeSelectionQuotaPressureBand::Critical),
        3 => Ok(RuntimeSelectionQuotaPressureBand::Exhausted),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub(crate) fn profile_scores_batch(
    inputs: &[RuntimeProxyQuotaProfileScoreInput],
) -> Vec<RuntimeProxyQuotaProfileScore> {
    let inputs = inputs
        .iter()
        .map(|input| prodex_mojo_core::runtime::ProfileScoreInput {
            weekly_pressure: input.weekly_pressure,
            five_hour_pressure: input.five_hour_pressure,
            scale_bps: input.scale_bps,
            weekly_remaining: input.weekly_remaining,
            five_hour_remaining: input.five_hour_remaining,
            reserve_bias: input.reserve_bias,
            weekly_weight: input.weekly_weight,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::profile_scores_batch(&inputs)
        .expect("Mojo runtime quota profile score returned invalid output")
        .into_iter()
        .map(|score| RuntimeProxyQuotaProfileScore {
            total_pressure: score.total_pressure,
            weekly_pressure: score.weekly_pressure,
            five_hour_pressure: score.five_hour_pressure,
            reserve_floor: score.reserve_floor,
        })
        .collect()
}

pub(crate) fn smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64 {
    prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body_bytes(body_bytes)
}

pub(crate) fn smart_context_pressure_snapshot(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_tokens: u64,
    effective_input_source: i64,
    unknown_token_window: bool,
    zero_context_window: bool,
    reserved_output_consumes_window: bool,
) -> Result<prodex_mojo_core::runtime::SmartContextPressureSnapshot, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::smart_context_pressure_snapshot(
        model_context_window_tokens,
        reserved_output_tokens,
        effective_input_tokens,
        effective_input_source,
        unknown_token_window,
        zero_context_window,
        reserved_output_consumes_window,
    )
}

pub(crate) fn runtime_response_candidate_plan_batch(
    candidates: &[RuntimeResponseCandidatePlanInput],
    options: RuntimeResponseCandidatePlanOptions<'_>,
) -> Result<prodex_mojo_core::runtime::RuntimeCandidatePlan, prodex_mojo_core::MojoError> {
    let mut fields = Vec::with_capacity(
        candidates.len() * prodex_mojo_core::runtime::RUNTIME_CANDIDATE_PLAN_FIELD_COUNT,
    );
    for candidate in candidates {
        let prompt_cache_affinity_sort_key =
            crate::runtime_prompt_cache_affinity_sort_key_with_owner(
                options.prompt_cache_key,
                options.prompt_cache_owner_profile,
                &candidate.name,
            );
        let push_usize = |fields: &mut Vec<i64>, value: usize| {
            fields
                .push(i64::try_from(value).map_err(|_| prodex_mojo_core::MojoError::InvalidInput)?);
            Ok::<(), prodex_mojo_core::MojoError>(())
        };
        fields.push(if candidate.in_selection_backoff { 1 } else { 0 });
        push_usize(&mut fields, candidate.provider_priority)?;
        fields.push(i64::from(candidate.quota_sort_key.0));
        fields.push(candidate.quota_sort_key.1);
        fields.push(candidate.quota_sort_key.2);
        fields.push(candidate.quota_sort_key.3);
        fields.push(candidate.quota_sort_key.4.0);
        fields.push(candidate.quota_sort_key.5.0);
        fields.push(candidate.quota_sort_key.6.0);
        fields.push(candidate.quota_sort_key.7);
        fields.push(candidate.quota_sort_key.8);
        fields.push(match candidate.quota_source {
            crate::RuntimeSelectionQuotaSource::LiveProbe => 0,
            crate::RuntimeSelectionQuotaSource::PersistedSnapshot => 1,
        });
        push_usize(&mut fields, candidate.inflight_count)?;
        fields.push(i64::from(candidate.health_sort_key));
        fields.push(i64::from(prompt_cache_affinity_sort_key.0));
        fields.push(encode_u64_for_signed_order(
            prompt_cache_affinity_sort_key.1,
        ));
        push_usize(&mut fields, candidate.order_index)?;
        fields.push(encode_u64_for_signed_order(candidate.jitter));
        push_usize(&mut fields, candidate.backoff_sort_key.0)?;
        fields.push(candidate.backoff_sort_key.1);
        fields.push(candidate.backoff_sort_key.2);
        fields.push(candidate.backoff_sort_key.3);
    }
    let route_kind = match options.route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    };
    prodex_mojo_core::runtime::runtime_candidate_plan_batch(&fields, route_kind)
}

fn encode_u64_for_signed_order(value: u64) -> i64 {
    i64::from_ne_bytes((value ^ (1_u64 << 63)).to_ne_bytes())
}
