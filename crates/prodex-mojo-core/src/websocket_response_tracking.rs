use crate::MojoError;

const ABI_VERSION: i64 = 1;
const FIELD_COUNT: usize = 10;
const OUTPUT_COUNT: usize = 8;

pub const FRAME_TEXT: i64 = 0;
pub const FRAME_BINARY: i64 = 1;
pub const FRAME_KEEPALIVE: i64 = 2;

pub const RETRY_NONE: i64 = 0;
pub const RETRY_CONNECTION_LIMIT: i64 = 1;
pub const RETRY_QUOTA: i64 = 2;
pub const RETRY_RATE_LIMITED: i64 = 3;
pub const RETRY_OVERLOADED: i64 = 4;
pub const RETRY_PREVIOUS_RESPONSE_NOT_FOUND: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketResponseFrameAction {
    Forward,
    ForwardUncommitted,
    Buffer,
    CommitBuffered,
    RetryConnectionLimit,
    RetryQuota,
    RetryRateLimited,
    RetryOverloaded,
    RetryPreviousResponseNotFound,
    Keepalive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketResponseFrameInput<'a> {
    pub frame_kind: i64,
    pub event_type: Option<&'a str>,
    pub text_nonempty: bool,
    pub committed: bool,
    pub precommit_hold: bool,
    pub promoted_precommit_hold: bool,
    pub retry_kind: i64,
    pub terminal_hint: bool,
    pub realtime_websocket: bool,
    pub generation_started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketResponseFramePlan {
    pub action: WebsocketResponseFrameAction,
    pub commit: bool,
    pub forward: bool,
    pub terminal: bool,
    pub reset_upstream_socket: bool,
    pub record_response_ids: bool,
    pub generation_start: bool,
    pub committed_previous_response_not_found: bool,
}

unsafe extern "C" {
    fn prodex_websocket_response_frame_plan_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        event_address: u64,
        event_length: i64,
        event_present: i64,
        output_address: u64,
        output_count: i64,
    ) -> i64;
}

