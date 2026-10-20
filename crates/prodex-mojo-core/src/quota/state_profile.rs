//! Typed compiled selection of quota current profile from persisted state.
//! Rust only observes name availability in the profile map and applies the
//! selected original name. Timestamp and active fallback policy belong to Mojo.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_ROWS: usize = 65_536;

unsafe extern "C" {
    fn prodex_quota_state_current_profile_v1(
        abi: i64,
        rows_address: u64,
        count: i64,
        active_usable: i64,
        result_address: u64,
    ) -> i64;
}

#[derive(Clone, Copy, Debug)]
pub struct QuotaStateProfileInput<'a> {
    pub name: &'a str,
    pub last_selected_at: i64,
    pub exists: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaStateProfileChoice {
    LastRun(usize),
    Active,
    Unavailable,
}

pub fn quota_state_profile_choice(
    profiles: &[QuotaStateProfileInput<'_>],
    active_usable: bool,
) -> Result<QuotaStateProfileChoice, MojoError> {
    if profiles.len() > MAX_ROWS {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<i64>::new();
    rows.try_reserve_exact(profiles.len().checked_mul(4).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for profile in profiles {
        rows.extend([
            i64::try_from(profile.name.as_ptr() as usize).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(profile.name.len()).map_err(|_| MojoError::InvalidInput)?,
            profile.last_selected_at,
            i64::from(profile.exists),
        ]);
    }
    let mut selected = -7_i64;
    match unsafe {
        prodex_quota_state_current_profile_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(profiles.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(active_usable),
            (&mut selected as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match selected {
        -2 => Ok(QuotaStateProfileChoice::Unavailable),
        -1 if active_usable => Ok(QuotaStateProfileChoice::Active),
        index if index >= 0 => {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            if !profiles.get(index).is_some_and(|profile| profile.exists) {
                return Err(MojoError::InvalidOutput);
            }
            Ok(QuotaStateProfileChoice::LastRun(index))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record<'a>(name: &'a str, at: i64, exists: bool) -> QuotaStateProfileInput<'a> {
        QuotaStateProfileInput {
            name,
            last_selected_at: at,
            exists,
        }
    }

    #[test]
    fn compiled_quota_state_profile_picks_latest_known_and_lexically_greatest_tie() {
        let rows = [
            record("active", 10, true),
            record("deleted", i64::MAX, false),
            record("雪", 20, true),
            record("é", 20, true),
            record("z", 20, true),
            record("old", i64::MIN, true),
            record("雪", 20, true),
        ];
        assert_eq!(
            quota_state_profile_choice(&rows, true),
            Ok(QuotaStateProfileChoice::LastRun(2)),
        );
        assert_eq!(
            quota_state_profile_choice(&[record("unknown", 99, false)], true),
            Ok(QuotaStateProfileChoice::Active),
        );
        assert_eq!(
            quota_state_profile_choice(&[record("unknown", 99, false)], false),
            Ok(QuotaStateProfileChoice::Unavailable),
        );
        assert_eq!(
            quota_state_profile_choice(&[], true),
            Ok(QuotaStateProfileChoice::Active),
        );
        assert_eq!(
            quota_state_profile_choice(&[], false),
            Ok(QuotaStateProfileChoice::Unavailable),
        );
        assert_eq!(
            quota_state_profile_choice(
                &[record("old", i64::MIN, true), record("new", -1, true)],
                false,
            ),
            Ok(QuotaStateProfileChoice::LastRun(1)),
        );
    }

    #[test]
    fn compiled_quota_state_profile_abi_rejects_malformed_late_rows_without_write() {
        let mut rows = [0_i64; 8];
        let mut result = 79_i64;
        let source = rows.as_ptr() as usize as u64;
        let target = (&mut result as *mut i64) as usize as u64;
        for (abi, input, count, active, destination, expected) in [
            (2, source, 2, 1, target, 4),
            (1, 0, 2, 1, target, 1),
            (1, source, -1, 1, target, 1),
            (1, source, 65_537, 1, target, 1),
            (1, source, 2, 2, target, 1),
            (1, source, 2, 1, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_quota_state_current_profile_v1(abi, input, count, active, destination)
                },
                expected,
            );
            assert_eq!(result, 79);
        }
        rows[7] = 2;
        assert_eq!(
            unsafe {
                prodex_quota_state_current_profile_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    1,
                    target,
                )
            },
            1,
        );
        assert_eq!(result, 79);
    }
}
