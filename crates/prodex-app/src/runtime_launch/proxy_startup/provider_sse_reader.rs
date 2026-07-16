use std::io::{self, BufRead, Read};
use std::sync::Arc;

use crate::RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES;

const RUNTIME_PROVIDER_SSE_LINE_MAX_BYTES: usize = RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES;
const RUNTIME_PROVIDER_SSE_EVENT_MAX_BYTES: usize = RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES;
const RUNTIME_PROVIDER_SSE_ACCUMULATED_JSON_MAX_BYTES: usize =
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES;

pub(super) type RuntimeProviderSseObserver = Arc<dyn Fn(&serde_json::Value) + Send + Sync>;

pub(super) trait RuntimeProviderSseJsonState {
    fn eof(&self) -> bool;
    fn set_eof(&mut self, eof: bool);
    fn observe_value(&mut self, value: &serde_json::Value) -> Vec<String>;
    fn complete_event(&mut self) -> Option<String>;
    fn failed_event(&mut self, code: &str, message: &str) -> Option<String>;
}

pub(super) struct RuntimeProviderSseJsonReader<R: Read, S> {
    reader: std::io::BufReader<R>,
    pending: Vec<u8>,
    pending_offset: usize,
    accumulated_json: String,
    line_max_bytes: usize,
    event_max_bytes: usize,
    accumulated_json_max_bytes: usize,
    state: S,
    observer: Option<RuntimeProviderSseObserver>,
}

impl<R: Read, S: RuntimeProviderSseJsonState> RuntimeProviderSseJsonReader<R, S> {
    pub(super) fn new_with_observer(
        reader: R,
        state: S,
        observer: Option<RuntimeProviderSseObserver>,
    ) -> Self {
        Self::new_with_limits(
            reader,
            state,
            observer,
            RUNTIME_PROVIDER_SSE_LINE_MAX_BYTES,
            RUNTIME_PROVIDER_SSE_EVENT_MAX_BYTES,
            RUNTIME_PROVIDER_SSE_ACCUMULATED_JSON_MAX_BYTES,
        )
    }

    fn new_with_limits(
        reader: R,
        state: S,
        observer: Option<RuntimeProviderSseObserver>,
        line_max_bytes: usize,
        event_max_bytes: usize,
        accumulated_json_max_bytes: usize,
    ) -> Self {
        Self {
            reader: std::io::BufReader::new(reader),
            pending: Vec::new(),
            pending_offset: 0,
            accumulated_json: String::new(),
            line_max_bytes,
            event_max_bytes,
            accumulated_json_max_bytes,
            state,
            observer,
        }
    }

    fn fail_stream(&mut self, err: io::Error) -> io::Result<()> {
        if let Some(event) = self
            .state
            .failed_event("provider_stream_error", &err.to_string())
        {
            self.pending = event.into_bytes();
            self.state.set_eof(true);
            return Ok(());
        }
        Err(err)
    }

    fn fill_pending(&mut self) -> io::Result<()> {
        while self.pending_offset >= self.pending.len() && !self.state.eof() {
            self.pending.clear();
            self.pending_offset = 0;
            let data_lines = match read_runtime_provider_sse_data_lines_with_limits(
                &mut self.reader,
                self.line_max_bytes,
                self.event_max_bytes,
            ) {
                Ok(data_lines) => data_lines,
                Err(err) => {
                    self.fail_stream(err)?;
                    break;
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
                    self.state.set_eof(true);
                    break;
                }
                if let Some(event) = self.state.complete_event() {
                    self.pending = event.into_bytes();
                }
                self.state.set_eof(true);
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
                        self.state.set_eof(true);
                        break;
                    }
                }
                if let Some(event) = self.state.complete_event() {
                    self.pending.extend_from_slice(event.as_bytes());
                }
                self.state.set_eof(true);
                break;
            }
            let events = match self.observe_sse_data_lines(&data_lines) {
                Ok(events) => events,
                Err(err) => {
                    self.fail_stream(err)?;
                    break;
                }
            };
            for event in events {
                self.pending.extend_from_slice(event.as_bytes());
            }
        }
        Ok(())
    }

    fn observe_sse_data_lines(&mut self, data_lines: &[String]) -> io::Result<Vec<String>> {
        let joined = data_lines.join("\n");
        if let Some(events) = self.observe_json_text(&joined) {
            return Ok(events);
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
                if let Some(observer) = &self.observer {
                    observer(&value);
                }
                events.extend(self.state.observe_value(&value));
            }
            return Ok(events);
        }

        let joined = joined.trim();
        if self.accumulated_json.len().saturating_add(joined.len())
            > self.accumulated_json_max_bytes
        {
            return Err(runtime_provider_sse_limit_error(
                "accumulated JSON",
                self.accumulated_json_max_bytes,
            ));
        }
        self.accumulated_json.push_str(joined);
        Ok(self
            .flush_accumulated_json()
            .map(|events| vec![events])
            .unwrap_or_default())
    }

    fn observe_json_text(&mut self, data: &str) -> Option<Vec<String>> {
        let data = data.trim();
        if data.is_empty() {
            return Some(Vec::new());
        }
        serde_json::from_str::<serde_json::Value>(data)
            .ok()
            .map(|value| {
                if let Some(observer) = &self.observer {
                    observer(&value);
                }
                self.state.observe_value(&value)
            })
    }

    fn flush_accumulated_json(&mut self) -> Option<String> {
        if self.accumulated_json.trim().is_empty() {
            self.accumulated_json.clear();
            return None;
        }
        let value = serde_json::from_str::<serde_json::Value>(&self.accumulated_json).ok()?;
        self.accumulated_json.clear();
        if let Some(observer) = &self.observer {
            observer(&value);
        }
        let events = self.state.observe_value(&value);
        (!events.is_empty()).then(|| events.join(""))
    }
}

