use super::*;

#[test]
fn deepseek_sse_reader_preserves_final_usage_cache_details() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_usage\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"done\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_usage\",\"choices\":[],\"usage\":{\"prompt_tokens\":20,\"completion_tokens\":5,\"total_tokens\":25,\"completion_tokens_details\":{\"reasoning_tokens\":3},\"prompt_cache_hit_tokens\":12,\"prompt_cache_miss_tokens\":8}}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"input_tokens_details\":{\"cached_tokens\":12}"));
    assert!(output.contains("\"output_tokens_details\":{\"reasoning_tokens\":3}"));
    assert!(output.contains("\"prompt_cache_miss_tokens\":8"));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn deepseek_sse_reader_preserves_logprobs_metadata() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_logprobs\",\"model\":\"deepseek-v4-pro\",\"created\":1782740700,\"system_fingerprint\":\"fp_deepseek_stream\",\"choices\":[{\"delta\":{\"content\":\"done\",\"refusal\":\"I cannot\",\"annotations\":[{\"type\":\"url_citation\",\"url\":\"https://example.com/ref\"}]},\"logprobs\":{\"content\":[{\"token\":\"done\",\"logprob\":-0.2,\"bytes\":[100,111,110,101],\"top_logprobs\":[]}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_logprobs\",\"choices\":[{\"delta\":{\"refusal\":\" help with that.\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_logprobs\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"deepseek\""));
    assert!(output.contains("\"logprobs\":{\"content\""));
    assert!(output.contains("\"token\":\"done\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
    assert!(output.contains("\"refusal\":\"I cannot help with that.\""));
    assert!(output.contains("\"annotations\":[{\"type\":\"url_citation\""));
    assert!(output.contains("\"url\":\"https://example.com/ref\""));
    assert!(output.contains("\"created_at\":1782740700"));
    assert!(output.contains("\"system_fingerprint\":\"fp_deepseek_stream\""));
}

#[test]
fn deepseek_sse_reader_merges_request_degradation_metadata() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_json\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"{\\\"answer\\\":\\\"ok\\\"}\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        57,
        Vec::new(),
        Some(serde_json::json!({
            "deepseek": {
                "degraded_response_format": {
                    "from": "json_schema",
                    "to": "json_object"
                }
            }
        })),
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"degraded_response_format\""));
    assert!(output.contains("\"from\":\"json_schema\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
}

#[test]
fn deepseek_sse_reader_maps_embedded_error_to_failed_event() {
    let stream = "data: {\"error\":{\"type\":\"rate_limit_error\",\"message\":\"busy\"}}\n\n";
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("event: response.failed"));
    assert!(output.contains("rate_limit_error"));
    assert!(!output.contains("event: response.completed"));
}

struct ErrorAfterDataReader {
    data: std::io::Cursor<Vec<u8>>,
    errored: bool,
}

impl Read for ErrorAfterDataReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.data.read(buf)?;
        if read > 0 {
            return Ok(read);
        }
        if !self.errored {
            self.errored = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "error decoding response body",
            ));
        }
        Ok(0)
    }
}

#[test]
fn deepseek_sse_reader_maps_upstream_decode_error_to_failed_event() {
    let mut reader = RuntimeDeepSeekChatSseReader::new(
        ErrorAfterDataReader {
            data: std::io::Cursor::new(
                b"data: {\"id\":\"chatcmpl_5\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n"
                    .to_vec(),
            ),
            errored: false,
        },
        7,
        Vec::new(),
        None,
        conversation_store(),
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"delta\":\"hi\""));
    assert!(output.contains("event: response.failed"));
    assert!(output.contains("provider_stream_error"));
    assert!(!output.contains("event: response.completed"));
}
