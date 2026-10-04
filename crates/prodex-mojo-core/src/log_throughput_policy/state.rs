use crate::MojoError;

use super::{ABI_VERSION, bool_output, status};

/// Sample acceptance, replay suppression, and monotonic-counter transition for one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThroughputObservationPlan {
    /// Whether this event has positive output tokens and a positive generation duration.
    pub accepted: bool,
    /// Whether the previous cumulative sample must be cleared.
    pub counter_reset: bool,
    /// Whether this event should be added to the rolling sample window.
    pub append_sample: bool,
    /// Whether this duplicate live/disk replay can be ignored without changing counters.
    pub ignore_duplicate: bool,
}

/// Ordering policy used to select a throughput state candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThroughputCandidateMode {
    /// Select the newest active stream whose age remains within the policy window.
    ActiveProfile,
    /// Select the newest active stream matching profile and rate availability.
    ActiveRate,
    /// Repair a current live identity by monotonic event time.
    LiveIdentity,
    /// Repair a historical identity by lexicographically ordered event timestamp.
    HistoricalIdentity,
}

/// Bounded state collection whose insertion threshold is owned by the throughput policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThroughputBoundedState {
    /// Active stream state keyed by runtime log and request identity.
    Streams,
    /// Seen observations retained for live/disk replay detection.
    Observations,
}

