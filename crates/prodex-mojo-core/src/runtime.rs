use std::cmp::min;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileScoreInput {
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub scale_bps: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub reserve_bias: i64,
    pub weekly_weight: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileScore {
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
}

#[cfg(not(prodex_mojo_fallback))]
unsafe extern "C" {
    fn prodex_runtime_quota_pressure_band_for_route(
        five_hour_remaining_percent: i64,
        five_hour_has_value: i64,
        weekly_remaining_percent: i64,
        weekly_has_value: i64,
        route_kind: i64,
    ) -> i64;
    fn prodex_runtime_quota_profile_score_batch(
        weekly_pressure: *const i64,
        five_hour_pressure: *const i64,
        scale_bps: *const i64,
        weekly_remaining: *const i64,
        five_hour_remaining: *const i64,
        reserve_bias: *const i64,
        weekly_weight: *const i64,
        total_pressure: *mut i64,
        scaled_weekly_pressure: *mut i64,
        scaled_five_hour_pressure: *mut i64,
        reserve_floor: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64;
}

pub fn pressure_band_for_route(
    five_hour: Option<(i64, i64)>,
    weekly: Option<(i64, i64)>,
    route_kind: i64,
) -> i64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        let (five_hour_remaining_percent, five_hour_has_value) = five_hour.unwrap_or((0, 0));
        let (weekly_remaining_percent, weekly_has_value) = weekly.unwrap_or((0, 0));
        unsafe {
            prodex_runtime_quota_pressure_band_for_route(
                five_hour_remaining_percent,
                five_hour_has_value,
                weekly_remaining_percent,
                weekly_has_value,
                route_kind,
            )
        }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        if five_hour.is_none() && weekly.is_none() {
            return 4;
        }
        if five_hour.is_some_and(|(remaining, _)| remaining == 0)
            || weekly.is_some_and(|(remaining, _)| remaining == 0)
        {
            return 3;
        }
        let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) =
            if route_kind == 0 || route_kind == 2 {
                (20, 10, 10, 5)
            } else {
                (10, 5, 5, 3)
            };
        let band = |value: Option<(i64, i64)>, thin: i64, critical: i64| match value {
            None => 0,
            Some((remaining, _)) if remaining <= critical => 2,
            Some((remaining, _)) if remaining <= thin => 1,
            Some(_) => 0,
        };
        band(weekly, thin_weekly, critical_weekly).max(band(
            five_hour,
            thin_five_hour,
            critical_five_hour,
        ))
    }
}

pub fn profile_scores_batch(inputs: &[ProfileScoreInput]) -> Vec<ProfileScore> {
    #[cfg(not(prodex_mojo_fallback))]
    {
        let weekly_pressure = inputs
            .iter()
            .map(|input| input.weekly_pressure)
            .collect::<Vec<_>>();
        let five_hour_pressure = inputs
            .iter()
            .map(|input| input.five_hour_pressure)
            .collect::<Vec<_>>();
        let scale_bps = inputs
            .iter()
            .map(|input| input.scale_bps)
            .collect::<Vec<_>>();
        let weekly_remaining = inputs
            .iter()
            .map(|input| input.weekly_remaining)
            .collect::<Vec<_>>();
        let five_hour_remaining = inputs
            .iter()
            .map(|input| input.five_hour_remaining)
            .collect::<Vec<_>>();
        let reserve_bias = inputs
            .iter()
            .map(|input| input.reserve_bias)
            .collect::<Vec<_>>();
        let weekly_weight = inputs
            .iter()
            .map(|input| input.weekly_weight)
            .collect::<Vec<_>>();
        let mut total_pressure = vec![0_i64; inputs.len()];
        let mut scaled_weekly_pressure = vec![0_i64; inputs.len()];
        let mut scaled_five_hour_pressure = vec![0_i64; inputs.len()];
        let mut reserve_floor = vec![0_i64; inputs.len()];
        let status = unsafe {
            prodex_runtime_quota_profile_score_batch(
                weekly_pressure.as_ptr(),
                five_hour_pressure.as_ptr(),
                scale_bps.as_ptr(),
                weekly_remaining.as_ptr(),
                five_hour_remaining.as_ptr(),
                reserve_bias.as_ptr(),
                weekly_weight.as_ptr(),
                total_pressure.as_mut_ptr(),
                scaled_weekly_pressure.as_mut_ptr(),
                scaled_five_hour_pressure.as_mut_ptr(),
                reserve_floor.as_mut_ptr(),
                i64::try_from(inputs.len()).unwrap_or(i64::MAX),
            )
        };
        if status == 0 {
            return inputs
                .iter()
                .enumerate()
                .map(|(index, _)| ProfileScore {
                    total_pressure: total_pressure[index],
                    weekly_pressure: scaled_weekly_pressure[index],
                    five_hour_pressure: scaled_five_hour_pressure[index],
                    reserve_floor: reserve_floor[index],
                })
                .collect();
        }
    }

    inputs.iter().copied().map(profile_score_rust).collect()
}

fn profile_score_rust(input: ProfileScoreInput) -> ProfileScore {
    let scale = |pressure: i64| {
        if pressure == i64::MAX {
            return i64::MAX;
        }
        pressure
            .saturating_mul(input.scale_bps.max(0))
            .checked_div(10_000)
            .unwrap_or(i64::MAX)
    };
    let weekly_pressure = scale(input.weekly_pressure);
    let five_hour_pressure = scale(input.five_hour_pressure);
    ProfileScore {
        total_pressure: input
            .reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(input.weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: min(input.weekly_remaining, input.five_hour_remaining),
    }
}

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: u64) -> u64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        unsafe { prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes) }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        body_bytes.saturating_add(3) / 4
    }
}
