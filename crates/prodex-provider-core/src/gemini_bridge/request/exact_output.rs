//! Gemini exact-output response shims.

pub fn gemini_provider_core_exact_output_generate_chunk(
    request_id: u64,
    model: &str,
    output: &str,
) -> serde_json::Value {
    serde_json::json!({
        "responseId": format!("resp_gemini_exact_{request_id}"),
        "modelVersion": model,
        "candidates": [{
            "content": {
                "parts": [{"text": output}]
            },
            "finishReason": "STOP"
        }]
    })
}

pub fn gemini_provider_core_exact_output_sse_stream(
    request_id: u64,
    model: &str,
    output: &str,
) -> String {
    let generate_chunk =
        gemini_provider_core_exact_output_generate_chunk(request_id, model, output);
    format!("data: {generate_chunk}\n\ndata: [DONE]\n\n")
}
