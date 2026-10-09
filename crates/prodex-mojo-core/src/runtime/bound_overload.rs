//! Bounded same-owner recovery policy for an uncommitted HTTP overload.
use crate::MojoError;
use std::time::Duration;

unsafe extern "C" {
    fn prodex_runtime_bound_overload_retry_v1(
        abi_version: i64,
        hard_affinity: i64,
        previous_response_present: i64,
        committed: i64,
        retries: u64,
        elapsed_ms: u64,
        retry_after_present: i64,
        retry_after_ms: u64,
        jitter_key: u64,
    ) -> i64;
}

pub fn bound_overload_retry_delay(
    hard_affinity: bool,
    previous_response_present: bool,
    committed: bool,
    retries: usize,
    elapsed: Duration,
    retry_after: Option<Duration>,
    request_id: u64,
) -> Result<Option<Duration>, MojoError> {
    let result = unsafe {
        prodex_runtime_bound_overload_retry_v1(
            1,
            i64::from(hard_affinity),
            i64::from(previous_response_present),
            i64::from(committed),
            u64::try_from(retries).unwrap_or(u64::MAX),
            u64::try_from(elapsed.as_nanos().div_ceil(1_000_000)).unwrap_or(u64::MAX),
            i64::from(retry_after.is_some()),
            retry_after.map_or(0, |delay| {
                u64::try_from(delay.as_nanos().div_ceil(1_000_000)).unwrap_or(u64::MAX)
            }),
            request_id,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        millis if millis >= 0 => Ok(Some(Duration::from_millis(millis as u64))),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound_overload_retry_is_affinity_safe_bounded_and_honors_retry_after() {
        let zero = Duration::ZERO;
        assert_eq!(
            bound_overload_retry_delay(true, true, false, 0, zero, None, 0).unwrap(),
            None,
            "previous-response-ID full-history repair keeps priority"
        );
        assert_eq!(
            bound_overload_retry_delay(false, false, false, 0, zero, None, 0).unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(true, false, true, 0, zero, None, 0).unwrap(),
            None
        );
        for (retry, millis) in [250, 500, 1000, 2000, 4000].into_iter().enumerate() {
            assert_eq!(
                bound_overload_retry_delay(true, false, false, retry, zero, None, 0).unwrap(),
                Some(Duration::from_millis(millis))
            );
        }
        assert_eq!(
            bound_overload_retry_delay(true, false, false, 5, zero, None, 0).unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(true, false, false, usize::MAX, zero, None, 0).unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(true, false, false, 0, Duration::from_secs(60), None, 0)
                .unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(
                true,
                false,
                false,
                0,
                Duration::from_millis(59_999),
                None,
                0
            )
            .unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(
                true,
                false,
                false,
                0,
                zero,
                Some(Duration::from_secs(2)),
                0
            )
            .unwrap(),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            bound_overload_retry_delay(
                true,
                false,
                false,
                0,
                zero,
                Some(Duration::from_secs(300)),
                0
            )
            .unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(true, false, false, 0, Duration::MAX, None, 0).unwrap(),
            None
        );
        assert_eq!(
            bound_overload_retry_delay(
                true,
                false,
                false,
                0,
                zero,
                Some(Duration::from_millis(250) + Duration::from_nanos(1)),
                0
            )
            .unwrap(),
            Some(Duration::from_millis(251)),
            "ABI conversion must not advance upstream retry advice"
        );
        assert_eq!(
            unsafe { prodex_runtime_bound_overload_retry_v1(0, 1, 0, 0, 0, 0, 0, 0, 0) },
            -4
        );
        assert_eq!(
            unsafe { prodex_runtime_bound_overload_retry_v1(1, 2, 0, 0, 0, 0, 0, 0, 0) },
            -2
        );
    }
}