unsafe extern "C" {
    fn prodex_log_throughput_observation_plan_v1(
        abi_version: i64,
        generation_present: i64,
        previous_present: i64,
        duplicate_replay: i64,
        previous_tokens: u64,
        previous_generation_ms: u64,
        current_tokens: u64,
        current_generation_ms: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_duplicate_replay_v1(
        abi_version: i64,
        same_path: i64,
        previous_address: u64,
        previous_length: u64,
        current_address: u64,
        current_length: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_select_candidate_v1(
        abi_version: i64,
        mode: i64,
        candidate_count: u64,
        rows_address: u64,
        payload_address: u64,
        payload_length: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_sample_expired_v1(
        abi_version: i64,
        age_ns: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_bounded_insert_v1(
        abi_version: i64,
        kind: i64,
        current_count: u64,
        already_present: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_finish_rate_v1(
        abi_version: i64,
        present_mask: u64,
        rates_address: u64,
        output_address: u64,
    ) -> i64;
}

/// Computes event eligibility, counter reset/sample behavior, and duplicate suppression.
pub fn observation_plan(
    generation_ms: Option<u64>,
    previous: Option<(u64, u64)>,
    duplicate_replay: bool,
    output_tokens: u64,
) -> Result<ThroughputObservationPlan, MojoError> {
    let (previous_present, previous_tokens, previous_generation_ms) = previous
        .map(|(tokens, generation_ms)| (1_i64, tokens, generation_ms))
        .unwrap_or((0_i64, 0, 0));
    let mut output = [-1_i64; 4];
    status(unsafe {
        prodex_log_throughput_observation_plan_v1(
            ABI_VERSION,
            i64::from(u8::from(generation_ms.is_some())),
            previous_present,
            i64::from(u8::from(duplicate_replay)),
            previous_tokens,
            previous_generation_ms,
            output_tokens,
            generation_ms.unwrap_or_default(),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(ThroughputObservationPlan {
        accepted: bool_output(output[0])?,
        counter_reset: bool_output(output[1])?,
        append_sample: bool_output(output[2])?,
        ignore_duplicate: bool_output(output[3])?,
    })
}

/// Classifies a matching observation as a duplicate only across one live and one disk path.
pub fn duplicate_live_disk_replay(
    previous_path: Option<&str>,
    current_path: Option<&str>,
    same_path: bool,
) -> Result<bool, MojoError> {
    let previous = previous_path.map(str::as_bytes).unwrap_or_default();
    let current = current_path.map(str::as_bytes).unwrap_or_default();
    let mut output = -1_i64;
    status(unsafe {
        prodex_log_throughput_duplicate_replay_v1(
            ABI_VERSION,
            i64::from(same_path),
            previous.as_ptr() as usize as u64,
            u64::try_from(previous.len()).map_err(|_| MojoError::InvalidInput)?,
            current.as_ptr() as usize as u64,
            u64::try_from(current.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    bool_output(output)
}

fn select_candidate(
    mode: ThroughputCandidateMode,
    rows: &[u64],
    payload: &[u8],
) -> Result<Option<usize>, MojoError> {
    const CANDIDATE_STRIDE: usize = 4;
    if !rows.len().is_multiple_of(CANDIDATE_STRIDE) {
        return Err(MojoError::InvalidInput);
    }
    let mode = match mode {
        ThroughputCandidateMode::ActiveProfile => 1,
        ThroughputCandidateMode::ActiveRate => 2,
        ThroughputCandidateMode::LiveIdentity => 3,
        ThroughputCandidateMode::HistoricalIdentity => 4,
    };
    let candidate_count =
        u64::try_from(rows.len() / CANDIDATE_STRIDE).map_err(|_| MojoError::InvalidInput)?;
    let payload_length = u64::try_from(payload.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = u64::MAX;
    status(unsafe {
        prodex_log_throughput_select_candidate_v1(
            ABI_VERSION,
            mode,
            candidate_count,
            rows.as_ptr() as usize as u64,
            payload.as_ptr() as usize as u64,
            payload_length,
            (&mut output as *mut u64) as usize as u64,
        )
    })?;
    if output == u64::MAX {
        return Ok(None);
    }
    usize::try_from(output)
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

/// Selects the newest active profile candidate, retaining later-key ordering for timestamp ties.
pub fn select_active_profile_candidate(
    candidates: &[(bool, Option<u64>)],
) -> Result<Option<usize>, MojoError> {
    let rows = candidates
        .iter()
        .map(|(active, age_ns)| {
            (
                u64::from(u8::from(*active)) | (u64::from(u8::from(age_ns.is_some())) << 1),
                age_ns.unwrap_or_default(),
                0,
                0,
            )
        })
        .flat_map(|(flags, age_ns, third, fourth)| [flags, age_ns, third, fourth])
        .collect::<Vec<_>>();
    select_candidate(ThroughputCandidateMode::ActiveProfile, &rows, &[])
}

/// Selects the newest eligible active stream for a rate display.
pub fn select_active_rate_candidate(
    candidates: &[(bool, bool, bool, Option<u64>)],
) -> Result<Option<usize>, MojoError> {
    let rows = candidates
        .iter()
        .map(|(active, preferred_profile, has_rate, age_ns)| {
            (
                u64::from(u8::from(*active))
                    | (u64::from(u8::from(*preferred_profile)) << 1)
                    | (u64::from(u8::from(*has_rate)) << 2)
                    | (u64::from(u8::from(age_ns.is_some())) << 3),
                age_ns.unwrap_or_default(),
                0,
                0,
            )
        })
        .flat_map(|(flags, age_ns, third, fourth)| [flags, age_ns, third, fourth])
        .collect::<Vec<_>>();
    select_candidate(ThroughputCandidateMode::ActiveRate, &rows, &[])
}

/// Selects the newest stored identity while preserving `None`-time and tie ordering.
pub fn select_live_identity_candidate(
    candidates: &[(bool, Option<u64>)],
) -> Result<Option<usize>, MojoError> {
    let rows = candidates
        .iter()
        .map(|(eligible, age_ns)| {
            (
                u64::from(u8::from(*eligible)) | (u64::from(u8::from(age_ns.is_some())) << 1),
                age_ns.unwrap_or_default(),
                0,
                0,
            )
        })
        .flat_map(|(flags, age_ns, third, fourth)| [flags, age_ns, third, fourth])
        .collect::<Vec<_>>();
    select_candidate(ThroughputCandidateMode::LiveIdentity, &rows, &[])
}

/// Selects the newest historical timestamp; equal timestamps retain later profile ordering.
pub fn select_historical_identity_candidate(
    candidates: &[Option<&str>],
) -> Result<Option<usize>, MojoError> {
    let mut rows = Vec::with_capacity(candidates.len() * 4);
    let mut payload = Vec::new();
    for candidate in candidates {
        if let Some(timestamp) = candidate {
            let start = u64::try_from(payload.len()).map_err(|_| MojoError::InvalidInput)?;
            let bytes = timestamp.as_bytes();
            let length = u64::try_from(bytes.len()).map_err(|_| MojoError::InvalidInput)?;
            payload.extend_from_slice(bytes);
            rows.extend_from_slice(&[1, start, length, 0]);
        } else {
            rows.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    select_candidate(ThroughputCandidateMode::HistoricalIdentity, &rows, &payload)
}

/// Applies the shared two-second sample-retention boundary to a monotonic age.
pub fn sample_expired(age_ns: u64) -> Result<bool, MojoError> {
    let mut output = -1_i64;
    status(unsafe {
        prodex_log_throughput_sample_expired_v1(
            ABI_VERSION,
            age_ns,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    bool_output(output)
}

/// Reports whether inserting a new item requires one bounded-state eviction.
pub fn bounded_insert_needs_eviction(
    state: ThroughputBoundedState,
    current_count: usize,
    already_present: bool,
) -> Result<bool, MojoError> {
    let kind = match state {
        ThroughputBoundedState::Streams => 1,
        ThroughputBoundedState::Observations => 2,
    };
    let current_count = u64::try_from(current_count).map_err(|_| MojoError::InvalidInput)?;
    let mut output = -1_i64;
    status(unsafe {
        prodex_log_throughput_bounded_insert_v1(
            ABI_VERSION,
            kind,
            current_count,
            i64::from(u8::from(already_present)),
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    bool_output(output)
}

/// Selects the first present candidate, accepting it only when finite and positive.
pub fn finish_rate_candidate(candidates: [Option<f64>; 4]) -> Result<Option<f64>, MojoError> {
    let present_mask = candidates
        .iter()
        .position(Option::is_some)
        .map_or(0, |index| 1_u64 << index);
    let rates = candidates.map(Option::unwrap_or_default);
    let mut output = u64::MAX;
    status(unsafe {
        prodex_log_throughput_finish_rate_v1(
            ABI_VERSION,
            present_mask,
            rates.as_ptr() as usize as u64,
            (&mut output as *mut u64) as usize as u64,
        )
    })?;
    if output == u64::MAX {
        return Ok(None);
    }
    let index = usize::try_from(output).map_err(|_| MojoError::InvalidOutput)?;
    rates
        .get(index)
        .copied()
        .map(Some)
        .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_state_policy_preserves_replay_freshness_order_and_bounds() {
        let initial = observation_plan(Some(1_000), None, false, 100).unwrap();
        assert_eq!(
            initial,
            ThroughputObservationPlan {
                accepted: true,
                counter_reset: false,
                append_sample: true,
                ignore_duplicate: false,
            }
        );
        assert_eq!(
            observation_plan(Some(1_100), Some((100, 1_000)), true, 100).unwrap(),
            ThroughputObservationPlan {
                accepted: true,
                counter_reset: false,
                append_sample: false,
                ignore_duplicate: true,
            }
        );
        assert_eq!(
            observation_plan(Some(900), Some((100, 1_000)), true, 99).unwrap(),
            ThroughputObservationPlan {
                accepted: true,
                counter_reset: true,
                append_sample: true,
                ignore_duplicate: false,
            }
        );
        assert!(
            !observation_plan(Some(1_000), None, false, 0)
                .unwrap()
                .accepted
        );
        assert!(!observation_plan(None, None, false, 100).unwrap().accepted);

        assert!(
            duplicate_live_disk_replay(
                Some("broker:runtime"),
                Some("/home/test-user/runtime.log"),
                false,
            )
            .unwrap()
        );
        assert!(
            !duplicate_live_disk_replay(Some("direct:runtime"), Some("broker:runtime"), false,)
                .unwrap()
        );
        assert!(
            !duplicate_live_disk_replay(Some("broker:runtime"), Some("broker:runtime"), true,)
                .unwrap()
        );

        assert_eq!(
            select_active_profile_candidate(&[
                (true, Some(9)),
                (true, Some(9)),
                (true, Some(2_000_000_001)),
                (false, None),
            ])
            .unwrap(),
            Some(1)
        );
        assert_eq!(
            select_active_rate_candidate(&[
                (true, false, true, Some(1)),
                (true, true, true, Some(2_000_000_000)),
                (true, true, true, Some(2_000_000_001)),
            ])
            .unwrap(),
            Some(1)
        );
        assert_eq!(
            select_historical_identity_candidate(&[
                Some("2026-10-04T00:00:01Z"),
                Some("2026-10-04T00:00:02Z"),
                Some("2026-10-04T00:00:02Z"),
            ])
            .unwrap(),
            Some(2)
        );
        assert_eq!(
            finish_rate_candidate([Some(f64::INFINITY), Some(75.0), Some(f64::NAN), Some(100.0),])
                .unwrap(),
            None
        );
        assert_eq!(
            finish_rate_candidate([None, Some(75.0), Some(f64::NAN), Some(100.0)]).unwrap(),
            Some(75.0)
        );
        assert_eq!(
            finish_rate_candidate([Some(0.0), Some(f64::NEG_INFINITY), None, None]).unwrap(),
            None
        );
        assert_eq!(
            select_live_identity_candidate(&[
                (true, None),
                (true, Some(1_000)),
                (true, Some(1_000)),
            ])
            .unwrap(),
            Some(2)
        );
        assert!(!sample_expired(2_000_000_000).unwrap());
        assert!(sample_expired(2_000_000_001).unwrap());
        assert!(
            !bounded_insert_needs_eviction(ThroughputBoundedState::Streams, 63, false,).unwrap()
        );
        assert!(
            bounded_insert_needs_eviction(ThroughputBoundedState::Streams, 64, false,).unwrap()
        );
        assert!(
            !bounded_insert_needs_eviction(ThroughputBoundedState::Streams, 64, true,).unwrap()
        );
        assert!(
            !bounded_insert_needs_eviction(ThroughputBoundedState::Observations, 255, false,)
                .unwrap()
        );
        assert!(
            bounded_insert_needs_eviction(ThroughputBoundedState::Observations, 256, false,)
                .unwrap()
        );
    }
}
