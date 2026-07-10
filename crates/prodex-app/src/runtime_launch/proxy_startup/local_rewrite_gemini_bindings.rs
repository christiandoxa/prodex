use super::gemini_sse::RuntimeGeminiBindingRecorder;
use prodex_provider_core::gemini_provider_core_response_bindings_from_body;

pub(super) fn runtime_gemini_remember_bindings_from_responses_body(
    recorder: Option<&RuntimeGeminiBindingRecorder>,
    body: &[u8],
) {
    let Some(recorder) = recorder else {
        return;
    };
    if let Some((response_id, tool_call_ids)) =
        gemini_provider_core_response_bindings_from_body(body)
    {
        recorder(response_id, tool_call_ids);
    }
}
