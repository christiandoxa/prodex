use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

#[path = "ping_protocol/diagnostic.rs"]
mod diagnostic;
pub use diagnostic::{
    PING_ERROR_DETAIL_MAX_BYTES, PingChildExit, ping_bounded_detail, ping_output_failure_detail,
    ping_process_error,
};
#[path = "ping_protocol/failure_detail.rs"]
mod failure_detail;
pub use failure_detail::ping_structured_failure_detail;

const PING_PROTOCOL_ABI_VERSION: i64 = 1;
const PING_PROTOCOL_MAX_BYTES: usize = 4 * 1024 * 1024;
const PING_STATUS_DETAIL_MAX_BYTES: usize = 128;
const PING_STATUS_LABEL_MAX_BYTES: usize = 32;
const PING_SUMMARY_MAX_STATUSES: usize = 65_536;

pub const PING_STATUS_OK: i64 = 0;
pub const PING_STATUS_PROTOCOL_FAILED: i64 = 1;
pub const PING_STATUS_UNEXPECTED_RESPONSE: i64 = 2;
pub const PING_STATUS_TURN_FAILED: i64 = 3;
pub const PING_STATUS_AUTH_FAILED: i64 = 4;
pub const PING_STATUS_DNS_FAILED: i64 = 5;
pub const PING_STATUS_TLS_FAILED: i64 = 6;
pub const PING_STATUS_TIMEOUT: i64 = 7;
pub const PING_STATUS_RATE_LIMITED: i64 = 8;
pub const PING_STATUS_QUOTA_EXHAUSTED: i64 = 9;
pub const PING_STATUS_UPSTREAM_OVERLOADED: i64 = 10;
pub const PING_STATUS_MODEL_UNAVAILABLE: i64 = 11;
pub const PING_STATUS_PROCESS_FAILED: i64 = 12;
pub const PING_STATUS_SPAWN_FAILED: i64 = 13;
pub const PING_STATUS_CANCELLED: i64 = 14;

