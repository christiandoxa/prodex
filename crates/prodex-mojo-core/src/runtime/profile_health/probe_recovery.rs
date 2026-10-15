//! The host supplies causal authentication and quota observations; compiled
//! Mojo decides which superseded local backoffs may be retired.
use crate::MojoError;

#[derive(Clone, Copy, Debug, Default)]
pub struct ProfileProbeRecoveryInput {
    pub authenticated_success: bool,
    pub same_credentials: bool,
    pub probe_started_at: i64,
    pub now: i64,
    pub auth_failure: Option<(u32, i64)>,
    pub quota_ready: bool,
    pub previous_checked_at: Option<i64>,
    pub previous_five_hour_exhausted: bool,
    pub previous_five_hour_reset_at: i64,
    pub previous_weekly_exhausted: bool,
    pub previous_weekly_reset_at: i64,
    pub retry_until: Option<i64>,
    pub retry_updated_at_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileProbeRecoveryPlan {
    pub clear_auth_failure: bool,
    pub clear_quota_backoff: bool,
    pub accept_observation: bool,
}

unsafe extern "C" {
    fn prodex_runtime_probe_recovery_v1(
        abi: i64,
        fields: u64,
        count: i64,
        output: u64,
        capacity: i64,
    ) -> i64;
}

pub fn profile_probe_recovery_plan(
    input: ProfileProbeRecoveryInput,
) -> Result<ProfileProbeRecoveryPlan, MojoError> {
    let fields = [
        i64::from(input.authenticated_success),
        i64::from(input.same_credentials),
        input.probe_started_at,
        input.now,
        i64::from(input.auth_failure.is_some()),
        i64::from(input.auth_failure.map_or(0, |v| v.0)),
        input.auth_failure.map_or(0, |v| v.1),
        i64::from(input.quota_ready),
        i64::from(input.previous_checked_at.is_some()),
        input.previous_checked_at.unwrap_or_default(),
        i64::from(input.previous_five_hour_exhausted),
        input.previous_five_hour_reset_at,
        i64::from(input.previous_weekly_exhausted),
        input.previous_weekly_reset_at,
        i64::from(input.retry_until.is_some()),
        input.retry_until.unwrap_or_default(),
        i64::from(input.retry_updated_at_ms.is_some()),
        input.retry_updated_at_ms.unwrap_or_default(),
    ];
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_probe_recovery_v1(
            1,
            fields.as_ptr() as usize as u64,
            fields.len() as i64,
            output.as_mut_ptr() as usize as u64,
            3,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if output.iter().any(|v| !matches!(v, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ProfileProbeRecoveryPlan {
        clear_auth_failure: output[0] == 1,
        clear_quota_backoff: output[1] == 1,
        accept_observation: output[2] == 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn verified() -> ProfileProbeRecoveryInput {
        ProfileProbeRecoveryInput {
            authenticated_success: true,
            same_credentials: true,
            probe_started_at: 120,
            now: 125,
            auth_failure: Some((12, 100)),
            quota_ready: true,
            previous_checked_at: Some(110),
            previous_five_hour_exhausted: true,
            previous_five_hour_reset_at: 500,
            retry_until: Some(500),
            retry_updated_at_ms: Some(110000),
            ..Default::default()
        }
    }
    #[test]
    fn confirmed_probe_recovers_only_causally_older_matching_backoffs() {
        assert_eq!(
            profile_probe_recovery_plan(verified()).unwrap(),
            ProfileProbeRecoveryPlan {
                clear_auth_failure: true,
                clear_quota_backoff: true,
                accept_observation: true
            }
        );
        for (field, value) in [(0, false), (1, false)] {
            let mut input = verified();
            if field == 0 {
                input.authenticated_success = value;
            } else {
                input.same_credentials = value;
            }
            assert_eq!(
                profile_probe_recovery_plan(input).unwrap(),
                ProfileProbeRecoveryPlan {
                    clear_auth_failure: false,
                    clear_quota_backoff: false,
                    accept_observation: false
                }
            );
        }
        let mut newer = verified();
        newer.auth_failure = Some((12, 120));
        newer.retry_updated_at_ms = Some(120000);
        assert_eq!(
            profile_probe_recovery_plan(newer).unwrap(),
            ProfileProbeRecoveryPlan {
                clear_auth_failure: false,
                clear_quota_backoff: false,
                accept_observation: false
            }
        );
        let mut rate_limit = verified();
        rate_limit.retry_until = Some(501);
        assert!(
            !profile_probe_recovery_plan(rate_limit)
                .unwrap()
                .clear_quota_backoff
        );
        let mut exhausted = verified();
        exhausted.quota_ready = false;
        assert!(
            !profile_probe_recovery_plan(exhausted)
                .unwrap()
                .clear_quota_backoff
        );
        let mut weekly = verified();
        weekly.previous_five_hour_exhausted = false;
        weekly.previous_weekly_exhausted = true;
        weekly.previous_weekly_reset_at = 500;
        assert!(
            profile_probe_recovery_plan(weekly)
                .unwrap()
                .clear_quota_backoff
        );
        let mut future = verified();
        future.probe_started_at = 130;
        assert_eq!(
            profile_probe_recovery_plan(future).unwrap(),
            ProfileProbeRecoveryPlan {
                clear_auth_failure: false,
                clear_quota_backoff: false,
                accept_observation: false
            }
        );
    }
    #[test]
    fn probe_recovery_abi_rejects_invalid_facts_without_partial_output() {
        let mut facts = [0_i64; 18];
        let mut output = [99_i64; 3];
        unsafe {
            assert_eq!(
                prodex_runtime_probe_recovery_v1(
                    2,
                    facts.as_ptr() as usize as u64,
                    18,
                    output.as_mut_ptr() as usize as u64,
                    3
                ),
                4
            );
            facts[0] = 2;
            assert_eq!(
                prodex_runtime_probe_recovery_v1(
                    1,
                    facts.as_ptr() as usize as u64,
                    18,
                    output.as_mut_ptr() as usize as u64,
                    3
                ),
                1
            );
        }
        assert_eq!(output, [99; 3]);
    }
}
