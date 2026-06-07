use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::gemini_rewrite::runtime_gemini_normalized_response_value;
use std::io::{self, Read};
use std::sync::Arc;

#[path = "gemini_sse_state.rs"]
mod gemini_sse_state;

use gemini_sse_state::RuntimeGeminiSseState;
pub(super) use gemini_sse_state::runtime_gemini_forced_command_output;

pub(super) type RuntimeGeminiBindingRecorder = Arc<dyn Fn(String, Vec<String>) + Send + Sync>;

pub(super) struct RuntimeGeminiGenerateSseReader<R: Read> {
    reader: std::io::BufReader<R>,
    pending: Vec<u8>,
    pending_offset: usize,
    accumulated_json: String,
    state: RuntimeGeminiSseState,
}

impl<R: Read> RuntimeGeminiGenerateSseReader<R> {
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
    ) -> Self {
        Self {
            reader: std::io::BufReader::new(reader),
            pending: Vec::new(),
            pending_offset: 0,
            accumulated_json: String::new(),
            state: RuntimeGeminiSseState::new(
                request_id,
                conversation_messages,
                conversations,
                binding_recorder,
            ),
        }
    }

    fn fill_pending(&mut self) -> io::Result<()> {
        while self.pending_offset >= self.pending.len() && !self.state.eof {
            self.pending.clear();
            self.pending_offset = 0;
            let data_lines = match self.read_next_sse_data_lines() {
                Ok(data_lines) => data_lines,
                Err(err) => {
                    if let Some(event) = self
                        .state
                        .failed_event("provider_stream_error", &err.to_string())
                    {
                        self.pending = event.into_bytes();
                        self.state.eof = true;
                        break;
                    }
                    return Err(err);
                }
            };
            let Some(data_lines) = data_lines else {
                if !self.accumulated_json.trim().is_empty() {
                    if let Some(events) = self.flush_accumulated_json() {
                        self.pending = events.into_bytes();
                        continue;
                    }
                    if let Some(event) = self
                        .state
                        .failed_event("provider_stream_error", "incomplete JSON segment")
                    {
                        self.pending = event.into_bytes();
                    }
                    self.state.eof = true;
                    break;
                }
                if let Some(event) = self.state.complete_event() {
                    self.pending = event.into_bytes();
                }
                self.state.eof = true;
                break;
            };
            if data_lines.iter().any(|data| data.trim() == "[DONE]") {
                if !self.accumulated_json.trim().is_empty() {
                    if let Some(events) = self.flush_accumulated_json() {
                        self.pending.extend_from_slice(events.as_bytes());
                    } else if let Some(event) = self
                        .state
                        .failed_event("provider_stream_error", "incomplete JSON segment")
                    {
                        self.pending.extend_from_slice(event.as_bytes());
                        self.state.eof = true;
                        break;
                    }
                }
                if let Some(event) = self.state.complete_event() {
                    self.pending.extend_from_slice(event.as_bytes());
                }
                self.state.eof = true;
                break;
            }
            for event in self.observe_sse_data_lines(&data_lines) {
                self.pending.extend_from_slice(event.as_bytes());
            }
        }
        Ok(())
    }

    fn observe_sse_data_lines(&mut self, data_lines: &[String]) -> Vec<String> {
        let joined = data_lines.join("\n");
        if let Some(events) = self.observe_json_text(&joined) {
            return events;
        }

        let mut separate_values = Vec::new();
        for line in data_lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                separate_values.clear();
                break;
            };
            separate_values.push(value);
        }
        if !separate_values.is_empty() {
            let mut events = Vec::new();
            for value in separate_values {
                events.extend(self.observe_generate_value(&value));
            }
            return events;
        }

        self.accumulated_json.push_str(joined.trim());
        self.flush_accumulated_json()
            .map(|events| vec![events])
            .unwrap_or_default()
    }

    fn observe_json_text(&mut self, data: &str) -> Option<Vec<String>> {
        let data = data.trim();
        if data.is_empty() {
            return Some(Vec::new());
        }
        serde_json::from_str::<serde_json::Value>(data)
            .ok()
            .map(|value| self.observe_generate_value(&value))
    }

    fn flush_accumulated_json(&mut self) -> Option<String> {
        if self.accumulated_json.trim().is_empty() {
            self.accumulated_json.clear();
            return None;
        }
        let value = serde_json::from_str::<serde_json::Value>(&self.accumulated_json).ok()?;
        self.accumulated_json.clear();
        let events = self.observe_generate_value(&value);
        (!events.is_empty()).then(|| events.join(""))
    }

    fn observe_generate_value(&mut self, value: &serde_json::Value) -> Vec<String> {
        let value = runtime_gemini_normalized_response_value(value);
        self.state.observe_generate_chunk(&value)
    }

    fn read_next_sse_data_lines(&mut self) -> io::Result<Option<Vec<String>>> {
        let mut data_lines = Vec::new();
        loop {
            let mut line = String::new();
            let read = std::io::BufRead::read_line(&mut self.reader, &mut line)?;
            if read == 0 {
                return if data_lines.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(data_lines))
                };
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if data_lines.is_empty() {
                    continue;
                }
                return Ok(Some(data_lines));
            }
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start().to_string());
            }
        }
    }
}

impl<R: Read> Read for RuntimeGeminiGenerateSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.fill_pending()?;
        if self.pending_offset >= self.pending.len() {
            return Ok(0);
        }
        let len = buf.len().min(self.pending.len() - self.pending_offset);
        buf[..len].copy_from_slice(&self.pending[self.pending_offset..self.pending_offset + len]);
        self.pending_offset += len;
        Ok(len)
    }
}

#[cfg(test)]
#[path = "gemini_sse_tests.rs"]
mod tests;
