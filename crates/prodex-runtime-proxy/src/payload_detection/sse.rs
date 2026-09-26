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
const RUNTIME_SSE_LINE_DATA: i64 = 2;
const RUNTIME_SSE_INSPECTION_CONTINUE: i64 = 0;
const RUNTIME_SSE_INSPECTION_QUOTA_BLOCKED: i64 = 1;
const RUNTIME_SSE_INSPECTION_RATE_LIMITED: i64 = 2;
const RUNTIME_SSE_INSPECTION_OVERLOADED: i64 = 3;
const RUNTIME_SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND: i64 = 4;

unsafe extern "C" {
    fn prodex_runtime_sse_line_plan_v1(address: u64, length: i64, output_address: u64) -> i64;
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

fn runtime_sse_line_plan(line: &[u8]) -> (i64, usize, usize) {
    let mut output = [0_i64; 3];
    let status = unsafe {
        prodex_runtime_sse_line_plan_v1(
            line.as_ptr() as usize as u64,
            i64::try_from(line.len()).expect("SSE line length exceeds Mojo ABI"),
            output.as_mut_ptr() as usize as u64,
        )
    };
    assert_eq!(status, 0, "Mojo SSE line planner returned invalid status");
    assert!((0..=2).contains(&output[0]), "Mojo SSE line tag is invalid");
    let start = usize::try_from(output[1]).expect("validated Mojo SSE value start");
    let end = usize::try_from(output[2]).expect("validated Mojo SSE value end");
    assert!(
        start <= end && end <= line.len(),
        "Mojo SSE line span is invalid"
    );
    (output[0], start, end)
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

fn runtime_sse_event_marked_invalid(data_lines: &[String]) -> bool {
    matches!(
        data_lines.first().map(String::as_str),
        Some(RUNTIME_SSE_INVALID_DATA_MARKER)
    )
}

fn runtime_sse_mark_invalid(data_lines: &mut Vec<String>) {
    data_lines.clear();
    data_lines.push(RUNTIME_SSE_INVALID_DATA_MARKER.to_string());
}

type RuntimeSseEventParser = fn(&[String]) -> RuntimeParsedSseEvent;

fn runtime_sse_emit_event<F>(
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    if data_lines.is_empty() {
        return;
    }
    if runtime_sse_event_marked_invalid(data_lines) {
        data_lines.clear();
        return;
    }
    on_event(parse_event(data_lines));
    data_lines.clear();
}

fn runtime_sse_finish_line<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    let (kind, value_start, value_end) = runtime_sse_line_plan(line);
    if kind == RUNTIME_SSE_LINE_BLANK {
        runtime_sse_emit_event(data_lines, parse_event, on_event);
    } else if kind == RUNTIME_SSE_LINE_DATA {
        match std::str::from_utf8(&line[value_start..value_end]) {
            Ok(text) if !runtime_sse_event_marked_invalid(data_lines) => {
                data_lines.push(text.to_owned());
            }
            Ok(_) => {}
            Err(_) => runtime_sse_mark_invalid(data_lines),
        }
    }
    line.clear();
}
pub fn runtime_sse_consume_chunk<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    chunk: &[u8],
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    for byte in chunk {
        line.push(*byte);
        if *byte == b'\n' {
            runtime_sse_finish_line(line, data_lines, parse_runtime_sse_event, &mut on_event);
        }
    }
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
    for byte in chunk {
        line.push(*byte);
        if *byte == b'\n' {
            runtime_sse_finish_line(
                line,
                data_lines,
                runtime_sse_inspection_event,
                &mut on_event,
            );
        }
    }
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
    response_ids: std::collections::BTreeSet<String>,
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
        self.response_ids.extend(event.response_ids);
        if event.turn_state.is_some() {
            self.turn_state = event.turn_state;
        }
        self.saw_commit_ready_event = committed;
        None
    }

    fn progress(self) -> RuntimeSseInspectionProgress {
        if self.saw_commit_ready_event {
            RuntimeSseInspectionProgress::Commit {
                response_ids: self.response_ids.into_iter().collect(),
                turn_state: self.turn_state,
            }
        } else {
            RuntimeSseInspectionProgress::Hold {
                response_ids: self.response_ids.into_iter().collect(),
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
    if data_lines.is_empty() || runtime_sse_event_marked_invalid(data_lines) {
        return None;
    }

    let payload = data_lines.join("\n");
    let payload = payload.trim_start_matches('\u{feff}');
    serde_json::from_str::<serde_json::Value>(payload).ok()
}

pub fn parse_runtime_sse_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    let Some(value) = parse_runtime_sse_payload(data_lines) else {
        return RuntimeParsedSseEvent::default();
    };

    let error_policy =
        runtime_stream_error_policy_from_value(&value, RuntimeHttpErrorPhase::PreCommit);
    RuntimeParsedSseEvent {
        quota_blocked: error_policy.action == RuntimeHttpErrorAction::RotateProfile
            && error_policy.class == RuntimeHttpErrorClass::Quota,
        rate_limited: error_policy.action == RuntimeHttpErrorAction::RetryProfile
            && error_policy.class == RuntimeHttpErrorClass::RateLimited,
        overloaded: error_policy.action == RuntimeHttpErrorAction::RetryProfile
            && matches!(
                error_policy.class,
                RuntimeHttpErrorClass::Overload | RuntimeHttpErrorClass::TransientServer
            )
            || (error_policy.action == RuntimeHttpErrorAction::RotateProfile
                && error_policy.class == RuntimeHttpErrorClass::ProfileUnavailable),
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
        type Case = (&'static [u8], (i64, usize, usize));
        let cases: &[Case] = &[
            (b"\n", (RUNTIME_SSE_LINE_BLANK, 0, 0)),
            (b"\r\n", (RUNTIME_SSE_LINE_BLANK, 0, 0)),
            (b": ping\n", (1, 6, 6)),
            (b"data\n", (RUNTIME_SSE_LINE_DATA, 4, 4)),
            (b"data:\n", (RUNTIME_SSE_LINE_DATA, 5, 5)),
            (b"data: hello\r\n", (RUNTIME_SSE_LINE_DATA, 6, 11)),
            (b"data:  hello\n", (RUNTIME_SSE_LINE_DATA, 6, 12)),
            (b"event: message\n", (1, 14, 14)),
            (b"database: nope\n", (1, 14, 14)),
            (b"data:\xff\n", (RUNTIME_SSE_LINE_DATA, 5, 6)),
        ];
        for (line, expected) in cases {
            assert_eq!(runtime_sse_line_plan(line), *expected, "line={line:?}");
        }
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