fn map_action(value: i64) -> Result<WebsocketResponseFrameAction, MojoError> {
    match value {
        0 => Ok(WebsocketResponseFrameAction::Forward),
        1 => Ok(WebsocketResponseFrameAction::ForwardUncommitted),
        2 => Ok(WebsocketResponseFrameAction::Buffer),
        3 => Ok(WebsocketResponseFrameAction::CommitBuffered),
        4 => Ok(WebsocketResponseFrameAction::RetryConnectionLimit),
        5 => Ok(WebsocketResponseFrameAction::RetryQuota),
        6 => Ok(WebsocketResponseFrameAction::RetryRateLimited),
        7 => Ok(WebsocketResponseFrameAction::RetryOverloaded),
        8 => Ok(WebsocketResponseFrameAction::RetryPreviousResponseNotFound),
        9 => Ok(WebsocketResponseFrameAction::Keepalive),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

/// Plans one WebSocket frame without touching transport or runtime state.
pub fn websocket_response_frame_plan(
    input: WebsocketResponseFrameInput<'_>,
) -> Result<WebsocketResponseFramePlan, MojoError> {
    let event_type = input.event_type.unwrap_or_default();
    let fields = [
        u64::from(input.committed),
        u64::from(input.precommit_hold),
        u64::from(input.promoted_precommit_hold),
        u64::try_from(input.retry_kind).map_err(|_| MojoError::InvalidInput)?,
        u64::from(input.terminal_hint),
        u64::from(input.realtime_websocket),
        u64::from(input.generation_started),
        u64::from(input.text_nonempty),
        u64::from(input.event_type.is_some()),
        u64::try_from(input.frame_kind).map_err(|_| MojoError::InvalidInput)?,
    ];
    let mut output = [-1_i64; OUTPUT_COUNT];
    let status = unsafe {
        prodex_websocket_response_frame_plan_v1(
            ABI_VERSION,
            fields.as_ptr() as usize as u64,
            i64::try_from(FIELD_COUNT).map_err(|_| MojoError::InvalidInput)?,
            event_type.as_ptr() as usize as u64,
            signed_len(event_type)?,
            i64::from(input.event_type.is_some()),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(OUTPUT_COUNT).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let bool_output = |value: i64| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketResponseFramePlan {
        action: map_action(output[0])?,
        commit: bool_output(output[1])?,
        forward: bool_output(output[2])?,
        terminal: bool_output(output[3])?,
        reset_upstream_socket: bool_output(output[4])?,
        record_response_ids: bool_output(output[5])?,
        generation_start: bool_output(output[6])?,
        committed_previous_response_not_found: bool_output(output[7])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(
        frame_kind: i64,
        event_type: Option<&'a str>,
        committed: bool,
        precommit_hold: bool,
        promoted_precommit_hold: bool,
        retry_kind: i64,
    ) -> WebsocketResponseFrameInput<'a> {
        WebsocketResponseFrameInput {
            frame_kind,
            event_type,
            text_nonempty: event_type.is_some(),
            committed,
            precommit_hold,
            promoted_precommit_hold,
            retry_kind,
            terminal_hint: matches!(
                event_type,
                Some("error" | "response.failed" | "response.completed")
            ),
            realtime_websocket: false,
            generation_started: false,
        }
    }

    fn expected_uncommitted(
        input: &WebsocketResponseFrameInput<'_>,
        generation_start: bool,
    ) -> Option<WebsocketResponseFramePlan> {
        let retry_action = match input.retry_kind {
            RETRY_CONNECTION_LIMIT => Some(WebsocketResponseFrameAction::RetryConnectionLimit),
            RETRY_QUOTA => Some(WebsocketResponseFrameAction::RetryQuota),
            RETRY_RATE_LIMITED => Some(WebsocketResponseFrameAction::RetryRateLimited),
            RETRY_OVERLOADED => Some(WebsocketResponseFrameAction::RetryOverloaded),
            RETRY_PREVIOUS_RESPONSE_NOT_FOUND => {
                Some(WebsocketResponseFrameAction::RetryPreviousResponseNotFound)
            }
            RETRY_NONE => None,
            _ => panic!("test input uses an invalid retry tag"),
        };
        if let Some(action) = retry_action {
            return Some(WebsocketResponseFramePlan {
                action,
                commit: false,
                forward: false,
                terminal: false,
                reset_upstream_socket: false,
                record_response_ids: false,
                generation_start,
                committed_previous_response_not_found: false,
            });
        }
        if input.precommit_hold {
            return Some(WebsocketResponseFramePlan {
                action: if input.promoted_precommit_hold {
                    WebsocketResponseFrameAction::CommitBuffered
                } else {
                    WebsocketResponseFrameAction::Buffer
                },
                commit: input.promoted_precommit_hold,
                forward: false,
                terminal: false,
                reset_upstream_socket: false,
                record_response_ids: false,
                generation_start,
                committed_previous_response_not_found: false,
            });
        }
        if !input.text_nonempty || input.event_type.is_none() {
            return Some(WebsocketResponseFramePlan {
                action: WebsocketResponseFrameAction::ForwardUncommitted,
                commit: false,
                forward: true,
                terminal: false,
                reset_upstream_socket: false,
                record_response_ids: false,
                generation_start,
                committed_previous_response_not_found: false,
            });
        }
        None
    }

    fn expected(input: WebsocketResponseFrameInput<'_>) -> WebsocketResponseFramePlan {
        let generation_start = !input.generation_started
            && matches!(
                input.event_type,
                Some(
                    "response.output_text.delta"
                        | "response.refusal.delta"
                        | "response.reasoning_summary_text.delta"
                        | "response.reasoning_text.delta"
                        | "response.function_call_arguments.delta"
                        | "response.mcp_call_arguments.delta"
                        | "response.custom_tool_call_input.delta"
                )
            );
        if input.frame_kind == FRAME_KEEPALIVE {
            return WebsocketResponseFramePlan {
                action: WebsocketResponseFrameAction::Keepalive,
                commit: false,
                forward: false,
                terminal: false,
                reset_upstream_socket: false,
                record_response_ids: false,
                generation_start,
                committed_previous_response_not_found: false,
            };
        }
        if input.frame_kind == FRAME_BINARY {
            return WebsocketResponseFramePlan {
                action: WebsocketResponseFrameAction::Forward,
                commit: !input.committed,
                forward: true,
                terminal: false,
                reset_upstream_socket: false,
                record_response_ids: false,
                generation_start: false,
                committed_previous_response_not_found: false,
            };
        }
        if !input.committed
            && let Some(plan) = expected_uncommitted(&input, generation_start)
        {
            return plan;
        }
        let terminal = input.terminal_hint;
        WebsocketResponseFramePlan {
            action: WebsocketResponseFrameAction::Forward,
            commit: !input.committed,
            forward: true,
            terminal,
            reset_upstream_socket: terminal
                && !input.realtime_websocket
                && matches!(
                    input.event_type,
                    Some("error" | "response.failed" | "response.incomplete")
                ),
            record_response_ids: !input.precommit_hold,
            generation_start,
            committed_previous_response_not_found: input.committed
                && input.retry_kind == RETRY_PREVIOUS_RESPONSE_NOT_FOUND,
        }
    }

    #[test]
    fn frame_sequence_keeps_prelude_and_keepalive_out_of_commit() {
        let sequence = [
            input(FRAME_KEEPALIVE, None, false, false, false, RETRY_NONE),
            input(
                FRAME_TEXT,
                Some("response.created"),
                false,
                true,
                false,
                RETRY_NONE,
            ),
            input(
                FRAME_TEXT,
                Some("response.output_text.delta"),
                false,
                false,
                false,
                RETRY_NONE,
            ),
            input(
                FRAME_TEXT,
                Some("response.completed"),
                true,
                false,
                false,
                RETRY_NONE,
            ),
            input(
                FRAME_TEXT,
                Some("response.completed"),
                true,
                false,
                false,
                RETRY_NONE,
            ),
        ];
        let expected_actions = [
            WebsocketResponseFrameAction::Keepalive,
            WebsocketResponseFrameAction::Buffer,
            WebsocketResponseFrameAction::Forward,
            WebsocketResponseFrameAction::Forward,
            WebsocketResponseFrameAction::Forward,
        ];
        let mut committed = false;
        for (frame, expected_action) in sequence.into_iter().zip(expected_actions) {
            let plan =
                websocket_response_frame_plan(WebsocketResponseFrameInput { committed, ..frame })
                    .expect("Mojo frame planner should accept sequence");
            assert_eq!(
                plan,
                expected(WebsocketResponseFrameInput { committed, ..frame })
            );
            assert_eq!(plan.action, expected_action);
            committed |= plan.commit;
        }
        assert!(committed);
    }

    #[test]
    fn frame_plan_matches_independent_boundary_spec() {
        for committed in [false, true] {
            for precommit_hold in [false, true] {
                for promoted_precommit_hold in [false, true] {
                    for retry_kind in RETRY_NONE..=RETRY_PREVIOUS_RESPONSE_NOT_FOUND {
                        for (event_type, text_nonempty) in [
                            (None, false),
                            (None, true),
                            (Some("response.created"), true),
                        ] {
                            let input = WebsocketResponseFrameInput {
                                frame_kind: FRAME_TEXT,
                                event_type,
                                text_nonempty,
                                committed,
                                precommit_hold,
                                promoted_precommit_hold,
                                retry_kind,
                                terminal_hint: false,
                                realtime_websocket: false,
                                generation_started: false,
                            };
                            assert_eq!(
                                websocket_response_frame_plan(input),
                                Ok(expected(input)),
                                "committed={committed} hold={precommit_hold} promoted={promoted_precommit_hold} retry={retry_kind} event={event_type:?} text_nonempty={text_nonempty}",
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn terminal_and_retry_precedence_are_fail_closed() {
        let terminal = websocket_response_frame_plan(WebsocketResponseFrameInput {
            frame_kind: FRAME_TEXT,
            event_type: Some("response.failed"),
            text_nonempty: true,
            committed: true,
            precommit_hold: false,
            promoted_precommit_hold: false,
            retry_kind: RETRY_NONE,
            terminal_hint: true,
            realtime_websocket: false,
            generation_started: false,
        })
        .expect("terminal frame plan");
        assert!(terminal.terminal);
        assert!(terminal.reset_upstream_socket);
        assert!(!terminal.commit);

        let retry = websocket_response_frame_plan(WebsocketResponseFrameInput {
            frame_kind: FRAME_TEXT,
            event_type: Some("response.failed"),
            text_nonempty: true,
            committed: false,
            precommit_hold: false,
            promoted_precommit_hold: false,
            retry_kind: RETRY_QUOTA,
            terminal_hint: true,
            realtime_websocket: false,
            generation_started: false,
        })
        .expect("precommit retry plan");
        assert_eq!(retry.action, WebsocketResponseFrameAction::RetryQuota);
        assert!(!retry.commit);
        assert!(!retry.forward);
        assert!(!retry.terminal);
    }

    #[test]
    fn abi_rejects_wrong_version_and_invalid_utf8() {
        let fields = [0_u64; FIELD_COUNT];
        let mut output = [0_i64; OUTPUT_COUNT];
        let event = [0xff_u8];
        assert_eq!(
            unsafe {
                prodex_websocket_response_frame_plan_v1(
                    0,
                    fields.as_ptr() as usize as u64,
                    FIELD_COUNT as i64,
                    event.as_ptr() as usize as u64,
                    1,
                    1,
                    output.as_mut_ptr() as usize as u64,
                    OUTPUT_COUNT as i64,
                )
            },
            4
        );
        assert_eq!(
            unsafe {
                prodex_websocket_response_frame_plan_v1(
                    ABI_VERSION,
                    fields.as_ptr() as usize as u64,
                    FIELD_COUNT as i64,
                    event.as_ptr() as usize as u64,
                    1,
                    1,
                    output.as_mut_ptr() as usize as u64,
                    OUTPUT_COUNT as i64,
                )
            },
            1
        );
    }
}
