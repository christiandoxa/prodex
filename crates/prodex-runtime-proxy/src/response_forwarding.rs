use std::collections::BTreeSet;

use crate::{RuntimeTokenUsage, runtime_sse_consume_chunk, runtime_sse_finish_pending};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponseForwardingBodyKind {
    Unary,
    Sse,
}

impl RuntimeResponseForwardingBodyKind {
    pub fn is_sse(self) -> bool {
        matches!(self, Self::Sse)
    }

    pub fn as_log_label(self) -> &'static str {
        match self {
            Self::Unary => "unary",
            Self::Sse => "sse",
        }
    }
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
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-encoding"
            | "content-length"
            | "date"
            | "server"
            | "transfer-encoding"
    )
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
    headers
        .into_iter()
        .filter_map(|(name, value)| runtime_forward_text_response_header(name, value))
        .collect()
}

pub fn runtime_forward_binary_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Vec<(String, Vec<u8>)> {
    headers
        .into_iter()
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
    if content_type.is_some_and(|value| value.contains("text/event-stream")) {
        RuntimeResponseForwardingBodyKind::Sse
    } else {
        RuntimeResponseForwardingBodyKind::Unary
    }
}

pub fn runtime_response_content_type_is_sse(content_type: Option<&str>) -> bool {
    runtime_response_forwarding_body_kind(content_type).is_sse()
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
    match event_type {
        None => true,
        Some("response.completed" | "response.failed") => true,
        Some(kind) => kind.ends_with(".completed"),
    }
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
        self.log_token_usage(event.event_type.as_deref(), event.token_usage, effects);
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
        effects: &mut Vec<RuntimeSseTapEffect>,
    ) {
        if !runtime_token_usage_event_is_loggable(event_type) {
            return;
        }
        let Some(token_usage) = token_usage else {
            return;
        };
        if self.logged_token_usage.insert(token_usage) {
            effects.push(RuntimeSseTapEffect::LogTokenUsage(token_usage));
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/crates/prodex-runtime-proxy/src/response_forwarding.rs"]
mod tests;
