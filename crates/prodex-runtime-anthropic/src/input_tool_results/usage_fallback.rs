use super::*;

pub(super) fn runtime_proxy_anthropic_tool_use_server_tool_usage_rust(
    block: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    let tool_name = match block.get("type").and_then(serde_json::Value::as_str) {
        Some(block_type) if runtime_proxy_anthropic_is_tool_use_block_type(block_type) => block
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim),
        _ => None,
    };
    match tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name) {
        Some("web_search") => RuntimeAnthropicServerToolUsage {
            web_search_requests: 1,
            ..RuntimeAnthropicServerToolUsage::default()
        },
        Some("web_fetch") => RuntimeAnthropicServerToolUsage {
            web_fetch_requests: 1,
            ..RuntimeAnthropicServerToolUsage::default()
        },
        Some("code_execution" | "bash_code_execution" | "text_editor_code_execution") => {
            RuntimeAnthropicServerToolUsage {
                code_execution_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            }
        }
        Some("tool_search_tool_regex" | "tool_search_tool_bm25") => {
            RuntimeAnthropicServerToolUsage {
                tool_search_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            }
        }
        _ => RuntimeAnthropicServerToolUsage::default(),
    }
}

pub(super) fn runtime_proxy_anthropic_carried_server_tool_usage_rust(
    messages: &[serde_json::Value],
) -> RuntimeAnthropicServerToolUsage {
    let mut usage = RuntimeAnthropicServerToolUsage::default();
    let mut collecting_suffix = false;
    for message in messages.iter().rev() {
        let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
            if collecting_suffix {
                break;
            }
            continue;
        };
        let mut saw_tool_chain_block = false;
        for block in blocks {
            let block_usage = runtime_proxy_anthropic_tool_use_server_tool_usage_rust(block);
            if block_usage != RuntimeAnthropicServerToolUsage::default() {
                usage.add_assign(block_usage);
                saw_tool_chain_block = true;
                continue;
            }
            if block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(runtime_proxy_anthropic_is_tool_result_block_type)
            {
                saw_tool_chain_block = true;
            }
        }
        if saw_tool_chain_block {
            collecting_suffix = true;
        } else if collecting_suffix
            || runtime_proxy_anthropic_message_has_tool_chain_blocks_rust(message)
        {
            break;
        }
    }
    usage
}

pub(super) fn runtime_proxy_anthropic_register_server_tools_from_messages_rust(
    messages: &[serde_json::Value],
    server_tools: &mut RuntimeAnthropicServerTools,
) {
    for message in messages {
        let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for block in blocks {
            let block_type = block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !matches!(block_type, "server_tool_use" | "mcp_tool_use") {
                continue;
            }
            let Some(tool_name) = block
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let response_name = runtime_proxy_anthropic_builtin_server_tool_name_rust(tool_name)
                .unwrap_or(tool_name);
            server_tools.register_with_block_type(tool_name, response_name, block_type);
        }
    }
}

pub(super) fn runtime_proxy_anthropic_message_has_tool_chain_blocks_rust(
    message: &serde_json::Value,
) -> bool {
    let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
        return false;
    };
    blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|block_type| {
                runtime_proxy_anthropic_is_tool_use_block_type(block_type)
                    || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
            })
    })
}
