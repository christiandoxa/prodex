use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use crate::{
    RuntimeTokenUsage, runtime_connection_header_tokens,
    runtime_header_name_matches_connection_token, runtime_sse_consume_chunk,
    runtime_sse_finish_pending,
};

unsafe extern "C" {
    fn prodex_runtime_response_forwarding_classify_v1(
        operation: i64,
        address: u64,
        length: i64,
        present: i64,
        numeric: u64,
    ) -> i64;
}

fn response_forwarding_mojo_bool(operation: i64, value: Option<&str>, numeric: u64) -> bool {
    let present = value.is_some();
    let value = value.unwrap_or_default();
    let result = unsafe {
        prodex_runtime_response_forwarding_classify_v1(
            operation,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).unwrap_or(i64::MAX),
            i64::from(present),
            numeric,
        )
    };
    assert!(
        matches!(result, 0 | 1),
        "Mojo response-forwarding classifier returned invalid output"
    );
    result == 1
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponseForwardingBodyKind {
    Unary,
    Sse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBufferedResponseMetadata<'a> {
    pub status: u16,
    pub content_type: Option<&'a str>,
    pub body_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSseForwardingCommitDetail {
    pub prelude_bytes: usize,
    pub response_id_count: usize,
}

pub fn should_skip_runtime_response_header(name: &str) -> bool {
    response_forwarding_mojo_bool(0, Some(name), 0)
}

pub fn runtime_forward_text_response_header(name: &str, value: &str) -> Option<(String, String)> {
    (!should_skip_runtime_response_header(name)).then(|| (name.to_string(), value.to_string()))
}

pub fn runtime_forward_binary_response_header(
    name: &str,
    value: &[u8],
) -> Option<(String, Vec<u8>)> {
    (!should_skip_runtime_response_header(name)).then(|| (name.to_string(), value.to_vec()))
}

pub fn runtime_forward_text_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<(String, String)> {
    let headers = headers.into_iter().collect::<Vec<_>>();
    let connection_headers = runtime_connection_header_tokens(headers.iter().copied());
    headers
        .into_iter()
        .filter(|(name, _)| {
            !runtime_header_name_matches_connection_token(name, &connection_headers)
        })
        .filter_map(|(name, value)| runtime_forward_text_response_header(name, value))
        .collect()
}

pub fn runtime_forward_binary_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Vec<(String, Vec<u8>)> {
    let headers = headers.into_iter().collect::<Vec<_>>();
    let connection_headers =
        runtime_connection_header_tokens(headers.iter().filter_map(|(name, value)| {
            std::str::from_utf8(value).ok().map(|value| (*name, value))
        }));
    headers
        .into_iter()
        .filter(|(name, _)| {
            !runtime_header_name_matches_connection_token(name, &connection_headers)
        })
        .filter_map(|(name, value)| runtime_forward_binary_response_header(name, value))
        .collect()
}

pub fn runtime_response_content_type_from_binary_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Option<&'a str> {
    headers.into_iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub fn runtime_response_forwarding_body_kind(
    content_type: Option<&str>,
) -> RuntimeResponseForwardingBodyKind {
    if runtime_response_content_type_is_sse(content_type) {
        RuntimeResponseForwardingBodyKind::Sse
    } else {
        RuntimeResponseForwardingBodyKind::Unary
    }
}

pub fn runtime_response_content_type_is_sse(content_type: Option<&str>) -> bool {
    response_forwarding_mojo_bool(1, content_type, 0)
}

pub fn runtime_response_header_value<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    name: &str,
) -> Option<String> {
    headers
        .into_iter()
        .find(|(candidate_name, _)| candidate_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn runtime_stream_response_should_flush_each_chunk<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> bool {
    headers.into_iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            && runtime_response_content_type_is_sse(Some(value))
    })
}

pub fn runtime_buffered_response_metadata<'a>(
    status: u16,
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    body_bytes: usize,
) -> RuntimeBufferedResponseMetadata<'a> {
    RuntimeBufferedResponseMetadata {
        status,
        content_type: runtime_response_content_type_from_binary_headers(headers),
        body_bytes,
    }
}

pub fn runtime_sse_forwarding_commit_detail(
    prelude_bytes: usize,
    response_id_count: usize,
) -> RuntimeSseForwardingCommitDetail {
    RuntimeSseForwardingCommitDetail {
        prelude_bytes,
        response_id_count,
    }
}

