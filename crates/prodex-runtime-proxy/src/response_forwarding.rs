use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use std::collections::BTreeSet;
use std::time::Instant;

use crate::{
    RuntimeHttpErrorAction, RuntimeHttpErrorClass, RuntimeTokenUsage, runtime_sse_consume_chunk,
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
    fn prodex_runtime_response_forwarding_header_v1(
        name_address: u64,
        name_length: i64,
        value_address: u64,
        value_length: i64,
        value_present: i64,
    ) -> i64;
    fn prodex_runtime_response_forwarding_content_type_v1(
        name_address: u64,
        name_length: i64,
        value_address: u64,
        value_length: i64,
    ) -> i64;
    fn prodex_runtime_response_forwarding_attempt_v1(
        status: i64,
        class_tag: i64,
        action_tag: i64,
        retryable_previous: i64,
        token_invalidated: i64,
        committed: i64,
    ) -> i64;
    fn prodex_runtime_token_usage_progress_plan_v1(
        abi_version: i64,
        output_tokens: u64,
        last_output_present: i64,
        last_output_tokens: u64,
        last_log_present: i64,
        elapsed_since_last_log_ms: u64,
    ) -> i64;
}

const RESPONSE_FORWARDING_PRECOMMIT_ATTEMPT: i64 = 9;
const RESPONSE_FORWARDING_TAP_PLAN: i64 = 10;

const RESPONSE_FORWARDING_HEADER_FORWARD: i64 = 0;
const RESPONSE_FORWARDING_HEADER_SKIP: i64 = 1;
const RESPONSE_FORWARDING_HEADER_CONNECTION_TOKEN: i64 = 2;
const RESPONSE_FORWARDING_HEADER_CONNECTION: i64 = 3;

const RESPONSE_FORWARDING_ATTEMPT_SUCCESS: i64 = 0;
const RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILED: i64 = 1;
const RESPONSE_FORWARDING_ATTEMPT_PROFILE_UNAVAILABLE: i64 = 2;
const RESPONSE_FORWARDING_ATTEMPT_QUOTA_RETRY: i64 = 3;
const RESPONSE_FORWARDING_ATTEMPT_RATE_LIMITED: i64 = 4;
const RESPONSE_FORWARDING_ATTEMPT_OVERLOADED: i64 = 5;
const RESPONSE_FORWARDING_ATTEMPT_PREVIOUS_RESPONSE_NOT_FOUND: i64 = 6;
const RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILURE_NOTICE: i64 = 7;

fn runtime_sse_tap_plan(event_type: Option<&str>, output_tokens: u64) -> u8 {
    let value = event_type.unwrap_or_default();
    let result = unsafe {
        prodex_runtime_response_forwarding_classify_v1(
            RESPONSE_FORWARDING_TAP_PLAN,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).unwrap_or(i64::MAX),
            i64::from(event_type.is_some()),
            output_tokens,
        )
    };
    assert!(
        (0..=15).contains(&result),
        "Mojo response-forwarding tap planner returned invalid output"
    );
    result as u8
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

fn response_forwarding_mojo_tag(operation: i64, numeric: u64) -> i64 {
    let result =
        unsafe { prodex_runtime_response_forwarding_classify_v1(operation, 0, 0, 0, numeric) };
    assert!(
        (0..=8).contains(&result),
        "Mojo response-forwarding planner returned invalid output"
    );
    result
}

fn runtime_response_forwarding_header_action(name: &str, value: Option<&[u8]>) -> i64 {
    let present = value.is_some();
    let value = value.unwrap_or_default();
    let result = unsafe {
        prodex_runtime_response_forwarding_header_v1(
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).unwrap_or(i64::MAX),
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).unwrap_or(i64::MAX),
            i64::from(present),
        )
    };
    assert!(
        matches!(
            result,
            RESPONSE_FORWARDING_HEADER_FORWARD
                | RESPONSE_FORWARDING_HEADER_SKIP
                | RESPONSE_FORWARDING_HEADER_CONNECTION_TOKEN
                | RESPONSE_FORWARDING_HEADER_CONNECTION
        ),
        "Mojo response-forwarding header planner returned invalid output"
    );
    result
}

