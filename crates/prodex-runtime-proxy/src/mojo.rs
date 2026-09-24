use crate::{
    RuntimeProxyQuotaObservationPair, RuntimeProxyQuotaScore, RuntimeProxyQuotaSummary,
    RuntimeProxyQuotaWindowObservation, RuntimeProxyQuotaWindowSummary, RuntimeProxyUsageSnapshot,
    RuntimeResponseCandidatePlanInput, RuntimeResponseCandidatePlanOptions, RuntimeRouteKind,
    RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaSource,
    RuntimeSelectionQuotaWindowStatus, RuntimeTokenUsage,
};

pub(crate) struct RuntimeQuotaSnapshotDecision {
    pub summary: RuntimeProxyQuotaSummary,
    pub hold_active: bool,
    pub hold_expired: bool,
    pub usable: bool,
}

fn route_kind_tag(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    }
}

fn quota_status_tag(status: RuntimeSelectionQuotaWindowStatus) -> i64 {
    match status {
        RuntimeSelectionQuotaWindowStatus::Ready => 0,
        RuntimeSelectionQuotaWindowStatus::Thin => 1,
        RuntimeSelectionQuotaWindowStatus::Critical => 2,
        RuntimeSelectionQuotaWindowStatus::Exhausted => 3,
        RuntimeSelectionQuotaWindowStatus::Unknown => 4,
    }
}

fn quota_status_from_tag(
    status: i64,
) -> Result<RuntimeSelectionQuotaWindowStatus, prodex_mojo_core::MojoError> {
    match status {
        0 => Ok(RuntimeSelectionQuotaWindowStatus::Ready),
        1 => Ok(RuntimeSelectionQuotaWindowStatus::Thin),
        2 => Ok(RuntimeSelectionQuotaWindowStatus::Critical),
        3 => Ok(RuntimeSelectionQuotaWindowStatus::Exhausted),
        4 => Ok(RuntimeSelectionQuotaWindowStatus::Unknown),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

fn quota_band_from_tag(
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

fn quota_source_tag(source: Option<RuntimeSelectionQuotaSource>) -> i64 {
    match source {
        None => -1,
        Some(RuntimeSelectionQuotaSource::LiveProbe) => 0,
        Some(RuntimeSelectionQuotaSource::PersistedSnapshot) => 1,
    }
}

pub(crate) fn quota_snapshot_plan(
    snapshot: RuntimeProxyUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
    stale_grace_seconds: i64,
) -> Result<RuntimeQuotaSnapshotDecision, prodex_mojo_core::MojoError> {
    let plan = prodex_mojo_core::runtime::quota_snapshot_plan(
        prodex_mojo_core::runtime::QuotaSnapshotPlanInput {
            five_hour_status: quota_status_tag(snapshot.five_hour_status),
            five_hour_remaining: snapshot.five_hour_remaining_percent,
            five_hour_reset_at: snapshot.five_hour_reset_at,
            weekly_status: quota_status_tag(snapshot.weekly_status),
            weekly_remaining: snapshot.weekly_remaining_percent,
            weekly_reset_at: snapshot.weekly_reset_at,
            route_kind: route_kind_tag(route_kind),
            checked_at: snapshot.checked_at,
            now,
            stale_grace_seconds,
        },
    )?;
    Ok(RuntimeQuotaSnapshotDecision {
        summary: RuntimeProxyQuotaSummary {
            five_hour: RuntimeProxyQuotaWindowSummary {
                status: quota_status_from_tag(plan.five_hour_status)?,
                remaining_percent: plan.five_hour_remaining,
                reset_at: plan.five_hour_reset_at,
            },
            weekly: RuntimeProxyQuotaWindowSummary {
                status: quota_status_from_tag(plan.weekly_status)?,
                remaining_percent: plan.weekly_remaining,
                reset_at: plan.weekly_reset_at,
            },
            route_band: quota_band_from_tag(plan.route_band)?,
        },
        hold_active: plan.hold_active,
        hold_expired: plan.hold_expired,
        usable: plan.usable,
    })
}

pub(crate) fn quota_gate_plan(
    summary: RuntimeProxyQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    has_continuation_context: bool,
    has_alternative_quota_profile: bool,
) -> Result<prodex_mojo_core::runtime::QuotaGatePlan, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::quota_gate_plan(prodex_mojo_core::runtime::QuotaGatePlanInput {
        five_hour_status: quota_status_tag(summary.five_hour.status),
        five_hour_reset_at: summary.five_hour.reset_at,
        weekly_status: quota_status_tag(summary.weekly.status),
        weekly_reset_at: summary.weekly.reset_at,
        route_kind: route_kind_tag(route_kind),
        source: quota_source_tag(source),
        has_continuation_context,
        has_alternative_quota_profile,
    })
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

pub(crate) fn window_status(
    remaining_percent: i64,
) -> Result<RuntimeSelectionQuotaWindowStatus, prodex_mojo_core::MojoError> {
    let status = prodex_mojo_core::quota::window_status(remaining_percent, true);
    if status == 4 {
        return Err(prodex_mojo_core::MojoError::InvalidOutput);
    }
    quota_status_from_tag(status)
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

pub(crate) fn smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64 {
    prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body_bytes(body_bytes)
}

pub(crate) fn smart_context_estimate_tokens_from_body(
    body: &[u8],
) -> Result<u64, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body(body)
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

pub(crate) fn smart_context_token_usage_summary(
    usages: &[RuntimeTokenUsage],
) -> Result<prodex_mojo_core::runtime::SmartContextTokenUsageSummary, prodex_mojo_core::MojoError> {
    let inputs = usages
        .iter()
        .map(
            |usage| prodex_mojo_core::runtime::SmartContextTokenUsageInput {
                input_tokens: usage.input_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                output_tokens: usage.output_tokens,
                reasoning_tokens: usage.reasoning_tokens,
            },
        )
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::smart_context_token_usage_summary_batch(&inputs)
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
