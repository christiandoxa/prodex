use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

const PING_PROTOCOL_ABI_VERSION: i64 = 1;
const PING_PROTOCOL_MAX_BYTES: usize = 4 * 1024 * 1024;

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

pub fn ping_validate_jsonl(input: &str) -> Result<i64, MojoError> {
    call(input, prodex_mojo_ping_validate_jsonl_v1)
}

pub fn ping_classify_failure(input: &str) -> Result<i64, MojoError> {
    call(input, prodex_mojo_ping_classify_failure_v1)
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
}