fn runtime_response_forwarding_header_is_connection(name: &str) -> bool {
    runtime_response_forwarding_header_action(name, None) == RESPONSE_FORWARDING_HEADER_CONNECTION
}

fn runtime_response_forwarding_header_is_connection_token(
    name: &str,
    connection_values: &[&[u8]],
) -> bool {
    connection_values.iter().any(|value| {
        runtime_response_forwarding_header_action(name, Some(value))
            == RESPONSE_FORWARDING_HEADER_CONNECTION_TOKEN
    })
}

fn runtime_response_content_type_header_is_usable(name: &str, value: &[u8]) -> bool {
    let result = unsafe {
        prodex_runtime_response_forwarding_content_type_v1(
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).unwrap_or(i64::MAX),
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).unwrap_or(i64::MAX),
        )
    };
    assert!(
        matches!(result, 0 | 1),
        "Mojo response-forwarding content-type planner returned invalid output"
    );
    result == 1
}

/// Mojo-owned pre-commit response outcome; Rust applies the returned effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponsesPrecommitAttemptPlan {
    Success { note_auth_failure: bool },
    AuthFailed,
    QuotaBlocked,
    RateLimited,
    Overloaded,
    PreviousResponseNotFound,
}

/// Plans one buffered Responses failure without touching affinity or runtime state.
pub fn runtime_responses_precommit_attempt_plan(
    status: u16,
    error_class: RuntimeHttpErrorClass,
    error_action: RuntimeHttpErrorAction,
    retryable_previous: bool,
    token_invalidated: bool,
    committed: bool,
) -> RuntimeResponsesPrecommitAttemptPlan {
    let class_tag = match error_class {
        RuntimeHttpErrorClass::Quota => 1,
        RuntimeHttpErrorClass::RateLimited => 2,
        RuntimeHttpErrorClass::ProfileUnavailable => 3,
        RuntimeHttpErrorClass::Overload => 4,
        RuntimeHttpErrorClass::TransientServer => 5,
        RuntimeHttpErrorClass::Other => 0,
    };
    let action_tag = match error_action {
        RuntimeHttpErrorAction::PassThrough => 0,
        RuntimeHttpErrorAction::RotateProfile => 1,
        RuntimeHttpErrorAction::RetryProfile => 2,
    };
    let numeric = u64::from(status)
        | (class_tag << 16)
        | (action_tag << 19)
        | (u64::from(retryable_previous) << 21)
        | (u64::from(token_invalidated) << 22)
        | (u64::from(committed) << 23);
    match response_forwarding_mojo_tag(RESPONSE_FORWARDING_PRECOMMIT_ATTEMPT, numeric) {
        0 => RuntimeResponsesPrecommitAttemptPlan::Success {
            note_auth_failure: false,
        },
        1 => RuntimeResponsesPrecommitAttemptPlan::AuthFailed,
        2 => RuntimeResponsesPrecommitAttemptPlan::QuotaBlocked,
        3 => RuntimeResponsesPrecommitAttemptPlan::RateLimited,
        4 => RuntimeResponsesPrecommitAttemptPlan::Overloaded,
        5 => RuntimeResponsesPrecommitAttemptPlan::PreviousResponseNotFound,
        8 => RuntimeResponsesPrecommitAttemptPlan::Success {
            note_auth_failure: true,
        },
        _ => unreachable!("validated Mojo response attempt plan"),
    }
}

/// Plans the terminal outcome for a standard or compact buffered response.
/// Rust keeps ownership of the response body and applies this decision to the
/// route-specific attempt state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponseForwardingAttemptPlan {
    Success { note_auth_failure: bool },
    AuthFailed,
    ProfileUnavailable,
    QuotaRetry,
    RateLimited,
    Overloaded,
    PreviousResponseNotFound,
}

