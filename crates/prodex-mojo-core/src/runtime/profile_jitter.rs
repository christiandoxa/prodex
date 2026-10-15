use crate::MojoError;

const RUNTIME_PROFILE_SELECTION_JITTER_ABI_VERSION: i64 = 1;
const RUNTIME_PROFILE_SELECTION_JITTER_MAX_BYTES: usize = 4_096;

unsafe extern "C" {
    fn prodex_runtime_profile_selection_jitter_v1(
        abi_version: i64,
        request_sequence: u64,
        profile_address: u64,
        profile_length: i64,
        route_kind: i64,
        output_address: u64,
    ) -> i64;
}

/// Computes the stable per-request profile-selection jitter owned by Mojo.
pub fn profile_selection_jitter(
    request_sequence: u64,
    profile_name: &str,
    route_kind: i64,
) -> Result<u64, MojoError> {
    if profile_name.len() > RUNTIME_PROFILE_SELECTION_JITTER_MAX_BYTES
        || !(0..=3).contains(&route_kind)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_u64;
    let status = unsafe {
        prodex_runtime_profile_selection_jitter_v1(
            RUNTIME_PROFILE_SELECTION_JITTER_ABI_VERSION,
            request_sequence,
            profile_name.as_ptr() as u64,
            i64::try_from(profile_name.len()).map_err(|_| MojoError::InvalidInput)?,
            route_kind,
            (&mut output as *mut u64) as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 | 2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_jitter_abi_rejects_invalid_inputs() {
        let mut output = 0_u64;
        let invalid_utf8 = [0xFF_u8];
        assert_eq!(
            unsafe { prodex_runtime_profile_selection_jitter_v1(0, 0, 0, 0, 0, 1) },
            4
        );
        assert_eq!(
            unsafe { prodex_runtime_profile_selection_jitter_v1(1, 0, 0, 0, 0, 0) },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_profile_selection_jitter_v1(
                    1,
                    0,
                    0,
                    0,
                    4,
                    (&mut output as *mut u64) as u64,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_profile_selection_jitter_v1(
                    1,
                    0,
                    1,
                    (RUNTIME_PROFILE_SELECTION_JITTER_MAX_BYTES + 1) as i64,
                    0,
                    (&mut output as *mut u64) as u64,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_profile_selection_jitter_v1(
                    1,
                    0,
                    0,
                    1,
                    0,
                    (&mut output as *mut u64) as u64,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_profile_selection_jitter_v1(
                    1,
                    0,
                    invalid_utf8.as_ptr() as u64,
                    invalid_utf8.len() as i64,
                    0,
                    (&mut output as *mut u64) as u64,
                )
            },
            2
        );
        assert_eq!(
            profile_selection_jitter(0, "", -1),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            profile_selection_jitter(
                0,
                &"x".repeat(RUNTIME_PROFILE_SELECTION_JITTER_MAX_BYTES + 1),
                0
            ),
            Err(MojoError::InvalidInput)
        );
    }
}
