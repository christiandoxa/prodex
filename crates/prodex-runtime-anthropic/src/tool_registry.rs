use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeAnthropicServerToolUsage {
    pub web_search_requests: u64,
    pub web_fetch_requests: u64,
    pub code_execution_requests: u64,
    pub tool_search_requests: u64,
}

impl RuntimeAnthropicServerToolUsage {
    pub fn add_assign(&mut self, other: Self) {
        self.web_search_requests += other.web_search_requests;
        self.web_fetch_requests += other.web_fetch_requests;
        self.code_execution_requests += other.code_execution_requests;
        self.tool_search_requests += other.tool_search_requests;
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeAnthropicServerTools {
    pub aliases: BTreeMap<String, RuntimeAnthropicRegisteredServerTool>,
    pub web_search: bool,
    pub mcp: bool,
    pub tool_search: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeAnthropicRegisteredServerTool {
    pub response_name: String,
    pub block_type: String,
}

impl RuntimeAnthropicServerTools {
    pub fn needs_buffered_translation(&self) -> bool {
        self.web_search
    }

    pub fn register(&mut self, tool_name: &str, canonical_name: &str) {
        self.register_with_block_type(tool_name, canonical_name, "server_tool_use");
    }

    pub fn register_with_block_type(
        &mut self,
        tool_name: &str,
        response_name: &str,
        block_type: &str,
    ) {
        let tool_name = tool_name.trim();
        let response_name = response_name.trim();
        let block_type = block_type.trim();
        if tool_name.is_empty() || response_name.is_empty() || block_type.is_empty() {
            return;
        }
        let registration = RuntimeAnthropicRegisteredServerTool {
            response_name: response_name.to_string(),
            block_type: block_type.to_string(),
        };
        self.aliases
            .insert(tool_name.to_string(), registration.clone());
        if let Some(normalized) = runtime_proxy_anthropic_builtin_server_tool_name(tool_name) {
            self.aliases.insert(normalized.to_string(), registration);
        }
        if response_name == "web_search" {
            self.web_search = true;
        } else if block_type == "mcp_tool_use" {
            self.mcp = true;
        } else if response_name.starts_with("tool_search_tool_") {
            self.tool_search = true;
        }
    }

    pub fn registration_for_call(
        &self,
        tool_name: &str,
    ) -> Option<&RuntimeAnthropicRegisteredServerTool> {
        let tool_name = tool_name.trim();
        if tool_name.is_empty() {
            return None;
        }
        self.aliases.get(tool_name).or_else(|| {
            runtime_proxy_anthropic_builtin_server_tool_name(tool_name)
                .and_then(|normalized| self.aliases.get(normalized))
        })
    }

    pub fn canonical_name_for_call(&self, tool_name: &str) -> Option<&str> {
        self.registration_for_call(tool_name)
            .map(|registration| registration.response_name.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeAnthropicMcpServer {
    pub name: String,
    pub url: Option<String>,
    pub authorization_token: Option<String>,
    pub headers: serde_json::Map<String, serde_json::Value>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeAnthropicTranslatedTools {
    pub tools: Vec<serde_json::Value>,
    pub server_tools: RuntimeAnthropicServerTools,
    pub tool_name_aliases: BTreeMap<String, String>,
    pub native_tool_names: BTreeSet<String>,
    pub memory: bool,
}

impl RuntimeAnthropicTranslatedTools {
    pub fn implicit_tool_choice(&self) -> Option<serde_json::Value> {
        (self.server_tools.web_search
            && self.tools.len() == 1
            && self.tools.first().and_then(|tool| {
                tool.get("type")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
            }) == Some("web_search"))
        .then(|| serde_json::Value::String("required".to_string()))
    }
}