pub fn runtime_token_usage_event_is_loggable(event_type: Option<&str>) -> bool {
    response_forwarding_mojo_bool(2, event_type, 0)
}

/// Returns whether a Responses event marks the first model-generation phase.
///
/// Queueing, response headers, and time-to-first-token are intentionally excluded. The
/// returned boundary is used only for output-throughput timing; it does not affect commit or
/// retry decisions.
pub fn runtime_response_event_is_generation_start(event_type: Option<&str>) -> bool {
    response_forwarding_mojo_bool(3, event_type, 0)
}

/// Measures elapsed generation time with a positive millisecond floor.
pub fn runtime_generation_elapsed_ms(started_at: Option<Instant>) -> Option<u64> {
    started_at.and_then(|started_at| started_at.elapsed().as_millis().max(1).try_into().ok())
}

const RUNTIME_LIVE_USAGE_LOG_INTERVAL: Duration = Duration::from_millis(250);

/// Throttles cumulative output-token snapshots before logging them for live viewers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTokenUsageProgress {
    last_output_tokens: Option<u64>,
    last_logged_at: Option<Instant>,
}

impl RuntimeTokenUsageProgress {
    /// Returns a new positive cumulative snapshot at most four times per second.
    pub fn observe(
        &mut self,
        usage: RuntimeTokenUsage,
        observed_at: Instant,
    ) -> Option<RuntimeTokenUsage> {
        if usage.output_tokens == 0
            || self
                .last_output_tokens
                .is_some_and(|last| usage.output_tokens <= last)
        {
            return None;
        }
        self.last_output_tokens = Some(usage.output_tokens);
        if self.last_logged_at.is_some_and(|last| {
            observed_at.saturating_duration_since(last) < RUNTIME_LIVE_USAGE_LOG_INTERVAL
        }) {
            return None;
        }
        self.last_logged_at = Some(observed_at);
        Some(usage)
    }
}

