use super::*;

#[test]
fn mojo_tool_result_text_plan_matches_rust_oracle() {
    let cases = [
        "Web search results for query: \"berita \u{1f980}\"\n\nLinks: []\n\nSummary one\n\nSummary two\nSources:\nignored",
        "\u{2003}Web search results for query: plain query\r\nNo links found.\nLink: ignored\nanswer\u{2003}\nIf you'd like more",
        "Web search results for query: \" spaced \" trailing\nREMINDER: stop",
        "not a web result",
        "",
    ];
    for text in cases {
        let (_, summary_start) =
            runtime_proxy_anthropic_web_search_urls_from_tool_result_text(text);
        assert_eq!(
            runtime_proxy_anthropic_tool_result_text_plan(text, summary_start),
            runtime_proxy_anthropic_tool_result_text_plan_rust(text, summary_start),
            "{text:?}"
        );
    }
}

#[test]
fn mojo_compact_summary_matches_rust_oracle() {
    for summary in [
        "  first  \n\n second \n",
        "No links found.\ntext\nKalau mau, saya bisa lanjutkan",
        "Link: ignored\n\u{2003}unicode\u{2003}",
        "",
    ] {
        assert_eq!(
            runtime_proxy_compact_web_search_tool_result_summary(summary),
            runtime_proxy_compact_web_search_tool_result_summary_rust(summary),
            "{summary:?}"
        );
    }
}

#[test]
fn mojo_tool_result_text_plan_enforces_size_boundary() {
    const MAX_BYTES: usize = 4 * 1024 * 1024;
    let exact = "x".repeat(MAX_BYTES);
    let mut input = prodex_mojo_core::rich::RuntimeAnthropicKernelInput::new(
        prodex_mojo_core::rich::RuntimeAnthropicKernelOperation::ToolResultTextPlan,
    );
    input.text = Some(&exact);
    input.flags = 1;
    assert!(prodex_mojo_core::rich::runtime_anthropic_kernel(input).is_ok());

    let oversized = format!("{exact}x");
    input.text = Some(&oversized);
    assert!(prodex_mojo_core::rich::runtime_anthropic_kernel(input).is_err());
}

#[test]
fn mojo_server_tool_usage_matches_rust_oracle() {
    for block in [
        serde_json::json!({"type":"server_tool_use","name":"Web Search"}),
        serde_json::json!({"type":"mcp_tool_use","name":"bash-code_execution"}),
        serde_json::json!({"type":"tool_use","name":"tool.search/tool_bm25"}),
        serde_json::json!({"type":"text","name":"web_search"}),
    ] {
        assert_eq!(
            runtime_proxy_anthropic_tool_use_server_tool_usage(&block),
            runtime_proxy_anthropic_tool_use_server_tool_usage_rust(&block),
            "{block}"
        );
    }
}

#[test]
fn mojo_carried_server_tool_usage_matches_rust_oracle() {
    let cases = [
        vec![],
        vec![serde_json::json!({"role":"assistant","content":[
            {"type":"server_tool_use","name":"web_search"},
            {"type":"web_search_tool_result"}
        ]})],
        vec![
            serde_json::json!({"role":"assistant","content":[{"type":"server_tool_use","name":"web_fetch"}]}),
            serde_json::json!({"role":"user","content":"break"}),
            serde_json::json!({"role":"assistant","content":[{"type":"mcp_tool_use","name":"CODE.Execution"}]}),
            serde_json::json!({"role":"user","content":[{"type":"tool_result"}]}),
        ],
        vec![
            serde_json::json!({"role":"assistant","content":[{"type":"server_tool_use","name":"web_search"}]}),
            serde_json::json!({"role":"assistant","content":[{"type":"tool_use","name":"client_tool"}]}),
        ],
        vec![
            serde_json::json!({"role":"assistant","content":[{"type":"server_tool_use","name":"web_search"}]}),
            serde_json::json!({"role":"assistant","content":"trailing text"}),
        ],
    ];
    for messages in cases {
        assert_eq!(
            runtime_proxy_anthropic_carried_server_tool_usage(&messages),
            runtime_proxy_anthropic_carried_server_tool_usage_rust(&messages),
            "{messages:?}"
        );
    }
}

#[test]
fn mojo_server_tool_registry_and_chain_detection_match_rust_oracle() {
    let messages = vec![
        serde_json::json!({"role":"assistant","content":[
            {"type":"server_tool_use","name":" Web Search "},
            {"type":"mcp_tool_use","name":"custom-tool"},
            {"type":"tool_use","name":"ignored"}
        ]}),
        serde_json::json!({"role":"user","content":[{"type":"web_fetch_tool_result"}]}),
    ];
    for message in &messages {
        assert_eq!(
            runtime_proxy_anthropic_message_has_tool_chain_blocks(message),
            runtime_proxy_anthropic_message_has_tool_chain_blocks_rust(message)
        );
    }

    let mut mojo = RuntimeAnthropicServerTools::default();
    let mut rust = RuntimeAnthropicServerTools::default();
    runtime_proxy_anthropic_register_server_tools_from_messages(&messages, &mut mojo);
    runtime_proxy_anthropic_register_server_tools_from_messages_rust(&messages, &mut rust);
    assert_eq!(mojo.aliases.len(), rust.aliases.len());
    for (name, expected) in rust.aliases {
        let actual = &mojo.aliases[&name];
        assert_eq!(actual.response_name, expected.response_name);
        assert_eq!(actual.block_type, expected.block_type);
    }
}
