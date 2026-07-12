use super::*;
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

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

#[derive(Default)]
pub struct RuntimeAnthropicMcpServer {
    pub name: String,
    pub url: Option<String>,
    pub authorization_token: Option<String>,
    pub headers: serde_json::Map<String, serde_json::Value>,
    pub description: Option<String>,
}

impl fmt::Debug for RuntimeAnthropicMcpServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAnthropicMcpServer")
            .field("name", &self.name)
            .field("url_configured", &self.url.is_some())
            .field(
                "authorization_token",
                &self.authorization_token.as_ref().map(|_| "<redacted>"),
            )
            .field("header_count", &self.headers.len())
            .field("headers", &"<redacted>")
            .field("description_configured", &self.description.is_some())
            .finish()
    }
}

impl Zeroize for RuntimeAnthropicMcpServer {
    fn zeroize(&mut self) {
        self.url.zeroize();
        self.authorization_token.zeroize();
        for value in self.headers.values_mut() {
            zeroize_json_secret_values(value);
        }
        self.headers.clear();
    }
}

impl Drop for RuntimeAnthropicMcpServer {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for RuntimeAnthropicMcpServer {}

#[derive(Default)]
pub struct RuntimeAnthropicTranslatedTools {
    pub tools: Vec<serde_json::Value>,
    pub server_tools: RuntimeAnthropicServerTools,
    pub tool_name_aliases: BTreeMap<String, String>,
    pub native_tool_names: BTreeSet<String>,
    pub memory: bool,
}

impl fmt::Debug for RuntimeAnthropicTranslatedTools {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAnthropicTranslatedTools")
            .field("tool_count", &self.tools.len())
            .field("tools", &"<redacted>")
            .field("server_tools", &self.server_tools)
            .field("tool_name_aliases", &self.tool_name_aliases)
            .field("native_tool_names", &self.native_tool_names)
            .field("memory", &self.memory)
            .finish()
    }
}

impl Zeroize for RuntimeAnthropicTranslatedTools {
    fn zeroize(&mut self) {
        for tool in &mut self.tools {
            zeroize_json_secret_values(tool);
        }
        self.tools.clear();
    }
}

impl Drop for RuntimeAnthropicTranslatedTools {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for RuntimeAnthropicTranslatedTools {}

fn zeroize_json_secret_values(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(value) => value.zeroize(),
        serde_json::Value::Array(values) => {
            for value in values {
                zeroize_json_secret_values(value);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                zeroize_json_secret_values(value);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
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
