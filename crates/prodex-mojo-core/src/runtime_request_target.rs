use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_request_target_valid_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
}

/// Validate one local rewrite HTTP request target through the Mojo ABI.
pub fn is_valid(value: &str) -> Result<bool, MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_request_target_valid_v1(
            ABI_VERSION,
            if value.is_empty() {
                0
            } else {
                value.as_ptr() as usize as u64
            },
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => match output {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_target_validation_handles_security_boundaries() {
        for value in [
            "/v1/responses",
            "/v1/items?query=./..",
            "/v1/%41",
            &format!("/{}", "x".repeat(8 * 1024 - 1)),
        ] {
            assert!(is_valid(value).unwrap(), "value={value:?}");
        }
        for value in [
            "",
            "responses",
            "//v1/responses",
            "/v1//responses",
            "/v1/./responses",
            "/v1/../responses",
            "/v1/%2fresponses",
            "/v1/%2E/responses",
            "/v1/%25responses",
            "/v1/%zz",
            "/v1/%",
            "/v1/responses#fragment",
            "/v1/responses\n",
            "\u{00e9}",
            &format!("/{}", "x".repeat(8 * 1024)),
        ] {
            assert!(!is_valid(value).unwrap(), "value={value:?}");
        }
    }

    #[test]
    fn request_target_abi_rejects_bad_version_and_pointers() {
        let mut output = 7_i64;
        let value = b"/v1/responses";
        assert_eq!(
            unsafe {
                prodex_runtime_request_target_valid_v1(
                    ABI_VERSION + 1,
                    value.as_ptr() as u64,
                    value.len() as i64,
                    (&mut output as *mut i64) as u64,
                )
            },
            4
        );
        assert_eq!(output, 7);
        assert_eq!(
            unsafe {
                prodex_runtime_request_target_valid_v1(
                    ABI_VERSION,
                    0,
                    1,
                    (&mut output as *mut i64) as u64,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_request_target_valid_v1(
                    ABI_VERSION,
                    value.as_ptr() as u64,
                    -1,
                    (&mut output as *mut i64) as u64,
                )
            },
            1
        );
        assert_eq!(output, 7);
        assert_eq!(
            unsafe {
                prodex_runtime_request_target_valid_v1(
                    ABI_VERSION,
                    value.as_ptr() as u64,
                    value.len() as i64,
                    0,
                )
            },
            1
        );
    }
}
