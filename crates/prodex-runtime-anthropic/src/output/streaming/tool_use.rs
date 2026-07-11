use super::*;

impl RuntimeAnthropicSseReader {
    pub(super) fn start_tool_use_block(&mut self, call_id: &str, name: &str) {
        let server_tool_registration =
            runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools));
        let block_type = server_tool_registration
            .as_ref()
            .map(|(_, block_type)| block_type.as_str())
            .unwrap_or("tool_use");
        let output_name = server_tool_registration
            .as_ref()
            .map(|(server_tool_name, _)| server_tool_name.as_str())
            .unwrap_or(name);
        let mut content_block = serde_json::Map::new();
        content_block.insert(
            "type".to_string(),
            serde_json::Value::String(block_type.to_string()),
        );
        content_block.insert(
            "id".to_string(),
            serde_json::Value::String(call_id.to_string()),
        );
        content_block.insert(
            "name".to_string(),
            serde_json::Value::String(output_name.to_string()),
        );
        content_block.insert("input".to_string(), serde_json::json!({}));
        self.close_thinking_block();
        self.close_text_block();
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": serde_json::Value::Object(content_block)
            }),
        );
        self.active_tool_use = Some(RuntimeAnthropicStreamToolUse {
            call_id: call_id.to_string(),
            name: name.to_string(),
            server_tool_name: server_tool_registration
                .as_ref()
                .map(|(server_tool_name, _)| server_tool_name.clone()),
            server_tool_block_type: server_tool_registration.map(|(_, block_type)| block_type),
            ..RuntimeAnthropicStreamToolUse::default()
        });
        self.has_content = true;
        self.has_tool_calls = true;
    }

    pub(super) fn finish_active_tool_use(
        &mut self,
        arguments_override: Option<&str>,
        name_override: Option<&str>,
        call_id_override: Option<&str>,
    ) {
        let Some(mut active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        if let Some(name) = name_override {
            active_tool_use.name = name.to_string();
            if let Some((server_tool_name, block_type)) =
                runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools))
            {
                active_tool_use.server_tool_name = Some(server_tool_name);
                active_tool_use.server_tool_block_type = Some(block_type);
            } else {
                active_tool_use.server_tool_name = None;
                active_tool_use.server_tool_block_type = None;
            }
        }
        if let Some(call_id) = call_id_override {
            active_tool_use.call_id = call_id.to_string();
        }
        if let Some(arguments) = arguments_override
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
        if !active_tool_use.saw_delta && !active_tool_use.arguments.is_empty() {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": active_tool_use.arguments,
                    }
                }),
            );
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
    }
}
