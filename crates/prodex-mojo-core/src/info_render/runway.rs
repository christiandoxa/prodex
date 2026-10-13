use super::{InfoStringView, MojoError};

const INFO_RUNWAY_ESTIMATE_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_info_runway_estimate_v1(
        abi_version: i64,
        timestamps_address: u64,
        profiles_address: u64,
        five_hour_address: u64,
        weekly_address: u64,
        ordered_indices_address: u64,
        count: i64,
        window: i64,
        current_remaining: i64,
        now: i64,
        minimum_span_seconds: i64,
        present_address: u64,
        burn_per_hour_address: u64,
        observed_profiles_address: u64,
        observed_span_seconds_address: u64,
        exhaust_at_address: u64,
    ) -> i64;
}

/// One parsed quota observation supplied to the Mojo-owned runway estimator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoRunwayObservation<'a> {
    pub timestamp: i64,
    pub profile: &'a str,
    pub five_hour_remaining: i64,
    pub weekly_remaining: i64,
}

/// Mojo-computed aggregate quota runway.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoRunwayEstimate {
    pub burn_per_hour: f64,
    pub observed_profiles: usize,
    pub observed_span_seconds: i64,
    pub exhaust_at: i64,
}

/// Estimate quota exhaustion from bounded, already-parsed runtime observations.
pub fn estimate_info_runway(
    observations: &[InfoRunwayObservation<'_>],
    window: i64,
    current_remaining: i64,
    now: i64,
    minimum_span_seconds: i64,
) -> Result<Option<InfoRunwayEstimate>, MojoError> {
    if !matches!(window, 0 | 1) || minimum_span_seconds < 0 {
        return Err(MojoError::InvalidInput);
    }
    let timestamps = observations
        .iter()
        .map(|observation| observation.timestamp)
        .collect::<Vec<_>>();
    let profiles = observations
        .iter()
        .map(|observation| {
            Ok(InfoStringView {
                ptr: observation.profile.as_ptr() as usize as u64,
                len: u64::try_from(observation.profile.len())
                    .map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let five_hour = observations
        .iter()
        .map(|observation| observation.five_hour_remaining)
        .collect::<Vec<_>>();
    let weekly = observations
        .iter()
        .map(|observation| observation.weekly_remaining)
        .collect::<Vec<_>>();
    let mut ordered_indices = vec![0_i64; observations.len()];
    let mut present = -1_i64;
    let mut burn_per_hour = 0.0_f64;
    let mut observed_profiles = -1_i64;
    let mut observed_span_seconds = 0_i64;
    let mut exhaust_at = 0_i64;
    let status = unsafe {
        prodex_mojo_info_runway_estimate_v1(
            INFO_RUNWAY_ESTIMATE_ABI_VERSION,
            timestamps.as_ptr() as usize as u64,
            profiles.as_ptr() as usize as u64,
            five_hour.as_ptr() as usize as u64,
            weekly.as_ptr() as usize as u64,
            ordered_indices.as_mut_ptr() as usize as u64,
            i64::try_from(observations.len()).map_err(|_| MojoError::InvalidInput)?,
            window,
            current_remaining,
            now,
            minimum_span_seconds,
            (&mut present as *mut i64) as usize as u64,
            (&mut burn_per_hour as *mut f64) as usize as u64,
            (&mut observed_profiles as *mut i64) as usize as u64,
            (&mut observed_span_seconds as *mut i64) as usize as u64,
            (&mut exhaust_at as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidInput),
    }
    match present {
        0 => Ok(None),
        1 if burn_per_hour.is_finite()
            && burn_per_hour >= 0.0
            && observed_profiles >= 0
            && observed_span_seconds >= 0 =>
        {
            Ok(Some(InfoRunwayEstimate {
                burn_per_hour,
                observed_profiles: usize::try_from(observed_profiles)
                    .map_err(|_| MojoError::InvalidOutput)?,
                observed_span_seconds,
                exhaust_at,
            }))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod runway_tests {
    use super::{InfoRunwayObservation, estimate_info_runway};

    #[test]
    fn runway_groups_profiles_and_projects_exhaustion_through_mojo() {
        let observations = [
            InfoRunwayObservation {
                timestamp: 0,
                profile: "alpha",
                five_hour_remaining: 100,
                weekly_remaining: 90,
            },
            InfoRunwayObservation {
                timestamp: 600,
                profile: "alpha",
                five_hour_remaining: 50,
                weekly_remaining: 80,
            },
            InfoRunwayObservation {
                timestamp: 0,
                profile: "steady",
                five_hour_remaining: 100,
                weekly_remaining: 100,
            },
            InfoRunwayObservation {
                timestamp: 600,
                profile: "steady",
                five_hour_remaining: 100,
                weekly_remaining: 100,
            },
        ];

        let estimate = estimate_info_runway(&observations, 0, 50, 1_000, 60)
            .unwrap()
            .expect("decreasing profile should produce a runway");
        assert_eq!(estimate.observed_profiles, 1);
        assert_eq!(estimate.observed_span_seconds, 600);
        assert_eq!(estimate.exhaust_at, 1_600);
        assert!((estimate.burn_per_hour - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn runway_reports_exhaustion_without_observations_and_rejects_bad_window() {
        let estimate = estimate_info_runway(&[], 1, 0, 77, 60)
            .unwrap()
            .expect("exhausted quota is a valid estimate");
        assert_eq!(estimate.exhaust_at, 77);
        assert_eq!(estimate.observed_profiles, 0);
        assert!(estimate_info_runway(&[], 2, 1, 77, 60).is_err());
    }
}