pub fn runtime_token_usage_event_is_live(
    event_type: Option<&str>,
    token_usage: Option<RuntimeTokenUsage>,
) -> bool {
    runtime_response_event_is_generation_start(event_type)
        && token_usage.is_some_and(|usage| usage.output_tokens > 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSseTapEffect {
    RememberResponseIds {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    ClearDeadResponseBindings {
        response_ids: Vec<String>,
    },
    LogTokenUsage(RuntimeTokenUsage),
    LogTokenUsageProgress {
        usage: RuntimeTokenUsage,
        generation_ms: u64,
    },
    LogTokenUsageWithGeneration {
        usage: RuntimeTokenUsage,
        generation_ms: u64,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeSseTapStateInit<'a> {
    pub remembered_response_ids: &'a [String],
    pub request_previous_response_id: Option<&'a str>,
    pub turn_state: Option<&'a str>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSseTapState {
    line: Vec<u8>,
    data_lines: Vec<String>,
    remembered_response_ids: BTreeSet<String>,
    response_ids_with_turn_state: BTreeSet<String>,
    logged_token_usage: BTreeSet<RuntimeTokenUsage>,
    output_token_usage_progress: RuntimeTokenUsageProgress,
    generation_started_at: Option<Instant>,
    turn_state: Option<String>,
    request_previous_response_id: Option<String>,
}

impl RuntimeSseTapState {
    pub fn new(init: RuntimeSseTapStateInit<'_>) -> Self {
        Self {
            remembered_response_ids: init.remembered_response_ids.iter().cloned().collect(),
            response_ids_with_turn_state: init
                .turn_state
                .map(|_| init.remembered_response_ids.iter().cloned().collect())
                .unwrap_or_default(),
            turn_state: init.turn_state.map(str::to_string),
            request_previous_response_id: init.request_previous_response_id.map(str::to_string),
            ..Self::default()
        }
    }

    pub fn observe_chunk(&mut self, chunk: &[u8]) -> Vec<RuntimeSseTapEffect> {
        let mut effects = Vec::new();
        let mut line = std::mem::take(&mut self.line);
        let mut data_lines = std::mem::take(&mut self.data_lines);
        runtime_sse_consume_chunk(&mut line, &mut data_lines, chunk, |event| {
            self.observe_stream_event(event, &mut effects);
        });
        self.line = line;
        self.data_lines = data_lines;
        effects
    }

    pub fn finish_pending(&mut self) -> Vec<RuntimeSseTapEffect> {
        let mut effects = Vec::new();
        let mut line = std::mem::take(&mut self.line);
        let mut data_lines = std::mem::take(&mut self.data_lines);
        runtime_sse_finish_pending(&mut line, &mut data_lines, |event| {
            self.observe_stream_event(event, &mut effects);
        });
        self.line = line;
        self.data_lines = data_lines;
        effects
    }

    fn observe_stream_event(
        &mut self,
        event: crate::RuntimeParsedSseEvent,
        effects: &mut Vec<RuntimeSseTapEffect>,
    ) {
        if let Some(turn_state) = event.turn_state {
            self.turn_state = Some(turn_state);
        }
        self.remember_response_ids(&event.response_ids, effects);
        if event.previous_response_not_found {
            effects.push(RuntimeSseTapEffect::ClearDeadResponseBindings {
                response_ids: self.dead_chain_response_ids(),
            });
        }
        let event_type = event.event_type.as_deref();
        if self.generation_started_at.is_none()
            && runtime_response_event_is_generation_start(event_type)
        {
            self.generation_started_at = Some(Instant::now());
        }
        let final_generation_ms = (event_type == Some("response.completed"))
            .then(|| runtime_generation_elapsed_ms(self.generation_started_at))
            .flatten();
        if runtime_token_usage_event_is_live(event_type, event.token_usage)
            && let Some(token_usage) = event.token_usage
            && let Some(token_usage) = self
                .output_token_usage_progress
                .observe(token_usage, Instant::now())
            && let Some(generation_ms) = runtime_generation_elapsed_ms(self.generation_started_at)
        {
            effects.push(RuntimeSseTapEffect::LogTokenUsageProgress {
                usage: token_usage,
                generation_ms,
            });
        }
        self.log_token_usage(event_type, event.token_usage, final_generation_ms, effects);
    }

    fn remember_response_ids(
        &mut self,
        response_ids: &[String],
        effects: &mut Vec<RuntimeSseTapEffect>,
    ) {
        let turn_state = self.turn_state.clone();
        let mut fresh_ids = Vec::new();
        for response_id in response_ids {
            if self.remembered_response_ids.contains(response_id.as_str()) {
                continue;
            }
            let fresh_id = response_id.clone();
            self.remembered_response_ids.insert(fresh_id.clone());
            if turn_state.is_some() {
                self.response_ids_with_turn_state.insert(fresh_id.clone());
            }
            fresh_ids.push(fresh_id);
        }

        let mut response_ids_needing_turn_state = Vec::new();
        if turn_state.is_some()
            && self.response_ids_with_turn_state.len() < self.remembered_response_ids.len()
        {
            for response_id in &self.remembered_response_ids {
                if self
                    .response_ids_with_turn_state
                    .contains(response_id.as_str())
                {
                    continue;
                }
                let rebound_id = response_id.clone();
                self.response_ids_with_turn_state.insert(rebound_id.clone());
                response_ids_needing_turn_state.push(rebound_id);
            }
        }

        if !fresh_ids.is_empty() {
            effects.push(RuntimeSseTapEffect::RememberResponseIds {
                response_ids: fresh_ids,
                turn_state: turn_state.clone(),
            });
        }
        if !response_ids_needing_turn_state.is_empty() {
            effects.push(RuntimeSseTapEffect::RememberResponseIds {
                response_ids: response_ids_needing_turn_state,
                turn_state,
            });
        }
    }

    fn dead_chain_response_ids(&self) -> Vec<String> {
        let mut dead_response_ids = self
            .remembered_response_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(previous_response_id) = self.request_previous_response_id.as_deref() {
            dead_response_ids.push(previous_response_id.to_string());
        }
        dead_response_ids
    }

    fn log_token_usage(
        &mut self,
        event_type: Option<&str>,
        token_usage: Option<RuntimeTokenUsage>,
        generation_ms: Option<u64>,
        effects: &mut Vec<RuntimeSseTapEffect>,
    ) {
        let Some(token_usage) = token_usage else {
            return;
        };
        if runtime_token_usage_event_is_loggable(event_type)
            && self.logged_token_usage.insert(token_usage)
        {
            if let Some(generation_ms) = generation_ms {
                effects.push(RuntimeSseTapEffect::LogTokenUsageWithGeneration {
                    usage: token_usage,
                    generation_ms,
                });
            } else {
                effects.push(RuntimeSseTapEffect::LogTokenUsage(token_usage));
            }
        }
    }
}

#[cfg(test)]
mod classifier_tests {
    use super::*;

    #[test]
    fn response_header_skip_expected_values_include_unicode_trimming() {
        for (name, expected) in [
            ("Connection", true),
            (" content-length ", true),
            ("\u{00a0}Transfer-Encoding\u{2003}", true),
            ("\u{2003}x-codex-turn-state\u{00a0}", false),
            ("Keep-Alive", true),
            ("TRANSFER-ENCODING", true),
            ("content length", false),
            ("\u{200b}Upgrade", false),
            ("", false),
        ] {
            assert_eq!(
                should_skip_runtime_response_header(name),
                expected,
                "header {name:?}"
            );
        }

        assert_eq!(
            runtime_forward_text_response_header("\u{00a0}Content-Length\u{2003}", "99"),
            None
        );
        assert_eq!(
            runtime_forward_text_response_header("x-codex-turn-state", " turn-state "),
            Some(("x-codex-turn-state".to_string(), " turn-state ".to_string()))
        );
    }

    #[test]
    fn response_header_classifier_rejects_invalid_utf8_at_abi_boundary() {
        let malformed = [0xc3_u8, 0x28];
        assert_eq!(
            unsafe {
                prodex_runtime_response_forwarding_classify_v1(
                    0,
                    malformed.as_ptr() as usize as u64,
                    malformed.len() as i64,
                    1,
                    0,
                )
            },
            -1
        );
    }

    #[test]
    fn sse_content_type_expected_values_and_body_boundary() {
        for (content_type, expected) in [
            (None, false),
            (Some(""), false),
            (Some("application/json"), false),
            (Some(" text/event-stream; charset=utf-8 "), true),
            (Some("TEXT/EVENT-STREAM"), true),
            (Some("prefix text/event-stream suffix"), true),
            (Some("世界 TEXT/EVENT-STREAM"), true),
            (Some("text/event-streaming"), true),
        ] {
            assert_eq!(
                runtime_response_content_type_is_sse(content_type),
                expected,
                "content type {content_type:?}"
            );
            assert_eq!(
                runtime_response_forwarding_body_kind(content_type),
                if expected {
                    RuntimeResponseForwardingBodyKind::Sse
                } else {
                    RuntimeResponseForwardingBodyKind::Unary
                }
            );
        }

        assert!(runtime_stream_response_should_flush_each_chunk([(
            "Content-Type",
            "TEXT/EVENT-STREAM; charset=utf-8"
        )]));
        assert!(!runtime_stream_response_should_flush_each_chunk([(
            "Content-Type",
            "application/json"
        )]));
    }

    #[test]
    fn token_usage_loggability_keeps_absent_exact_and_suffix_cases() {
        for (event_type, expected) in [
            (None, true),
            (Some("response.completed"), true),
            (Some("response.failed"), true),
            (Some("tool.completed"), true),
            (Some("é.completed"), true),
            (Some(".completed"), true),
            (Some("completed"), false),
            (Some("response.delta"), false),
            (Some("response.completed.extra"), false),
            (Some("RESPONSE.COMPLETED"), false),
            (Some(""), false),
        ] {
            assert_eq!(
                runtime_token_usage_event_is_loggable(event_type),
                expected,
                "event loggability {event_type:?}"
            );
        }
    }

    #[test]
    fn generation_start_expected_values_drive_live_usage_boundary() {
        for event_type in [
            "response.output_text.delta",
            "response.refusal.delta",
            "response.reasoning_summary_text.delta",
            "response.reasoning_text.delta",
            "response.function_call_arguments.delta",
            "response.mcp_call_arguments.delta",
            "response.custom_tool_call_input.delta",
        ] {
            assert!(
                runtime_response_event_is_generation_start(Some(event_type)),
                "generation start {event_type:?}"
            );
            assert!(runtime_token_usage_event_is_live(
                Some(event_type),
                Some(RuntimeTokenUsage {
                    output_tokens: 1,
                    ..RuntimeTokenUsage::default()
                })
            ));
        }

        for event_type in [
            "response.output_item.added",
            "response.output_text.delta.extra",
            "prefix.response.output_text.delta",
            "response.created",
            "",
        ] {
            assert!(
                !runtime_response_event_is_generation_start(Some(event_type)),
                "not a generation start: {event_type:?}"
            );
        }
        assert!(!runtime_response_event_is_generation_start(None));
        assert!(!runtime_token_usage_event_is_live(
            Some("response.output_text.delta"),
            Some(RuntimeTokenUsage::default())
        ));
    }
}

#[cfg(test)]
#[path = "../tests/src/response_forwarding.rs"]
mod tests;
