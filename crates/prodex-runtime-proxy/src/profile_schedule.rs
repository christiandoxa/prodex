use super::RuntimeProxyQuotaProfileScoreInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyQuotaProfileScheduleInput {
    pub score: RuntimeProxyQuotaProfileScoreInput,
    pub provider_priority: i64,
    pub in_selection_cooldown: bool,
    pub last_selected_at: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
    pub quota_source: i64,
    pub preferred: bool,
    pub affinity_preferred: bool,
    pub order_index: i64,
}

pub fn runtime_proxy_profile_schedule_batch(
    inputs: &[RuntimeProxyQuotaProfileScheduleInput],
) -> Result<Vec<usize>, prodex_mojo_core::MojoError> {
    let inputs = inputs
        .iter()
        .map(|input| prodex_mojo_core::runtime::ProfileScheduleInput {
            score: prodex_mojo_core::runtime::ProfileScoreInput {
                weekly_pressure: input.score.weekly_pressure,
                five_hour_pressure: input.score.five_hour_pressure,
                scale_bps: input.score.scale_bps,
                weekly_remaining: input.score.weekly_remaining,
                five_hour_remaining: input.score.five_hour_remaining,
                reserve_bias: input.score.reserve_bias,
                weekly_weight: input.score.weekly_weight,
            },
            provider_priority: input.provider_priority,
            in_selection_cooldown: input.in_selection_cooldown,
            last_selected_at: input.last_selected_at,
            weekly_reset_at: input.weekly_reset_at,
            five_hour_reset_at: input.five_hour_reset_at,
            quota_source: input.quota_source,
            preferred: input.preferred,
            affinity_preferred: input.affinity_preferred,
            order_index: input.order_index,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::profile_schedule_batch(&inputs)
}
