use std::collections::{BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Result as IoResult};
use std::sync::Arc;

pub(super) type RuntimeCopilotBindingRecorder = Arc<dyn Fn(String) + Send + Sync>;

pub(super) fn runtime_copilot_remember_bindings_from_responses_body(
    recorder: Option<&RuntimeCopilotBindingRecorder>,
    body: &[u8],
) {
    let Some(recorder) = recorder else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return;
    };
    if let Some(response_id) = runtime_copilot_response_id_from_value(&value) {
        recorder(response_id);
    }
}

pub(super) struct RuntimeCopilotResponsesSseBindingReader<R> {
    reader: BufReader<R>,
    pending: VecDeque<u8>,
    event_name: String,
    data: String,
    binding_recorder: Option<RuntimeCopilotBindingRecorder>,
    seen_response_ids: BTreeSet<String>,
    eof_dispatched: bool,
}

impl<R: Read> RuntimeCopilotResponsesSseBindingReader<R> {
    pub(super) fn new(reader: R, binding_recorder: Option<RuntimeCopilotBindingRecorder>) -> Self {
        Self {
            reader: BufReader::new(reader),
            pending: VecDeque::new(),
            event_name: String::new(),
            data: String::new(),
            binding_recorder,
            seen_response_ids: BTreeSet::new(),
            eof_dispatched: false,
        }
    }

    fn fill_pending(&mut self) -> IoResult<bool> {
        let mut line = Vec::new();
        let read = self.reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            if !self.eof_dispatched {
                self.dispatch_event();
                self.eof_dispatched = true;
            }
            return Ok(false);
        }
        self.observe_sse_line(&line);
        self.pending.extend(line);
        Ok(true)
    }

    fn observe_sse_line(&mut self, line: &[u8]) {
        let Ok(text) = std::str::from_utf8(line) else {
            return;
        };
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            self.dispatch_event();
            return;
        }
        if let Some(event_name) = text.strip_prefix("event:") {
            self.event_name = event_name.trim().to_string();
            return;
        }
        if let Some(data) = text.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(data.trim_start());
        }
    }

    fn dispatch_event(&mut self) {
        let data = std::mem::take(&mut self.data);
        self.event_name.clear();
        if data.trim().is_empty() || data.trim() == "[DONE]" {
            return;
        }
        let Some(recorder) = self.binding_recorder.as_ref() else {
            return;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        if let Some(response_id) = runtime_copilot_response_id_from_value(&value)
            && self.seen_response_ids.insert(response_id.clone())
        {
            recorder(response_id);
        }
    }
}

impl<R: Read> Read for RuntimeCopilotResponsesSseBindingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.pending.is_empty() {
            if !self.fill_pending()? {
                return Ok(0);
            }
        }
        let mut written = 0;
        while written < buf.len() {
            let Some(byte) = self.pending.pop_front() else {
                break;
            };
            buf[written] = byte;
            written += 1;
        }
        Ok(written)
    }
}

fn runtime_copilot_response_id_from_value(value: &serde_json::Value) -> Option<String> {
    prodex_provider_core::copilot_provider_core_response_id_from_value(value)
}