impl<R: Read, S: RuntimeProviderSseJsonState> Read for RuntimeProviderSseJsonReader<R, S> {
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

fn read_runtime_provider_sse_data_lines_with_limits<R: BufRead>(
    reader: &mut R,
    line_max_bytes: usize,
    event_max_bytes: usize,
) -> io::Result<Option<Vec<String>>> {
    let mut data_lines = Vec::new();
    let mut event_bytes = 0usize;
    loop {
        let mut line = String::new();
        let read = (&mut *reader)
            .take((line_max_bytes as u64).saturating_add(1))
            .read_line(&mut line)?;
        if read > line_max_bytes {
            return Err(runtime_provider_sse_limit_error("line", line_max_bytes));
        }
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
            let data = data.trim_start();
            event_bytes = event_bytes.saturating_add(data.len()).saturating_add(1);
            if event_bytes > event_max_bytes {
                return Err(runtime_provider_sse_limit_error("event", event_max_bytes));
            }
            data_lines.push(data.to_string());
        }
    }
}

fn runtime_provider_sse_limit_error(kind: &str, max_bytes: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("provider SSE {kind} exceeded safe size limit ({max_bytes})"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[derive(Default)]
    struct TestState {
        eof: bool,
    }

    impl RuntimeProviderSseJsonState for TestState {
        fn eof(&self) -> bool {
            self.eof
        }

        fn set_eof(&mut self, eof: bool) {
            self.eof = eof;
        }

        fn observe_value(&mut self, _value: &serde_json::Value) -> Vec<String> {
            Vec::new()
        }

        fn complete_event(&mut self) -> Option<String> {
            None
        }

        fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
            Some(format!("{code}:{message}"))
        }
    }

    #[test]
    fn provider_sse_rejects_oversized_line() {
        let mut reader = Cursor::new(format!("data: {}\n\n", "x".repeat(32)));
        let err = read_runtime_provider_sse_data_lines_with_limits(&mut reader, 16, 64)
            .expect_err("oversized SSE line should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("provider SSE line"));
    }

    #[test]
    fn provider_sse_rejects_oversized_event() {
        let mut reader = Cursor::new("data: 12345678\ndata: 12345678\n\n");
        let err = read_runtime_provider_sse_data_lines_with_limits(&mut reader, 32, 12)
            .expect_err("oversized SSE event should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("provider SSE event"));
    }

    #[test]
    fn provider_sse_emits_failed_event_for_oversized_accumulated_json() {
        let input = Cursor::new("data: {\"a\":\n\ndata: \"12345\"}\n\n");
        let mut reader = RuntimeProviderSseJsonReader::new_with_limits(
            input,
            TestState::default(),
            None,
            64,
            64,
            10,
        );
        let mut output = String::new();

        reader
            .read_to_string(&mut output)
            .expect("state should translate the size error into a failed event");

        assert!(output.contains("provider_stream_error"));
        assert!(output.contains("provider SSE accumulated JSON"));
    }
}
