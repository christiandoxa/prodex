//! Bounded typed ABI for canonical Mojo quota-watch cadence and cache lifetime.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_quota_watch_refresh_v1(
        abi_version: i64,
        windows_address: u64,
        windows_count: i64,
        watch: i64,
        profile_count: u64,
        now: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_cache_alive_until_v1(
        abi_version: i64,
        now: i64,
        refresh_seconds: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_cache_remaining_v1(
        abi_version: i64,
        alive_until: i64,
        now: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_quota_watch_cache_live_v1(
        abi_version: i64,
        alive_until: i64,
        now: i64,
        result_address: u64,
    ) -> i64;
}

fn check(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn refresh_seconds(
    reset_windows: &[i64],
    watch: bool,
    profile_count: usize,
    now: i64,
) -> Result<u64, MojoError> {
    let mut result = 0_u64;
    check(unsafe {
        prodex_quota_watch_refresh_v1(
            ABI_VERSION,
            reset_windows.as_ptr() as usize as u64,
            i64::try_from(reset_windows.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(watch),
            u64::try_from(profile_count).map_err(|_| MojoError::InvalidInput)?,
            now,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    Ok(result)
}

pub fn cache_alive_until(now: i64, refresh_seconds: u64) -> Result<i64, MojoError> {
    let mut result = 0_i64;
    check(unsafe {
        prodex_quota_watch_cache_alive_until_v1(
            ABI_VERSION,
            now,
            refresh_seconds,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    Ok(result)
}

pub fn cache_remaining_seconds(alive_until: i64, now: i64) -> Result<u64, MojoError> {
    let mut result = 0_u64;
    check(unsafe {
        prodex_quota_watch_cache_remaining_v1(
            ABI_VERSION,
            alive_until,
            now,
            (&mut result as *mut u64) as usize as u64,
        )
    })?;
    Ok(result)
}

pub fn cache_is_live(alive_until: i64, now: i64) -> Result<bool, MojoError> {
    let mut result = -1_i64;
    check(unsafe {
        prodex_quota_watch_cache_live_v1(
            ABI_VERSION,
            alive_until,
            now,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_watch_ffi_rejects_invalid_version_and_output_pointers() {
        let mut result = -1_i64;
        let result_address = (&mut result as *mut i64) as usize as u64;
        assert_eq!(
            unsafe { prodex_quota_watch_cache_live_v1(0, 1, 1, result_address) },
            4,
        );
        assert_eq!(
            unsafe { prodex_quota_watch_cache_live_v1(ABI_VERSION, 1, 1, 0) },
            1
        );
        assert_eq!(
            unsafe { prodex_quota_watch_refresh_v1(ABI_VERSION, 0, 1, 0, 1, 0, result_address) },
            1,
        );
    }

    #[test]
    fn cadence_and_window_boundaries_are_mojo_owned() {
        assert_eq!(refresh_seconds(&[], false, 1, 100).unwrap(), 45);
        assert_eq!(refresh_seconds(&[], true, 1, 100).unwrap(), 10);
        assert_eq!(refresh_seconds(&[220], false, 1, 100).unwrap(), 5);
        assert_eq!(refresh_seconds(&[221], false, 1, 100).unwrap(), 10);
        assert_eq!(refresh_seconds(&[1001], false, 1, 100).unwrap(), 45);
        assert_eq!(refresh_seconds(&[220], true, 100, 100).unwrap(), 200);
        assert_eq!(refresh_seconds(&[i64::MAX], false, 0, i64::MAX).unwrap(), 5);
        assert_eq!(
            refresh_seconds(&[], false, usize::MAX, 100).unwrap(),
            u64::MAX
        );
    }

    #[test]
    fn lifetime_overflow_and_expiry_are_mojo_owned() {
        assert_eq!(cache_alive_until(100, 0).unwrap(), 111);
        assert_eq!(cache_alive_until(i64::MAX - 2, 1).unwrap(), i64::MAX);
        assert_eq!(
            cache_alive_until(100, u64::MAX).unwrap(),
            100 + i64::MAX / 2 + 10
        );
        assert_eq!(cache_remaining_seconds(50, 50).unwrap(), 1);
        assert_eq!(cache_remaining_seconds(70, 50).unwrap(), 20);
        assert_eq!(
            cache_remaining_seconds(i64::MAX, i64::MIN).unwrap(),
            i64::MAX as u64
        );
        assert!(cache_is_live(50, 50).unwrap());
        assert!(!cache_is_live(49, 50).unwrap());
    }
}
