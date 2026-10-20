use super::{
    RuntimeTokenUsage, extract_runtime_proxy_previous_response_message_from_value,
    extract_runtime_response_ids_from_value, extract_runtime_token_usage_from_value,
    extract_runtime_turn_state_from_value, runtime_proxy_value_is_invalid_previous_response_id,
    runtime_response_event_type_from_value,
};
use crate::{
    RuntimeHttpErrorAction, RuntimeHttpErrorClass, RuntimeHttpErrorPhase,
    runtime_stream_error_policy_from_value,
};
use prodex_mojo_core::runtime_sse_chunk::{
    SseEventStateAction, SseEventStateOperation, sse_event_state_action,
};
use std::time::Duration;

const RUNTIME_SSE_INVALID_DATA_MARKER: &str = "\u{0}prodex-invalid-sse-data";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeParsedSseEvent {
    pub quota_blocked: bool,
    pub rate_limited: bool,
    pub overloaded: bool,
    pub previous_response_not_found: bool,
    pub invalid_previous_response_id: bool,
    pub response_ids: Vec<String>,
    pub event_type: Option<String>,
    pub turn_state: Option<String>,
    pub token_usage: Option<RuntimeTokenUsage>,
    pub retry_after: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSseInspectionProgress {
    Hold {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    Commit {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    QuotaBlocked,
    RateLimited {
        retry_after: Option<Duration>,
    },
    Overloaded,
    PreviousResponseNotFound,
}

const RUNTIME_SSE_LINE_BLANK: i64 = 0;
const RUNTIME_SSE_LINE_IGNORE: i64 = 1;
const RUNTIME_SSE_LINE_DATA: i64 = 2;
const RUNTIME_SSE_INSPECTION_CONTINUE: i64 = 0;
const RUNTIME_SSE_INSPECTION_QUOTA_BLOCKED: i64 = 1;
const RUNTIME_SSE_INSPECTION_RATE_LIMITED: i64 = 2;
const RUNTIME_SSE_INSPECTION_OVERLOADED: i64 = 3;
const RUNTIME_SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND: i64 = 4;

unsafe extern "C" {
    fn prodex_runtime_sse_line_plan_v1(address: u64, length: i64, output_address: u64) -> i64;
    fn prodex_runtime_sse_event_byte_plan_v1(
        current_bytes: u64,
        data_bytes: u64,
        max_bytes: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_sse_inspection_step_v1(
        committed: i64,
        quota_blocked: i64,
        rate_limited: i64,
        overloaded: i64,
        previous_response_not_found: i64,
        precommit_hold: i64,
        output_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSseLineKind {
    Blank,
    Ignore,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSseLinePlan {
    pub kind: RuntimeSseLineKind,
    pub value_start: usize,
    pub value_end: usize,
}

pub fn runtime_sse_line_plan(line: &[u8]) -> RuntimeSseLinePlan {
    let mut output = [0_i64; 3];
    let status = unsafe {
        prodex_runtime_sse_line_plan_v1(
            line.as_ptr() as usize as u64,
            i64::try_from(line.len()).expect("SSE line length exceeds Mojo ABI"),
            output.as_mut_ptr() as usize as u64,
        )
    };
    assert_eq!(status, 0, "Mojo SSE line planner returned invalid status");
    let kind = match output[0] {
        RUNTIME_SSE_LINE_BLANK => RuntimeSseLineKind::Blank,
        RUNTIME_SSE_LINE_IGNORE => RuntimeSseLineKind::Ignore,
        RUNTIME_SSE_LINE_DATA => RuntimeSseLineKind::Data,
        _ => panic!("Mojo SSE line tag is invalid"),
    };
    let value_start = usize::try_from(output[1]).expect("validated Mojo SSE value start");
    let value_end = usize::try_from(output[2]).expect("validated Mojo SSE value end");
    assert!(
        value_start <= value_end && value_end <= line.len(),
        "Mojo SSE line span is invalid"
    );
    RuntimeSseLinePlan {
        kind,
        value_start,
        value_end,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSseEventBytePlan {
    pub next_bytes: usize,
    pub exceeds_limit: bool,
}

pub fn runtime_sse_event_byte_plan(
    current_bytes: usize,
    data_bytes: usize,
    max_bytes: usize,
) -> RuntimeSseEventBytePlan {
    let mut output = [0_u64; 2];
    let status = unsafe {
        prodex_runtime_sse_event_byte_plan_v1(
            u64::try_from(current_bytes).expect("SSE event bytes fit u64"),
            u64::try_from(data_bytes).expect("SSE data bytes fit u64"),
            u64::try_from(max_bytes).expect("SSE event limit fits u64"),
            output.as_mut_ptr() as usize as u64,
        )
    };
    assert_eq!(
        status, 0,
        "Mojo SSE event byte planner returned invalid status"
    );
    RuntimeSseEventBytePlan {
        next_bytes: usize::try_from(output[0]).unwrap_or(usize::MAX),
        exceeds_limit: match output[1] {
            0 => false,
            1 => true,
            _ => panic!("Mojo SSE event byte planner returned invalid flag"),
        },
    }
}

fn runtime_sse_inspection_step(
    committed: bool,
    event: &RuntimeParsedSseEvent,
    precommit_hold: bool,
) -> (i64, bool) {
    let mut output = [0_i64; 2];
    let status = unsafe {
        prodex_runtime_sse_inspection_step_v1(
            i64::from(committed),
            i64::from(event.quota_blocked),
            i64::from(event.rate_limited),
            i64::from(event.overloaded),
            i64::from(event.previous_response_not_found),
            i64::from(precommit_hold),
            output.as_mut_ptr() as usize as u64,
        )
    };
    assert_eq!(
        status, 0,
        "Mojo SSE inspection planner returned invalid status"
    );
    assert!(
        (RUNTIME_SSE_INSPECTION_CONTINUE..=RUNTIME_SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND)
            .contains(&output[0])
            && matches!(output[1], 0 | 1),
        "Mojo SSE inspection planner returned invalid output"
    );
    (output[0], output[1] == 1)
}

type RuntimeSseEventParser = fn(&[String]) -> RuntimeParsedSseEvent;

fn runtime_sse_emit_event<F>(
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    match sse_event_state_action(data_lines, SseEventStateOperation::Emit, true)
        .expect("compiled Mojo SSE buffered-event state returned invalid output")
    {
        SseEventStateAction::Empty => {}
        SseEventStateAction::Discard => data_lines.clear(),
        SseEventStateAction::Emit => {
            on_event(parse_event(data_lines));
            data_lines.clear();
        }
        _ => unreachable!("validated Mojo SSE emission state"),
    }
}

fn runtime_sse_finish_line<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    let plan = runtime_sse_line_plan(line);
    if plan.kind == RuntimeSseLineKind::Blank {
        runtime_sse_emit_event(data_lines, parse_event, on_event);
    } else if plan.kind == RuntimeSseLineKind::Data {
        let text = std::str::from_utf8(&line[plan.value_start..plan.value_end]);
        match sse_event_state_action(
            data_lines,
            SseEventStateOperation::IncomingData,
            text.is_ok(),
        )
        .expect("compiled Mojo SSE incoming data state returned invalid output")
        {
            SseEventStateAction::AppendIncoming => {
                data_lines.push(
                    text.expect("validated Mojo incoming UTF-8 state")
                        .to_owned(),
                );
            }
            SseEventStateAction::IgnoreIncoming => {}
            SseEventStateAction::MarkInvalid => {
                data_lines.clear();
                data_lines.push(RUNTIME_SSE_INVALID_DATA_MARKER.to_string());
            }
            _ => unreachable!("validated Mojo SSE incoming-data state"),
        }
    }
    line.clear();
}
fn runtime_sse_consume_chunk_with_parser<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    chunk: &[u8],
    parser: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    let mut cursor = 0;
    while cursor < chunk.len() {
        let plan = prodex_mojo_core::runtime_sse_chunk::newline_plan(&chunk[cursor..])
            .expect("Mojo SSE chunk boundary scanner returned invalid output");
        let consumed = plan.consumed;
        let mut start = 0;
        for end in plan.newlines {
            line.extend_from_slice(&chunk[cursor + start..cursor + end + 1]);
            runtime_sse_finish_line(line, data_lines, parser, on_event);
            start = end + 1;
        }
        line.extend_from_slice(&chunk[cursor + start..cursor + consumed]);
        cursor += consumed;
    }
}

pub fn runtime_sse_consume_chunk<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    chunk: &[u8],
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    runtime_sse_consume_chunk_with_parser(
        line,
        data_lines,
        chunk,
        parse_runtime_sse_event,
        &mut on_event,
    );
}

pub fn runtime_sse_finish_pending<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    runtime_sse_finish_pending_with_parser(
        line,
        data_lines,
        parse_runtime_sse_event,
        &mut on_event,
    );
}

fn runtime_sse_finish_pending_with_parser<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    if !line.is_empty() {
        runtime_sse_finish_line(line, data_lines, parse_event, on_event);
    }
    runtime_sse_emit_event(data_lines, parse_event, on_event);
}

fn runtime_sse_consume_inspection_buffer<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    chunk: &[u8],
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    runtime_sse_consume_chunk_with_parser(
        line,
        data_lines,
        chunk,
        runtime_sse_inspection_event,
        &mut on_event,
    );
}

pub fn inspect_runtime_sse_buffer(buffered: &[u8]) -> RuntimeSseInspectionProgress {
    inspect_runtime_sse_buffer_with_eof(buffered, false)
}

pub fn inspect_runtime_sse_buffer_at_eof(buffered: &[u8]) -> RuntimeSseInspectionProgress {
    inspect_runtime_sse_buffer_with_eof(buffered, true)
}

fn inspect_runtime_sse_buffer_with_eof(
    buffered: &[u8],
    at_eof: bool,
) -> RuntimeSseInspectionProgress {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut state = RuntimeSseInspectionState::default();
    let mut terminal = None;
    runtime_sse_consume_inspection_buffer(&mut line, &mut data_lines, buffered, |event| {
        record_runtime_sse_inspection_event(&mut state, &mut terminal, event);
    });
    if at_eof && terminal.is_none() {
        runtime_sse_finish_pending_with_parser(
            &mut line,
            &mut data_lines,
            runtime_sse_inspection_event,
            &mut |event| {
                record_runtime_sse_inspection_event(&mut state, &mut terminal, event);
            },
        );
    }
    terminal.unwrap_or_else(|| state.progress())
}

#[derive(Default)]
struct RuntimeSseInspectionState {
    response_ids: Vec<String>,
    saw_commit_ready_event: bool,
    turn_state: Option<String>,
}

impl RuntimeSseInspectionState {
    fn observe(&mut self, event: RuntimeParsedSseEvent) -> Option<RuntimeSseInspectionProgress> {
        let precommit_hold = event
            .event_type
            .as_deref()
            .is_some_and(crate::runtime_proxy_precommit_hold_event_kind);
        let (action, committed) =
            runtime_sse_inspection_step(self.saw_commit_ready_event, &event, precommit_hold);
        let terminal = match action {
            RUNTIME_SSE_INSPECTION_CONTINUE => None,
            RUNTIME_SSE_INSPECTION_QUOTA_BLOCKED => {
                Some(RuntimeSseInspectionProgress::QuotaBlocked)
            }
            RUNTIME_SSE_INSPECTION_RATE_LIMITED => {
                Some(RuntimeSseInspectionProgress::RateLimited {
                    retry_after: event.retry_after,
                })
            }
            RUNTIME_SSE_INSPECTION_OVERLOADED => Some(RuntimeSseInspectionProgress::Overloaded),
            RUNTIME_SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND => {
                Some(RuntimeSseInspectionProgress::PreviousResponseNotFound)
            }
            _ => unreachable!("validated Mojo SSE inspection action"),
        };
        if terminal.is_some() {
            return terminal;
        }
        // Empty ID events leave the already Mojo-sorted state unchanged.
        if !event.response_ids.is_empty() {
            let borrowed = self
                .response_ids
                .iter()
                .chain(event.response_ids.iter())
                .map(String::as_str)
                .collect::<Vec<_>>();
            let selected = prodex_mojo_core::runtime_sse_chunk::response_id_sort_unique(&borrowed)
                .expect("compiled Mojo SSE response-ID ordering returned invalid output");
            let sorted = selected
                .into_iter()
                .map(|index| borrowed[index].to_owned())
                .collect::<Vec<_>>();
            self.response_ids = sorted;
        }
        if event.turn_state.is_some() {
            self.turn_state = event.turn_state;
        }
        self.saw_commit_ready_event = committed;
        None
    }

    fn progress(self) -> RuntimeSseInspectionProgress {
        if self.saw_commit_ready_event {
            RuntimeSseInspectionProgress::Commit {
                response_ids: self.response_ids,
                turn_state: self.turn_state,
            }
        } else {
            RuntimeSseInspectionProgress::Hold {
                response_ids: self.response_ids,
                turn_state: self.turn_state,
            }
        }
    }
}

fn record_runtime_sse_inspection_event(
    state: &mut RuntimeSseInspectionState,
    terminal: &mut Option<RuntimeSseInspectionProgress>,
    event: RuntimeParsedSseEvent,
) {
    if terminal.is_none() {
        *terminal = state.observe(event);
    }
}

fn runtime_sse_inspection_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    parse_runtime_sse_event(data_lines)
}

pub fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    if sse_event_state_action(data_lines, SseEventStateOperation::ParsePayload, true)
        .expect("compiled Mojo SSE payload gate returned invalid output")
        != SseEventStateAction::Emit
    {
        return None;
    }

    let payload = prodex_mojo_core::runtime_sse_chunk::payload_text(data_lines)
        .expect("Mojo SSE payload assembler returned an invalid output");
    serde_json::from_str::<serde_json::Value>(&payload).ok()
}

pub fn parse_runtime_sse_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    let Some(value) = parse_runtime_sse_payload(data_lines) else {
        return RuntimeParsedSseEvent::default();
    };

    let error_policy =
        runtime_stream_error_policy_from_value(&value, RuntimeHttpErrorPhase::PreCommit);
    // Rust maps typed error tags into the ABI; Mojo exclusively owns the
    // rotate/retry-class matrix and its three precommit terminal flags.
    let class_tag = match error_policy.class {
        RuntimeHttpErrorClass::Other => 0,
        RuntimeHttpErrorClass::Quota => 1,
        RuntimeHttpErrorClass::RateLimited => 2,
        RuntimeHttpErrorClass::ProfileUnavailable => 3,
        RuntimeHttpErrorClass::Overload => 4,
        RuntimeHttpErrorClass::TransientServer => 5,
    };
    let action_tag = match error_policy.action {
        RuntimeHttpErrorAction::PassThrough => 0,
        RuntimeHttpErrorAction::RotateProfile => 1,
        RuntimeHttpErrorAction::RetryProfile => 2,
    };
    let flags = prodex_mojo_core::runtime_sse_chunk::failure_flags(class_tag, action_tag)
        .expect("Mojo SSE failure action matrix returned invalid output");
    RuntimeParsedSseEvent {
        quota_blocked: flags.quota_blocked,
        rate_limited: flags.rate_limited,
        overloaded: flags.overloaded,
        previous_response_not_found: extract_runtime_proxy_previous_response_message_from_value(
            &value,
        )
        .is_some(),
        invalid_previous_response_id: runtime_proxy_value_is_invalid_previous_response_id(&value),
        response_ids: extract_runtime_response_ids_from_value(&value),
        event_type: runtime_response_event_type_from_value(&value),
        turn_state: extract_runtime_turn_state_from_value(&value),
        token_usage: extract_runtime_token_usage_from_value(&value),
        retry_after: error_policy.retry_after,
    }
}

