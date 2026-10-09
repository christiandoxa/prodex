use crate::MojoError;

const WEBSOCKET_LOOP_CONTROL_ABI_VERSION: i64 = 1;
const WEBSOCKET_LOOP_CONTROL_FIELD_COUNT: usize = 10;
const WEBSOCKET_SESSION_CONTROL_ABI_VERSION: i64 = 1;

const WEBSOCKET_FRAME_TEXT: i64 = 0;
const WEBSOCKET_FRAME_BINARY: i64 = 1;
const WEBSOCKET_FRAME_PING: i64 = 2;
const WEBSOCKET_FRAME_PONG: i64 = 3;
const WEBSOCKET_FRAME_RAW: i64 = 4;
const WEBSOCKET_FRAME_CLOSE: i64 = 5;
const WEBSOCKET_FRAME_READ_ERROR: i64 = 6;

const WEBSOCKET_READ_CONTINUE: i64 = 0;
const WEBSOCKET_READ_BREAK: i64 = 1;
const WEBSOCKET_READ_RETURN: i64 = 2;
const WEBSOCKET_READ_ERROR: i64 = 3;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketErrorKind {
    ConnectionClosed = 0,
    AlreadyClosed = 1,
    BrokenPipe = 2,
    ConnectionAborted = 3,
    ConnectionReset = 4,
    NotConnected = 5,
    UnexpectedEof = 6,
    ResetWithoutClosingHandshake = 7,
    Other = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketReadFrame {
    Text,
    Binary,
    Ping,
    Pong,
    Raw,
    Close,
    ReadError { local_disconnect: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketReadAction {
    Continue,
    Break,
    Return,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketPrecommitBudgetInput {
    pub fresh_retry_pending: bool,
    pub budget_exhausted: bool,
    pub attempts: usize,
    pub continuation: bool,
    pub saw_overload_failure: bool,
    pub saw_rate_limit_failure: bool,
    pub saw_transport_failure: bool,
    pub route_has_retryable_profile: bool,
    pub recovery_sweeps: usize,
    pub profile_count: usize,
    pub attempt_limit: usize,
}

unsafe extern "C" {
    fn prodex_runtime_websocket_precommit_exhausted_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_websocket_local_disconnect_v1(abi_version: i64, error_kind: i64) -> i64;
    fn prodex_runtime_websocket_read_action_v1(
        abi_version: i64,
        frame_kind: i64,
        realtime_duplex: i64,
        has_socket: i64,
        local_disconnect: i64,
        output_address: u64,
    ) -> i64;
}

/// Applies websocket-specific precommit retry exhaustion after Rust gathers runtime state.
pub fn websocket_precommit_budget_exhausted(
    input: WebsocketPrecommitBudgetInput,
) -> Result<bool, MojoError> {
    let usize_to_u64 = |value| u64::try_from(value).unwrap_or(u64::MAX);
    let fields = [
        u64::from(input.fresh_retry_pending),
        u64::from(input.budget_exhausted),
        usize_to_u64(input.attempts),
        u64::from(input.continuation),
        u64::from(input.saw_overload_failure),
        u64::from(input.saw_rate_limit_failure),
        u64::from(input.saw_transport_failure),
        u64::from(input.route_has_retryable_profile),
        usize_to_u64(input.recovery_sweeps),
        usize_to_u64(if input.recovery_sweeps == 0 {
            input.profile_count
        } else {
            input.attempt_limit
        }),
    ];
    let mut output = i64::MIN;
    let status = unsafe {
        prodex_runtime_websocket_precommit_exhausted_v1(
            WEBSOCKET_LOOP_CONTROL_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            i64::try_from(WEBSOCKET_LOOP_CONTROL_FIELD_COUNT)
                .map_err(|_| MojoError::InvalidInput)?,
            &mut output as *mut i64 as usize as u64,
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

pub fn websocket_local_disconnect(kind: WebsocketErrorKind) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_websocket_local_disconnect_v1(
            WEBSOCKET_SESSION_CONTROL_ABI_VERSION,
            kind as i64,
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        -2 => Err(MojoError::InvalidInput),
        -3 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn websocket_read_action(
    frame: WebsocketReadFrame,
    realtime_duplex: bool,
    has_socket: bool,
) -> Result<WebsocketReadAction, MojoError> {
    let (frame_kind, local_disconnect) = match frame {
        WebsocketReadFrame::Text => (WEBSOCKET_FRAME_TEXT, false),
        WebsocketReadFrame::Binary => (WEBSOCKET_FRAME_BINARY, false),
        WebsocketReadFrame::Ping => (WEBSOCKET_FRAME_PING, false),
        WebsocketReadFrame::Pong => (WEBSOCKET_FRAME_PONG, false),
        WebsocketReadFrame::Raw => (WEBSOCKET_FRAME_RAW, false),
        WebsocketReadFrame::Close => (WEBSOCKET_FRAME_CLOSE, false),
        WebsocketReadFrame::ReadError { local_disconnect } => {
            (WEBSOCKET_FRAME_READ_ERROR, local_disconnect)
        }
    };
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_websocket_read_action_v1(
            WEBSOCKET_SESSION_CONTROL_ABI_VERSION,
            frame_kind,
            i64::from(realtime_duplex),
            i64::from(has_socket),
            i64::from(local_disconnect),
            &mut output as *mut i64 as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        WEBSOCKET_READ_CONTINUE => Ok(WebsocketReadAction::Continue),
        WEBSOCKET_READ_BREAK => Ok(WebsocketReadAction::Break),
        WEBSOCKET_READ_RETURN => Ok(WebsocketReadAction::Return),
        WEBSOCKET_READ_ERROR => Ok(WebsocketReadAction::Error),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only specification, independent from ABI implementation. Not a
    // runtime fallback: production invokes only the Mojo policy.
    fn expected(input: WebsocketPrecommitBudgetInput) -> bool {
        if input.fresh_retry_pending {
            return false;
        }
        if !input.continuation
            && (input.saw_overload_failure
                || input.saw_rate_limit_failure
                || input.saw_transport_failure)
            && input.route_has_retryable_profile
        {
            return false;
        }
        if input.recovery_sweeps == 0 {
            if input.saw_transport_failure
                && !input.continuation
                && !input.budget_exhausted
                && input.attempts < input.profile_count
            {
                return false;
            }
            return input.budget_exhausted;
        }
        input.attempts >= input.attempt_limit
    }

    #[test]
    fn precommit_budget_mojo_preserves_retry_precedence_all_boolean_states() {
        for flags in 0_u8..128 {
            for sweeps in [0, 1, 2] {
                for attempts in [0, 1, 2, 3, 5, 9, usize::MAX] {
                    for bound in [0, 1, 2, 4, 8, usize::MAX] {
                        let input = WebsocketPrecommitBudgetInput {
                            fresh_retry_pending: flags & 1 != 0,
                            budget_exhausted: flags & 2 != 0,
                            attempts,
                            continuation: flags & 4 != 0,
                            saw_overload_failure: flags & 8 != 0,
                            saw_rate_limit_failure: flags & 16 != 0,
                            saw_transport_failure: flags & 32 != 0,
                            route_has_retryable_profile: flags & 64 != 0,
                            recovery_sweeps: sweeps,
                            profile_count: bound,
                            attempt_limit: bound,
                        };
                        assert_eq!(
                            websocket_precommit_budget_exhausted(input),
                            Ok(expected(input)),
                            "flags={flags:07b}, sweeps={sweeps}, attempts={attempts}, bound={bound}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn precommit_budget_rejects_nonboolean_abi_fields() {
        let mut fields = [0_u64; WEBSOCKET_LOOP_CONTROL_FIELD_COUNT];
        let mut output = -1_i64;
        for index in [0, 1, 3, 4, 5, 6, 7] {
            fields[index] = 2;
            assert_eq!(
                unsafe {
                    prodex_runtime_websocket_precommit_exhausted_v1(
                        WEBSOCKET_LOOP_CONTROL_ABI_VERSION,
                        fields.as_ptr() as usize as u64,
                        WEBSOCKET_LOOP_CONTROL_FIELD_COUNT as i64,
                        &mut output as *mut i64 as usize as u64,
                    )
                },
                1,
                "nonboolean field {index} must fail closed",
            );
            fields[index] = 0;
        }
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_precommit_exhausted_v1(
                    0,
                    fields.as_ptr() as usize as u64,
                    WEBSOCKET_LOOP_CONTROL_FIELD_COUNT as i64,
                    &mut output as *mut i64 as usize as u64,
                )
            },
            4,
        );
    }

    #[test]
    fn websocket_read_action_covers_frames_keepalive_and_realtime_return() {
        assert_eq!(
            websocket_read_action(WebsocketReadFrame::Text, false, false),
            Ok(WebsocketReadAction::Continue)
        );
        assert_eq!(
            websocket_read_action(WebsocketReadFrame::Text, true, true),
            Ok(WebsocketReadAction::Return)
        );
        for frame in [
            WebsocketReadFrame::Binary,
            WebsocketReadFrame::Ping,
            WebsocketReadFrame::Pong,
            WebsocketReadFrame::Raw,
        ] {
            assert_eq!(
                websocket_read_action(frame, true, true),
                Ok(WebsocketReadAction::Continue)
            );
        }
        assert_eq!(
            websocket_read_action(WebsocketReadFrame::Close, false, false),
            Ok(WebsocketReadAction::Break)
        );
        assert_eq!(
            websocket_read_action(
                WebsocketReadFrame::ReadError {
                    local_disconnect: true,
                },
                false,
                false,
            ),
            Ok(WebsocketReadAction::Break)
        );
        assert_eq!(
            websocket_read_action(
                WebsocketReadFrame::ReadError {
                    local_disconnect: false,
                },
                false,
                false,
            ),
            Ok(WebsocketReadAction::Error)
        );
    }

    #[test]
    fn websocket_local_disconnect_covers_terminal_and_transient_errors() {
        for kind in [
            WebsocketErrorKind::ConnectionClosed,
            WebsocketErrorKind::AlreadyClosed,
            WebsocketErrorKind::BrokenPipe,
            WebsocketErrorKind::ConnectionAborted,
            WebsocketErrorKind::ConnectionReset,
            WebsocketErrorKind::NotConnected,
            WebsocketErrorKind::UnexpectedEof,
            WebsocketErrorKind::ResetWithoutClosingHandshake,
        ] {
            assert_eq!(websocket_local_disconnect(kind), Ok(true));
        }
        assert_eq!(
            websocket_local_disconnect(WebsocketErrorKind::Other),
            Ok(false)
        );
    }

    #[test]
    fn websocket_read_action_rejects_invalid_abi_inputs() {
        let mut output = -1_i64;
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_read_action_v1(
                    WEBSOCKET_SESSION_CONTROL_ABI_VERSION,
                    99,
                    0,
                    0,
                    0,
                    &mut output as *mut i64 as usize as u64,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_read_action_v1(
                    WEBSOCKET_SESSION_CONTROL_ABI_VERSION,
                    WEBSOCKET_FRAME_TEXT,
                    2,
                    0,
                    0,
                    &mut output as *mut i64 as usize as u64,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_local_disconnect_v1(
                    WEBSOCKET_SESSION_CONTROL_ABI_VERSION,
                    99,
                )
            },
            -2
        );
    }
}