pub fn runtime_response_forwarding_attempt_plan(
    status: u16,
    error_class: RuntimeHttpErrorClass,
    error_action: RuntimeHttpErrorAction,
    retryable_previous: bool,
    token_invalidated: bool,
    committed: bool,
) -> RuntimeResponseForwardingAttemptPlan {
    let class_tag = match error_class {
        RuntimeHttpErrorClass::Quota => 1,
        RuntimeHttpErrorClass::RateLimited => 2,
        RuntimeHttpErrorClass::ProfileUnavailable => 3,
        RuntimeHttpErrorClass::Overload => 4,
        RuntimeHttpErrorClass::TransientServer => 5,
        RuntimeHttpErrorClass::Other => 0,
    };
    let action_tag = match error_action {
        RuntimeHttpErrorAction::PassThrough => 0,
        RuntimeHttpErrorAction::RotateProfile => 1,
        RuntimeHttpErrorAction::RetryProfile => 2,
    };
    let result = unsafe {
        prodex_runtime_response_forwarding_attempt_v1(
            i64::from(status),
            class_tag,
            action_tag,
            i64::from(retryable_previous),
            i64::from(token_invalidated),
            i64::from(committed),
        )
    };
    match result {
        RESPONSE_FORWARDING_ATTEMPT_SUCCESS => RuntimeResponseForwardingAttemptPlan::Success {
            note_auth_failure: false,
        },
        RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILED => RuntimeResponseForwardingAttemptPlan::AuthFailed,
        RESPONSE_FORWARDING_ATTEMPT_PROFILE_UNAVAILABLE => {
            RuntimeResponseForwardingAttemptPlan::ProfileUnavailable
        }
        RESPONSE_FORWARDING_ATTEMPT_QUOTA_RETRY => RuntimeResponseForwardingAttemptPlan::QuotaRetry,
        RESPONSE_FORWARDING_ATTEMPT_RATE_LIMITED => {
            RuntimeResponseForwardingAttemptPlan::RateLimited
        }
        RESPONSE_FORWARDING_ATTEMPT_OVERLOADED => RuntimeResponseForwardingAttemptPlan::Overloaded,
        RESPONSE_FORWARDING_ATTEMPT_PREVIOUS_RESPONSE_NOT_FOUND => {
            RuntimeResponseForwardingAttemptPlan::PreviousResponseNotFound
        }
        RESPONSE_FORWARDING_ATTEMPT_AUTH_FAILURE_NOTICE => {
            RuntimeResponseForwardingAttemptPlan::Success {
                note_auth_failure: true,
            }
        }
        _ => panic!("Mojo response-forwarding attempt planner returned invalid output: {result}"),
    }
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
    runtime_response_forwarding_header_action(name, None) != RESPONSE_FORWARDING_HEADER_FORWARD
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
    let connection_values = headers
        .iter()
        .filter(|(name, _)| runtime_response_forwarding_header_is_connection(name))
        .map(|(_, value)| value.as_bytes())
        .collect::<Vec<_>>();
    headers
        .into_iter()
        .filter(|(name, _)| {
            !runtime_response_forwarding_header_is_connection_token(name, &connection_values)
        })
        .filter_map(|(name, value)| runtime_forward_text_response_header(name, value))
        .collect()
}

pub fn runtime_forward_binary_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Vec<(String, Vec<u8>)> {
    let headers = headers.into_iter().collect::<Vec<_>>();
    let connection_values = headers
        .iter()
        .filter(|(name, _)| runtime_response_forwarding_header_is_connection(name))
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    headers
        .into_iter()
        .filter(|(name, _)| {
            !runtime_response_forwarding_header_is_connection_token(name, &connection_values)
        })
        .filter_map(|(name, value)| runtime_forward_binary_response_header(name, value))
        .collect()
}