/// Detects the exact invalid incremental-response error in an SSE payload.
///
/// The ordinary body classifier intentionally parses JSON bodies only. Responses failures can
/// also arrive as `data:` SSE events, where treating this as the older generic
/// `previous_response_not_found` signal would incorrectly re-enter profile rotation.
pub fn runtime_sse_body_is_invalid_previous_response_id(body: &[u8]) -> bool {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut invalid = false;
    runtime_sse_consume_chunk(&mut line, &mut data_lines, body, |event| {
        invalid |= event.invalid_previous_response_id;
    });
    runtime_sse_finish_pending(&mut line, &mut data_lines, |event| {
        invalid |= event.invalid_previous_response_id;
    });
    invalid
}

#[cfg(test)]
mod planner_tests {
    use super::*;

    #[test]
    fn sse_line_planner_handles_sse_field_shapes() {
        type Case = (&'static [u8], RuntimeSseLineKind, usize, usize);
        let cases: &[Case] = &[
            (b"\n", RuntimeSseLineKind::Blank, 0, 0),
            (b"\r\n", RuntimeSseLineKind::Blank, 0, 0),
            (b": ping\n", RuntimeSseLineKind::Ignore, 6, 6),
            (b"data\n", RuntimeSseLineKind::Data, 4, 4),
            (b"data:\n", RuntimeSseLineKind::Data, 5, 5),
            (b"data: hello\r\n", RuntimeSseLineKind::Data, 6, 11),
            (b"data:  hello\n", RuntimeSseLineKind::Data, 6, 12),
            (b"event: message\n", RuntimeSseLineKind::Ignore, 14, 14),
            (b"database: nope\n", RuntimeSseLineKind::Ignore, 14, 14),
            (b"data:\xff\n", RuntimeSseLineKind::Data, 5, 6),
        ];
        for (line, kind, value_start, value_end) in cases {
            assert_eq!(
                runtime_sse_line_plan(line),
                RuntimeSseLinePlan {
                    kind: *kind,
                    value_start: *value_start,
                    value_end: *value_end,
                },
                "line={line:?}"
            );
        }
        assert_eq!(
            runtime_sse_event_byte_plan(7, 4, 12),
            RuntimeSseEventBytePlan {
                next_bytes: 12,
                exceeds_limit: false,
            }
        );
        assert!(runtime_sse_event_byte_plan(8, 4, 12).exceeds_limit);
    }

    #[test]
    fn sse_inspection_step_orders_precommit_terminal_signals() {
        let mut event = RuntimeParsedSseEvent {
            quota_blocked: true,
            rate_limited: true,
            overloaded: true,
            previous_response_not_found: true,
            ..RuntimeParsedSseEvent::default()
        };
        assert_eq!(
            runtime_sse_inspection_step(false, &event, true),
            (RUNTIME_SSE_INSPECTION_QUOTA_BLOCKED, false)
        );
        event.quota_blocked = false;
        assert_eq!(
            runtime_sse_inspection_step(false, &event, true),
            (RUNTIME_SSE_INSPECTION_RATE_LIMITED, false)
        );
        event.rate_limited = false;
        assert_eq!(
            runtime_sse_inspection_step(false, &event, true),
            (RUNTIME_SSE_INSPECTION_OVERLOADED, false)
        );
        event.overloaded = false;
        assert_eq!(
            runtime_sse_inspection_step(false, &event, true),
            (RUNTIME_SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND, false)
        );
        event.previous_response_not_found = false;
        assert_eq!(
            runtime_sse_inspection_step(false, &event, true),
            (RUNTIME_SSE_INSPECTION_CONTINUE, false)
        );
        assert_eq!(
            runtime_sse_inspection_step(false, &event, false),
            (RUNTIME_SSE_INSPECTION_CONTINUE, true)
        );
        event.quota_blocked = true;
        assert_eq!(
            runtime_sse_inspection_step(true, &event, true),
            (RUNTIME_SSE_INSPECTION_CONTINUE, true)
        );
    }
}
