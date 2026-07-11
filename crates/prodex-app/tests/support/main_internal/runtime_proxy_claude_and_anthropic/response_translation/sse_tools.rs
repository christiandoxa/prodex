use super::*;

#[test]
fn runtime_anthropic_response_from_sse_bytes_collects_deltas_and_tool_use() {
    let body = concat!(
        "event: response.reasoning_summary_text.delta\r\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Plan.\"}\r\n",
        "\r\n",
        "event: response.output_text.delta\r\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\r\n",
        "\r\n",
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4}}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", true)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(9)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_maps_shell_call_to_bash_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"shell_call\",\"call_id\":\"call_shell\"}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"shell_call\",\"call_id\":\"call_shell\",\"action\":{\"commands\":[\"pwd\",\"ls -la\"],\"timeout_ms\":1200}}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":7,\"output_tokens\":3},\"output\":[{\"type\":\"shell_call\",\"call_id\":\"call_shell\",\"action\":{\"commands\":[\"pwd\",\"ls -la\"],\"timeout_ms\":1200}}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("bash")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd\nls -la")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("timeout_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_maps_computer_call_to_computer_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"computer_call\",\"call_id\":\"call_computer\"}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"computer_call\",\"call_id\":\"call_computer\",\"actions\":[{\"type\":\"move\",\"x\":100,\"y\":200}]}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":6,\"output_tokens\":3},\"output\":[{\"type\":\"computer_call\",\"call_id\":\"call_computer\",\"actions\":[{\"type\":\"move\",\"x\":100,\"y\":200}]}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-opus-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("action"))
            .and_then(serde_json::Value::as_str),
        Some("mouse_move")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("coordinate"))
            .and_then(serde_json::Value::as_array)
            .map(|coordinates| {
                coordinates
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![100, 200])
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_preserves_web_search_usage() {
    let body = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_123\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"queries\":[\"OpenAI latest news today\"],\"sources\":[{\"type\":\"url\",\"url\":\"https://openai.com/index/industrial-policy-for-the-intelligence-age\",\"title\":\"Industrial policy for the Intelligence Age\"}]}}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"tool_usage\":{\"web_search\":{\"num_requests\":1}},\"output\":[{\"type\":\"web_search_call\",\"id\":\"ws_123\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"queries\":[\"OpenAI latest news today\"],\"sources\":[{\"type\":\"url\",\"url\":\"https://openai.com/index/industrial-policy-for-the-intelligence-age\",\"title\":\"Industrial policy for the Intelligence Age\"}]}},{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"OpenAI published a new industrial policy post.\"}]}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("web_search_tool_result")
    );
    assert_eq!(
        content[2].get("text").and_then(serde_json::Value::as_str),
        Some("OpenAI published a new industrial policy post.")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_counts_web_search_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"query\\\":\\\"OpenAI latest news today\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_counts_web_fetch_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_2\",\"name\":\"web_fetch\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_2\",\"delta\":\"{\\\"url\\\":\\\"https://example.com/article\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_2\",\"name\":\"web_fetch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}
