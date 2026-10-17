use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_CONTENT_LENGTH: i64 = 0;
const MODE_ADMISSION: i64 = 1;
const MODE_ERROR_RESPONSE: i64 = 2;

const REJECTION_NONE: u8 = 0;
#[cfg(test)]
const REJECTION_LANE: u8 = 2;
const REJECTION_DEADLINE: u8 = 3;

const ADMISSION_ALLOW: u64 = 0;
const ADMISSION_REJECT: u64 = 1;
const ADMISSION_CAPTURE_AND_RETRY: u64 = 2;

const RESPONSE_BODY_TOO_LARGE: u64 = 0;
const RESPONSE_CAPTURE_FAILED: u64 = 1;
const RESPONSE_TRANSPORT_FAILED: u64 = 2;
const RESPONSE_REWRITE_FAILED: u64 = 3;

unsafe extern "C" {
    fn prodex_runtime_proxy_dispatch_policy_v1(
        abi_version: i64,
        mode: i64,
        rejection_kind: i64,
        lane_kind: i64,
        websocket: i64,
        capture_attempted: i64,
        error_kind: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_precommit_wait_slice_v1(
        abi_version: i64,
        requested_nanos: u64,
        remaining_nanos: u64,
        output_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyAdmissionAction {
    Allow,
    Reject,
    CaptureAndRetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyAdmissionPlan {
    pub action: RuntimeProxyAdmissionAction,
    pub marks_global_overload: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyErrorResponse {
    BodyTooLarge,
    CaptureFailed,
    TransportFailed,
    RewriteFailed,
}

/// Selects the smaller host wait and request-deadline remainder in Mojo.
pub fn precommit_wait_slice(
    requested: std::time::Duration,
    remaining: std::time::Duration,
) -> Result<std::time::Duration, MojoError> {
    let requested_nanos = u64::try_from(requested.as_nanos()).unwrap_or(u64::MAX);
    let remaining_nanos = u64::try_from(remaining.as_nanos()).unwrap_or(u64::MAX);
    let mut output = u64::MAX;
    let status = unsafe {
        prodex_runtime_precommit_wait_slice_v1(
            ABI_VERSION,
            requested_nanos,
            remaining_nanos,
            (&mut output as *mut u64) as usize as u64,
        )
    };
    match status {
        0 => Ok(std::time::Duration::from_nanos(output)),
        4 => Err(MojoError::AbiMismatch),
        1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn call(
    mode: i64,
    rejection_kind: u8,
    lane_kind: u8,
    websocket: bool,
    capture_attempted: bool,
    error_kind: u8,
    input: &str,
) -> Result<[u64; 4], MojoError> {
    let mut output = [0_u64; 4];
    let status = unsafe {
        prodex_runtime_proxy_dispatch_policy_v1(
            ABI_VERSION,
            mode,
            i64::from(rejection_kind),
            i64::from(lane_kind),
            i64::from(websocket),
            i64::from(capture_attempted),
            i64::from(error_kind),
            if input.is_empty() {
                0
            } else {
                input.as_ptr() as usize as u64
            },
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Parse one already-selected `Content-Length` header value.
pub fn content_length_value(value: &str) -> Result<Option<u64>, MojoError> {
    let output = call(
        MODE_CONTENT_LENGTH,
        REJECTION_NONE,
        0,
        false,
        false,
        0,
        value,
    )?;
    match output[0] {
        0 => Ok(None),
        1 => Ok(Some(output[1])),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Decide whether an admission rejection may take the one bounded capture retry.
pub fn admission_plan(
    rejection_kind: u8,
    lane_kind: u8,
    websocket: bool,
    capture_attempted: bool,
) -> Result<RuntimeProxyAdmissionPlan, MojoError> {
    if rejection_kind > REJECTION_DEADLINE || lane_kind > 3 {
        return Err(MojoError::InvalidInput);
    }
    let output = call(
        MODE_ADMISSION,
        rejection_kind,
        lane_kind,
        websocket,
        capture_attempted,
        0,
        "",
    )?;
    let action = match output[0] {
        ADMISSION_ALLOW => RuntimeProxyAdmissionAction::Allow,
        ADMISSION_REJECT => RuntimeProxyAdmissionAction::Reject,
        ADMISSION_CAPTURE_AND_RETRY => RuntimeProxyAdmissionAction::CaptureAndRetry,
        _ => return Err(MojoError::InvalidOutput),
    };
    let marks_global_overload = match output[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(RuntimeProxyAdmissionPlan {
        action,
        marks_global_overload,
    })
}

/// Select the local response class for a classified dispatch error.
pub fn error_response(error_kind: u8) -> Result<RuntimeProxyErrorResponse, MojoError> {
    if error_kind > 3 {
        return Err(MojoError::InvalidInput);
    }
    let output = call(
        MODE_ERROR_RESPONSE,
        REJECTION_NONE,
        0,
        false,
        false,
        error_kind,
        "",
    )?;
    match output[0] {
        RESPONSE_BODY_TOO_LARGE => Ok(RuntimeProxyErrorResponse::BodyTooLarge),
        RESPONSE_CAPTURE_FAILED => Ok(RuntimeProxyErrorResponse::CaptureFailed),
        RESPONSE_TRANSPORT_FAILED => Ok(RuntimeProxyErrorResponse::TransportFailed),
        RESPONSE_REWRITE_FAILED => Ok(RuntimeProxyErrorResponse::RewriteFailed),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_content_length_policy_handles_empty_malformed_and_unsigned_boundaries() {
        for value in [
            "",
            "  ",
            "not-a-number",
            "1_0",
            "-1",
            "18446744073709551616",
        ] {
            assert_eq!(content_length_value(value), Ok(None), "value={value:?}");
        }
        for (value, expected) in [
            ("0", 0),
            (" +01 ", 1),
            ("64", 64),
            (" \u{2003}64\u{2003} ", 64),
            ("18446744073709551615", u64::MAX),
        ] {
            assert_eq!(
                content_length_value(value),
                Ok(Some(expected)),
                "value={value:?}"
            );
        }
    }

    #[test]
    fn precommit_wait_slice_uses_the_smallest_deadline_bound() {
        assert_eq!(
            precommit_wait_slice(
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(2),
            ),
            Ok(std::time::Duration::from_secs(2)),
        );
        assert_eq!(
            precommit_wait_slice(
                std::time::Duration::from_millis(2),
                std::time::Duration::from_secs(5),
            ),
            Ok(std::time::Duration::from_millis(2)),
        );
    }

    #[test]
    fn precommit_wait_slice_abi_rejects_bad_version_without_writing() {
        let mut output = 77_u64;
        assert_eq!(
            unsafe {
                prodex_runtime_precommit_wait_slice_v1(
                    ABI_VERSION + 1,
                    1,
                    2,
                    (&mut output as *mut u64) as usize as u64,
                )
            },
            4
        );
        assert_eq!(output, 77);
    }

    #[test]
    fn dispatch_admission_policy_preserves_lane_precedence() {
        assert_eq!(
            admission_plan(1, 0, false, false).unwrap(),
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::Reject,
                marks_global_overload: true,
            }
        );
        assert_eq!(
            admission_plan(REJECTION_LANE, 0, false, false).unwrap(),
            RuntimeProxyAdmissionPlan {
                action: RuntimeProxyAdmissionAction::CaptureAndRetry,
                marks_global_overload: true,
            }
        );
        assert_eq!(
            admission_plan(REJECTION_LANE, 1, false, false)
                .unwrap()
                .action,
            RuntimeProxyAdmissionAction::CaptureAndRetry
        );
        assert_eq!(
            admission_plan(REJECTION_LANE, 2, true, false)
                .unwrap()
                .action,
            RuntimeProxyAdmissionAction::Reject
        );
        assert_eq!(
            admission_plan(REJECTION_LANE, 3, false, true)
                .unwrap()
                .action,
            RuntimeProxyAdmissionAction::Reject
        );
        assert_eq!(
            admission_plan(REJECTION_NONE, 3, false, false)
                .unwrap()
                .action,
            RuntimeProxyAdmissionAction::Allow
        );
    }

    #[test]
    fn dispatch_expired_admission_is_terminal_without_account_penalty_04381() {
        for lane in 0..=3 {
            for websocket in [false, true] {
                for captured in [false, true] {
                    assert_eq!(
                        admission_plan(REJECTION_DEADLINE, lane, websocket, captured).unwrap(),
                        RuntimeProxyAdmissionPlan {
                            action: RuntimeProxyAdmissionAction::Reject,
                            marks_global_overload: false
                        }
                    );
                }
            }
        }
        assert_eq!(
            admission_plan(4, 0, false, false),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn dispatch_error_policy_keeps_capture_transport_and_rewrite_classes_distinct() {
        assert_eq!(
            error_response(0).unwrap(),
            RuntimeProxyErrorResponse::BodyTooLarge
        );
        assert_eq!(
            error_response(1).unwrap(),
            RuntimeProxyErrorResponse::CaptureFailed
        );
        assert_eq!(
            error_response(2).unwrap(),
            RuntimeProxyErrorResponse::TransportFailed
        );
        assert_eq!(
            error_response(3).unwrap(),
            RuntimeProxyErrorResponse::RewriteFailed
        );
    }
}
