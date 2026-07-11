use super::*;

impl RuntimeAnthropicSseReader {
    pub(super) fn push_event(&mut self, event_type: &str, data: serde_json::Value) {
        let frame = format!(
            "event: {event_type}\ndata: {}\n\n",
            serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
        );
        self.pending.extend(frame.into_bytes());
    }

    pub(super) fn close_thinking_block(&mut self) {
        if !self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.thinking_block_open = false;
    }

    pub(super) fn close_text_block(&mut self) {
        if !self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.text_block_open = false;
    }

    pub(super) fn ensure_text_block(&mut self) {
        if self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "text",
                    "text": "",
                }
            }),
        );
        self.text_block_open = true;
    }

    pub(super) fn ensure_thinking_block(&mut self) {
        if self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "thinking",
                    "thinking": "",
                }
            }),
        );
        self.thinking_block_open = true;
    }

    pub(super) fn emit_completed_content_block(
        &mut self,
        block: serde_json::Value,
        has_tool_calls: bool,
    ) {
        let Some(block_type) = block.get("type").and_then(serde_json::Value::as_str) else {
            return;
        };

        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();

        match block_type {
            "mcp_tool_use" => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let mut content_block = serde_json::Map::new();
                content_block.insert(
                    "type".to_string(),
                    serde_json::Value::String("mcp_tool_use".to_string()),
                );
                content_block.insert(
                    "id".to_string(),
                    block
                        .get("id")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp_tool_use".to_string())),
                );
                content_block.insert(
                    "name".to_string(),
                    block
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp_tool".to_string())),
                );
                content_block.insert(
                    "server_name".to_string(),
                    block
                        .get("server_name")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp".to_string())),
                );
                content_block.insert("input".to_string(), serde_json::json!({}));
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": serde_json::Value::Object(content_block),
                    }),
                );
                self.push_event(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": self.content_index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                );
            }
            block_type if block_type.ends_with("_tool_result") => {
                let mut content_block = serde_json::Map::new();
                content_block.insert(
                    "type".to_string(),
                    serde_json::Value::String(block_type.to_string()),
                );
                content_block.insert(
                    "tool_use_id".to_string(),
                    block
                        .get("tool_use_id")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String(format!("{block_type}_call"))),
                );
                content_block.insert(
                    "content".to_string(),
                    block
                        .get("content")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
                if let Some(is_error) = block.get("is_error").cloned() {
                    content_block.insert("is_error".to_string(), is_error);
                }
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": serde_json::Value::Object(content_block),
                    }),
                );
            }
            "mcp_approval_request" | "mcp_list_tools" => {
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": block,
                    }),
                );
            }
            _ => return,
        }

        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.has_content = true;
        self.has_tool_calls |= has_tool_calls;
    }

    pub(super) fn emit_mcp_call_blocks(&mut self, item: &serde_json::Value) {
        for block in runtime_anthropic_mcp_call_blocks_from_output_item(item) {
            self.emit_completed_content_block(block, false);
        }
    }
}