unsafe extern "C" {
    fn prodex_mojo_ping_profile_order_v1(
        abi_version: i64,
        names_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_mojo_ping_validate_jsonl_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        status_address: u64,
    ) -> i64;
    fn prodex_mojo_ping_classify_failure_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        status_address: u64,
    ) -> i64;
    fn prodex_mojo_ping_status_detail_v1(
        abi_version: i64,
        status: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_ping_status_label_v1(
        abi_version: i64,
        status: i64,
        json_label: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_ping_status_temporary_v1(
        abi_version: i64,
        status: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_ping_summary_plan_v1(
        abi_version: i64,
        statuses_address: u64,
        status_count: i64,
        output_address: u64,
    ) -> i64;
}

fn call(
    input: &str,
    function: unsafe extern "C" fn(i64, u64, i64, u64) -> i64,
) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    if input.len() > PING_PROTOCOL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut status = PING_STATUS_PROCESS_FAILED;
    let code = unsafe {
        function(
            PING_PROTOCOL_ABI_VERSION,
            input.as_ptr() as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut status),
        )
    };
    match code {
        0 => Ok(status),
        1 | 2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Sort completed ping result source indices by original profile name
/// bytes, with stable indices on equal names. Host retains result ownership.
pub fn ping_profile_order(names: &[&str]) -> Result<Vec<usize>, MojoError> {
    ensure_rich_abi()?;
    if names.len() > PING_SUMMARY_MAX_STATUSES
        || names
            .iter()
            .any(|name| name.len() > PING_PROTOCOL_MAX_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut views = Vec::<u64>::new();
    views
        .try_reserve_exact(names.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for name in names {
        views.extend([
            name.as_ptr() as usize as u64,
            u64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut indices = vec![-1_i64; names.len()];
    match unsafe {
        prodex_mojo_ping_profile_order_v1(
            PING_PROTOCOL_ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(names.len()).map_err(|_| MojoError::InvalidInput)?,
            indices.as_mut_ptr() as usize as u64,
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let mut seen = vec![false; names.len()];
    indices
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            let slot = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *slot {
                return Err(MojoError::InvalidOutput);
            }
            *slot = true;
            Ok(index)
        })
        .collect()
}

pub fn ping_validate_jsonl(input: &str) -> Result<i64, MojoError> {
    call(input, prodex_mojo_ping_validate_jsonl_v1)
}

pub fn ping_classify_failure(input: &str) -> Result<i64, MojoError> {
    call(input, prodex_mojo_ping_classify_failure_v1)
}

pub fn ping_status_detail(status: i64) -> Result<String, MojoError> {
    let mut output = vec![0_u8; PING_STATUS_DETAIL_MAX_BYTES];
    let mut written = -1_i64;
    let code = unsafe {
        prodex_mojo_ping_status_detail_v1(
            PING_PROTOCOL_ABI_VERSION,
            status,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match code {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn ping_status_label(status: i64, json_label: bool) -> Result<String, MojoError> {
    let mut output = vec![0_u8; PING_STATUS_LABEL_MAX_BYTES];
    let mut written = -1_i64;
    let code = unsafe {
        prodex_mojo_ping_status_label_v1(
            PING_PROTOCOL_ABI_VERSION,
            status,
            i64::from(json_label),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match code {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn ping_status_is_temporary(status: i64) -> Result<bool, MojoError> {
    let mut temporary = -1_i64;
    let code = unsafe {
        prodex_mojo_ping_status_temporary_v1(
            PING_PROTOCOL_ABI_VERSION,
            status,
            (&mut temporary as *mut i64) as usize as u64,
        )
    };
    match code {
        0 if matches!(temporary, 0 | 1) => Ok(temporary == 1),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Mojo-owned aggregate buckets used by the multi-profile ping summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingSummaryPlan {
    pub total: u64,
    pub healthy: u64,
    pub exhausted: u64,
    pub auth_failures: u64,
    pub temporary_failures: u64,
    pub other_failures: u64,
    pub pool_usable: bool,
}

/// Aggregate ping status codes without reimplementing the failure taxonomy in Rust.
pub fn ping_summary_plan(statuses: &[i64]) -> Result<PingSummaryPlan, MojoError> {
    ensure_rich_abi()?;
    if statuses.len() > PING_SUMMARY_MAX_STATUSES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [u64::MAX; 7];
    let code = unsafe {
        prodex_mojo_ping_summary_plan_v1(
            PING_PROTOCOL_ABI_VERSION,
            if statuses.is_empty() {
                0
            } else {
                statuses.as_ptr() as usize as u64
            },
            i64::try_from(statuses.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match code {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let expected_total = u64::try_from(statuses.len()).map_err(|_| MojoError::InvalidOutput)?;
    if output[0] != expected_total
        || output[1]
            .saturating_add(output[2])
            .saturating_add(output[3])
            .saturating_add(output[4])
            .saturating_add(output[5])
            != output[0]
    {
        return Err(MojoError::InvalidOutput);
    }
    let pool_usable = match output[6] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if pool_usable != (output[1] > 0) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(PingSummaryPlan {
        total: output[0],
        healthy: output[1],
        exhausted: output[2],
        auth_failures: output[3],
        temporary_failures: output[4],
        other_failures: output[5],
        pool_usable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_jsonl_requires_completed_model_response() {
        let ok = r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"agent_message","text":"Hello"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#;
        assert_eq!(ping_validate_jsonl(ok).unwrap(), PING_STATUS_OK);

        let incomplete = r#"{"type":"thread.started","thread_id":"t"}"#;
        assert_eq!(
            ping_validate_jsonl(incomplete).unwrap(),
            PING_STATUS_PROTOCOL_FAILED
        );
    }

    #[test]
    fn ping_jsonl_terminal_state_boundaries() {
        let no_agent = r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"turn.completed"}"#;
        assert_eq!(
            ping_validate_jsonl(no_agent).unwrap(),
            PING_STATUS_UNEXPECTED_RESPONSE
        );

        let no_completion = r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"agent_message","text":"Hello"}}"#;
        assert_eq!(
            ping_validate_jsonl(no_completion).unwrap(),
            PING_STATUS_PROTOCOL_FAILED
        );

        let full = r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"agent_message","text":"Hello"}}
{"type":"turn.completed"}"#;
        assert_eq!(ping_validate_jsonl(full).unwrap(), PING_STATUS_OK);
    }

    #[test]
    fn ping_jsonl_rejects_tool_activity() {
        let input = r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"command_execution","command":"touch file"}}
{"type":"turn.completed"}"#;
        assert_eq!(
            ping_validate_jsonl(input).unwrap(),
            PING_STATUS_PROTOCOL_FAILED
        );
    }

    #[test]
    fn ping_failure_taxonomy_preserves_precedence() {
        assert_eq!(
            ping_classify_failure("HTTP 503 quota exceeded").unwrap(),
            PING_STATUS_UPSTREAM_OVERLOADED
        );
        assert_eq!(
            ping_classify_failure("HTTP 429 insufficient_quota").unwrap(),
            PING_STATUS_QUOTA_EXHAUSTED
        );
        assert_eq!(
            ping_classify_failure("HTTP 429 quota exceeded").unwrap(),
            PING_STATUS_RATE_LIMITED
        );
        assert_eq!(
            ping_classify_failure("usage_limit_reached").unwrap(),
            PING_STATUS_QUOTA_EXHAUSTED
        );
        assert_eq!(
            ping_classify_failure("failed to start codex child").unwrap(),
            PING_STATUS_SPAWN_FAILED
        );
    }

    #[test]
    fn structured_turn_failure_classifies_quota() {
        let input = r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"turn.failed","error":{"message":"usage_limit_reached"}}"#;
        assert_eq!(
            ping_validate_jsonl(input).unwrap(),
            PING_STATUS_QUOTA_EXHAUSTED
        );
    }

    #[test]
    fn status_details_are_owned_by_the_ping_protocol_kernel() {
        assert_eq!(
            ping_status_detail(PING_STATUS_OK).unwrap(),
            "valid model response received"
        );
        assert_eq!(
            ping_status_detail(PING_STATUS_QUOTA_EXHAUSTED).unwrap(),
            "OpenAI reported quota exhaustion"
        );
        assert_eq!(
            ping_status_detail(PING_STATUS_UNEXPECTED_RESPONSE).unwrap(),
            "completed turn did not return a model response"
        );
    }

    #[test]
    fn status_labels_and_temporary_classification_are_owned_by_mojo() {
        assert_eq!(ping_status_label(PING_STATUS_OK, false).unwrap(), "OK");
        assert_eq!(
            ping_status_label(PING_STATUS_QUOTA_EXHAUSTED, true).unwrap(),
            "exhausted"
        );
        assert!(ping_status_is_temporary(PING_STATUS_TIMEOUT).unwrap());
        assert!(!ping_status_is_temporary(PING_STATUS_QUOTA_EXHAUSTED).unwrap());
    }

    #[test]
    fn summary_buckets_preserve_status_precedence() {
        let plan = ping_summary_plan(&[
            PING_STATUS_OK,
            PING_STATUS_QUOTA_EXHAUSTED,
            PING_STATUS_AUTH_FAILED,
            PING_STATUS_TIMEOUT,
            PING_STATUS_PROTOCOL_FAILED,
            PING_STATUS_OK,
        ])
        .unwrap();
        assert_eq!(
            plan,
            PingSummaryPlan {
                total: 6,
                healthy: 2,
                exhausted: 1,
                auth_failures: 1,
                temporary_failures: 1,
                other_failures: 1,
                pool_usable: true,
            }
        );
        assert!(!ping_summary_plan(&[]).unwrap().pool_usable);
        assert_eq!(
            ping_summary_plan(&[PING_STATUS_CANCELLED, PING_STATUS_TURN_FAILED])
                .unwrap()
                .other_failures,
            2
        );
    }
}

#[cfg(test)]
mod compiled_ping_profile_order_tests {
    use super::*;

    #[test]
    fn compiled_ping_profile_order_preserves_utf8_lexical_and_original_equal_order() {
        let names = ["雪", "z", "a", "é", "", "a", "🦀", "雪"];
        assert_eq!(ping_profile_order(&names), Ok(vec![4, 2, 5, 1, 3, 0, 7, 6]));
        assert_eq!(ping_profile_order(&[]), Ok(vec![]));
        assert_eq!(ping_profile_order(&["same", "same"]), Ok(vec![0, 1]));
        let many = (0..1025)
            .rev()
            .map(|index| format!("profile-{index:04}"))
            .collect::<Vec<_>>();
        let borrowed = many.iter().map(String::as_str).collect::<Vec<_>>();
        let indices = ping_profile_order(&borrowed).unwrap();
        assert_eq!(indices.len(), 1025);
        assert_eq!(indices[0], 1024);
        assert_eq!(indices[1024], 0);
        assert!(
            indices
                .windows(2)
                .all(|pair| borrowed[pair[0]] <= borrowed[pair[1]])
        );
    }

    #[test]
    fn compiled_ping_profile_order_abi_rejects_malformed_late_view_without_writing() {
        let mut views = [0_u64; 4];
        let mut indices = [71_i64; 2];
        let source = views.as_ptr() as usize as u64;
        let output = indices.as_mut_ptr() as usize as u64;
        for (abi, input, count, capacity, expected) in [
            (2, source, 2, 2, 4),
            (1, 0, 2, 2, 1),
            (1, source, -1, 2, 1),
            (1, source, 65_537, 2, 1),
            (1, source, 2, 1, 1),
            (1, source, 2, -1, 1),
            (1, source, 2, 2, 0),
        ] {
            assert_eq!(
                unsafe { prodex_mojo_ping_profile_order_v1(abi, input, count, output, capacity,) },
                expected,
            );
            if expected != 0 {
                assert_eq!(indices, [71; 2]);
            }
        }
        indices = [71; 2];
        views[3] = u64::MAX;
        assert_eq!(
            unsafe {
                prodex_mojo_ping_profile_order_v1(1, views.as_ptr() as usize as u64, 2, output, 2)
            },
            1,
        );
        assert_eq!(indices, [71; 2]);
        let malformed = [255_u8, 254];
        views[0] = malformed.as_ptr() as usize as u64;
        views[1] = 2;
        views[3] = 0;
        assert_eq!(
            unsafe {
                prodex_mojo_ping_profile_order_v1(1, views.as_ptr() as usize as u64, 2, output, 2)
            },
            1,
        );
        assert_eq!(indices, [71; 2]);
    }
}
