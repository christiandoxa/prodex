use crate::{
    RuntimeProxyQuotaObservationPair, RuntimeProxyQuotaScore, RuntimeProxyQuotaWindowObservation,
    RuntimeResponseCandidatePlanInput, RuntimeResponseCandidatePlanOptions, RuntimeRouteKind,
    RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaSource,
    RuntimeSelectionQuotaWindowStatus,
};

pub(super) fn route_kind_tag(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    }
}

pub(super) fn quota_band_from_tag(
    band: i64,
) -> Result<RuntimeSelectionQuotaPressureBand, prodex_mojo_core::MojoError> {
    match band {
        0 => Ok(RuntimeSelectionQuotaPressureBand::Healthy),
        1 => Ok(RuntimeSelectionQuotaPressureBand::Thin),
        2 => Ok(RuntimeSelectionQuotaPressureBand::Critical),
        3 => Ok(RuntimeSelectionQuotaPressureBand::Exhausted),
        4 => Ok(RuntimeSelectionQuotaPressureBand::Unknown),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub(crate) fn pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> Result<RuntimeSelectionQuotaPressureBand, prodex_mojo_core::MojoError> {
    let five_hour = five_hour.map(|window| (window.remaining_percent, 1));
    let weekly = weekly.map(|window| (window.remaining_percent, 1));
    quota_band_from_tag(prodex_mojo_core::runtime::pressure_band_for_route(
        five_hour,
        weekly,
        route_kind_tag(route_kind),
    )?)
}

pub(crate) fn quota_score_batch(
    observations: &[RuntimeProxyQuotaObservationPair],
    route_kind: RuntimeRouteKind,
) -> Result<Vec<RuntimeProxyQuotaScore>, prodex_mojo_core::MojoError> {
    let inputs = observations
        .iter()
        .map(|(five_hour, weekly)| {
            let five_hour = five_hour.as_ref();
            let weekly = weekly.as_ref();
            prodex_mojo_core::runtime::QuotaScoreInput {
                weekly_pressure: weekly.map_or(i64::MAX, |window| window.pressure_score),
                five_hour_pressure: five_hour.map_or(i64::MAX, |window| window.pressure_score),
                weekly_remaining: weekly.map_or(0, |window| window.remaining_percent),
                five_hour_remaining: five_hour.map_or(0, |window| window.remaining_percent),
                weekly_has_value: weekly.is_some(),
                five_hour_has_value: five_hour.is_some(),
                weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
                five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
            }
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::quota_score_batch(&inputs, route_kind_tag(route_kind))?
        .into_iter()
        .map(|score| {
            Ok(RuntimeProxyQuotaScore {
                pressure_band: quota_band_from_tag(score.pressure_band)?,
                total_pressure: score.total_pressure,
                weekly_pressure: score.weekly_pressure,
                five_hour_pressure: score.five_hour_pressure,
                reserve_floor: score.reserve_floor,
                weekly_remaining: score.weekly_remaining,
                five_hour_remaining: score.five_hour_remaining,
                weekly_reset_at: score.weekly_reset_at,
                five_hour_reset_at: score.five_hour_reset_at,
            })
        })
        .collect()
}

pub(super) fn quota_status_tag(status: RuntimeSelectionQuotaWindowStatus) -> i64 {
    match status {
        RuntimeSelectionQuotaWindowStatus::Ready => 0,
        RuntimeSelectionQuotaWindowStatus::Thin => 1,
        RuntimeSelectionQuotaWindowStatus::Critical => 2,
        RuntimeSelectionQuotaWindowStatus::Exhausted => 3,
        RuntimeSelectionQuotaWindowStatus::Unknown => 4,
    }
}

pub(super) fn quota_source_tag(source: Option<RuntimeSelectionQuotaSource>) -> i64 {
    match source {
        None => -1,
        Some(RuntimeSelectionQuotaSource::LiveProbe) => 0,
        Some(RuntimeSelectionQuotaSource::PersistedSnapshot) => 1,
    }
}

pub(crate) fn runtime_response_candidate_plan_batch(
    candidates: &[RuntimeResponseCandidatePlanInput],
    options: RuntimeResponseCandidatePlanOptions<'_>,
) -> Result<prodex_mojo_core::runtime::RuntimeCandidatePlan, prodex_mojo_core::MojoError> {
    let mut fields = Vec::with_capacity(
        candidates.len() * prodex_mojo_core::runtime::RUNTIME_CANDIDATE_PLAN_FIELD_COUNT,
    );
    let profiles = candidates
        .iter()
        .map(|candidate| candidate.name.as_str())
        .collect::<Vec<_>>();
    let prompt_cache_affinity = crate::runtime_prompt_cache_affinity_batch(
        options.prompt_cache_key,
        options.prompt_cache_owner_profile,
        &profiles,
    )
    .expect("Mojo prompt-cache affinity batch returned invalid output");
    for (candidate, prompt_cache_affinity_sort_key) in candidates.iter().zip(prompt_cache_affinity)
    {
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
        fields.push(quota_source_tag(Some(candidate.quota_source)));
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
        fields.push(i64::from(candidate.auth_failure_active));
        fields.push(quota_status_tag(candidate.quota_summary.five_hour.status));
    }
    let route_kind = route_kind_tag(options.route_kind);
    let excluded = vec![0_i64; candidates.len()];
    prodex_mojo_core::runtime::runtime_candidate_plan_batch(
        &fields,
        &excluded,
        route_kind,
        options.inflight_soft_limit,
        options.responses_critical_floor_percent,
    )
}

fn encode_u64_for_signed_order(value: u64) -> i64 {
    i64::from_ne_bytes((value ^ (1_u64 << 63)).to_ne_bytes())
}
