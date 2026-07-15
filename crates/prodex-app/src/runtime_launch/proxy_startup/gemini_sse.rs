use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::provider_sse_reader::{
    RuntimeProviderSseJsonReader, RuntimeProviderSseJsonState, RuntimeProviderSseObserver,
};
use prodex_provider_core::gemini_provider_core_normalized_response_value;
use std::io::{self, Read};
use std::sync::Arc;

#[path = "gemini_sse_state.rs"]
mod gemini_sse_state;

use gemini_sse_state::RuntimeGeminiSseState;
pub(super) use prodex_provider_core::gemini_provider_core_forced_command_output as runtime_gemini_forced_command_output;

pub(super) type RuntimeGeminiBindingRecorder = Arc<dyn Fn(String, Vec<String>) + Send + Sync>;

pub(super) struct RuntimeGeminiGenerateSseReader<R: Read> {
    inner: RuntimeProviderSseJsonReader<R, RuntimeGeminiSseState>,
}

impl<R: Read> RuntimeGeminiGenerateSseReader<R> {
    #[cfg(test)]
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
    ) -> Self {
        let config = crate::RuntimeConfig::compatibility_current();
        Self::new_with_config(
            reader,
            request_id,
            conversation_messages,
            conversations,
            binding_recorder,
            None,
            prodex_provider_core::EffectiveHarnessMode::Native,
            None,
            config.gemini,
        )
    }

    pub(super) fn new_with_config(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
        observer: Option<RuntimeProviderSseObserver>,
        harness_mode: prodex_provider_core::EffectiveHarnessMode,
        harness_model: Option<String>,
        gemini_config: crate::RuntimeGeminiConfig,
    ) -> Self {
        Self {
            inner: RuntimeProviderSseJsonReader::new_with_observer(
                reader,
                RuntimeGeminiSseState::new(
                    request_id,
                    conversation_messages,
                    conversations,
                    binding_recorder,
                    harness_mode,
                    harness_model,
                    gemini_config,
                ),
                observer,
            ),
        }
    }
}

impl RuntimeProviderSseJsonState for RuntimeGeminiSseState {
    fn eof(&self) -> bool {
        self.eof
    }

    fn set_eof(&mut self, eof: bool) {
        self.eof = eof;
    }

    fn observe_value(&mut self, value: &serde_json::Value) -> Vec<String> {
        let value = gemini_provider_core_normalized_response_value(value);
        let events = self.observe_generate_chunk(&value);
        self.postprocess_harness_events(events)
    }

    fn complete_event(&mut self) -> Option<String> {
        RuntimeGeminiSseState::complete_event(self)
            .map(|event| self.postprocess_harness_event(event))
    }

    fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
        RuntimeGeminiSseState::failed_event(self, code, message)
            .map(|event| self.postprocess_harness_event(event))
    }
}

impl<R: Read> Read for RuntimeGeminiGenerateSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

#[cfg(test)]
#[path = "gemini_sse_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gemini_sse_exact_output_tests.rs"]
mod exact_output_tests;

#[cfg(test)]
#[path = "gemini_sse_guardrail_tests.rs"]
mod guardrail_tests;

#[cfg(test)]
#[path = "gemini_sse_media_tests.rs"]
mod media_tests;

#[cfg(test)]
#[path = "gemini_sse_status_tests.rs"]
mod status_tests;
