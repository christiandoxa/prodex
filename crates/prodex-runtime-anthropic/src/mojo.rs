use prodex_mojo_core::rich::{
    RUNTIME_ANTHROPIC_FLAG_CACHED_TOKENS, RuntimeAnthropicKernelInput,
    RuntimeAnthropicKernelOperation, runtime_anthropic_kernel,
};

pub(crate) fn bytes(input: RuntimeAnthropicKernelInput<'_>) -> Vec<u8> {
    runtime_anthropic_kernel(input)
        .unwrap_or_else(|error| panic!("runtime Anthropic Mojo kernel failed: {error:?}"))
}

pub(crate) fn json(input: RuntimeAnthropicKernelInput<'_>) -> serde_json::Value {
    let output = bytes(input);
    serde_json::from_slice(&output)
        .unwrap_or_else(|error| panic!("runtime Anthropic Mojo output was invalid JSON: {error}"))
}

pub(crate) fn sse_event(event_type: &str, data: &serde_json::Value) -> Vec<u8> {
    let data = serde_json::to_string(data).expect("Anthropic SSE event data serializes");
    let mut input = RuntimeAnthropicKernelInput::new(RuntimeAnthropicKernelOperation::SseEvent);
    input.text = Some(event_type);
    input.message = Some(&data);
    bytes(input)
}

pub(crate) fn message_sse(value: &serde_json::Value) -> Vec<u8> {
    let message = serde_json::to_string(value).expect("Anthropic message serializes");
    let mut input = RuntimeAnthropicKernelInput::new(RuntimeAnthropicKernelOperation::MessageSse);
    input.message = Some(&message);
    bytes(input)
}

pub(crate) fn response_message(
    id: &str,
    model: &str,
    content: &str,
    usage: &str,
    stop_reason: &str,
) -> serde_json::Value {
    let mut input =
        RuntimeAnthropicKernelInput::new(RuntimeAnthropicKernelOperation::ResponseMessage);
    input.id = Some(id);
    input.name = Some(model);
    input.content = Some(content);
    input.usage = Some(usage);
    input.stop_reason = Some(stop_reason);
    json(input)
}

pub(crate) fn usage(
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
) -> serde_json::Map<String, serde_json::Value> {
    let mut input = RuntimeAnthropicKernelInput::new(RuntimeAnthropicKernelOperation::Usage);
    input.input_tokens = input_tokens;
    input.output_tokens = output_tokens;
    input.cached_tokens = cached_tokens.unwrap_or_default();
    input.web_search_requests = web_search_requests;
    input.web_fetch_requests = web_fetch_requests;
    input.code_execution_requests = code_execution_requests;
    input.tool_search_requests = tool_search_requests;
    if cached_tokens.is_some() {
        input.flags |= RUNTIME_ANTHROPIC_FLAG_CACHED_TOKENS;
    }
    json(input)
        .as_object()
        .cloned()
        .expect("Anthropic Mojo usage output should be an object")
}