pub fn runtime_response_content_type_from_binary_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Option<&'a str> {
    headers.into_iter().find_map(|(name, value)| {
        runtime_response_content_type_header_is_usable(name, value)
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

/// Keep Responses precommit inspection when a streaming request receives no MIME
/// declaration. Explicit response types stay authoritative; headers are unmodified.
pub fn runtime_responses_should_inspect_sse(
    content_type: Option<&str>,
    request_streaming: bool,
) -> bool {
    response_forwarding_mojo_bool(11, content_type, u64::from(request_streaming))
}

pub fn runtime_response_header_value<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    name: &str,
) -> Option<String> {
    headers
        .into_iter()
        .find(|(candidate_name, _)| {
            ascii_casefold_equal_exact(candidate_name, name)
                .expect("Mojo response header-name comparison failed")
        })
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn runtime_stream_response_should_flush_each_chunk<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> bool {
    headers.into_iter().any(|(name, value)| {
        ascii_casefold_equal_exact(name, "content-type")
            .expect("Mojo response Content-Type comparison failed")
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

pub fn runtime_websocket_terminal_should_reset(
    event_type: Option<&str>,
    realtime_websocket: bool,
) -> bool {
    response_forwarding_mojo_bool(4, event_type, u64::from(realtime_websocket))
}

pub fn runtime_response_ids_should_record(precommit_hold: bool) -> bool {
    response_forwarding_mojo_bool(5, None, u64::from(precommit_hold))
}

pub fn runtime_committed_previous_response_not_found(
    committed: bool,
    previous_response_not_found: bool,
) -> bool {
    response_forwarding_mojo_bool(
        6,
        None,
        u64::from(committed) | (u64::from(previous_response_not_found) << 1),
    )
}

pub fn runtime_response_generation_should_start(
    event_type: Option<&str>,
    already_started: bool,
) -> bool {
    response_forwarding_mojo_bool(7, event_type, u64::from(already_started))
}

/// Measures elapsed generation time with a positive millisecond floor.
pub fn runtime_generation_elapsed_ms(started_at: Option<Instant>) -> Option<u64> {
    started_at.and_then(|started_at| started_at.elapsed().as_millis().max(1).try_into().ok())
}

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
        let elapsed_since_last_log_ms = self
            .last_logged_at
            .map(|last| observed_at.saturating_duration_since(last).as_millis())
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(u64::MAX);
        let action = unsafe {
            prodex_runtime_token_usage_progress_plan_v1(
                1,
                usage.output_tokens,
                i64::from(self.last_output_tokens.is_some()),
                self.last_output_tokens.unwrap_or_default(),
                i64::from(self.last_logged_at.is_some()),
                elapsed_since_last_log_ms,
            )
        };
        match action {
            0 => None,
            1 => {
                self.last_output_tokens = Some(usage.output_tokens);
                None
            }
            2 => {
                self.last_output_tokens = Some(usage.output_tokens);
                self.last_logged_at = Some(observed_at);
                Some(usage)
            }
            _ => panic!("Mojo token-usage progress planner returned invalid output"),
        }
    }
}

pub fn runtime_token_usage_event_is_live(
    event_type: Option<&str>,
    token_usage: Option<RuntimeTokenUsage>,
) -> bool {
    response_forwarding_mojo_bool(
        8,
        event_type,
        token_usage.map_or(0, |usage| usage.output_tokens),
    )
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
        let tap_plan = runtime_sse_tap_plan(
            event_type,
            event.token_usage.map_or(0, |usage| usage.output_tokens),
        );
        if self.generation_started_at.is_none() && tap_plan & 1 != 0 {
            self.generation_started_at = Some(Instant::now());
        }
        let final_generation_ms = (tap_plan & 2 != 0)
            .then(|| runtime_generation_elapsed_ms(self.generation_started_at))
            .flatten();
        if tap_plan & 4 != 0
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
        self.log_token_usage(tap_plan, event.token_usage, final_generation_ms, effects);
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
        tap_plan: u8,
        token_usage: Option<RuntimeTokenUsage>,
        generation_ms: Option<u64>,
        effects: &mut Vec<RuntimeSseTapEffect>,
    ) {
        let Some(token_usage) = token_usage else {
            return;
        };
        if tap_plan & 8 != 0 && self.logged_token_usage.insert(token_usage) {
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
#[path = "response_forwarding/classifier_tests.rs"]
mod classifier_tests;

#[cfg(test)]
#[path = "../tests/src/response_forwarding.rs"]
mod tests;
