use anyhow::{Context, Result, bail};
use base64::Engine;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::io::{self, Read};

pub const DEFAULT_PRODEX_CLAUDE_MODEL: &str = "gpt-5.4";
pub const RUNTIME_PROXY_OPENAI_UPSTREAM_PATH: &str = "/backend-api/codex";
pub const RUNTIME_PROXY_ANTHROPIC_MODEL_CREATED_AT: &str = "2026-01-01T00:00:00Z";
pub const RUNTIME_PROXY_ANTHROPIC_MODELS_PATH: &str = "/v1/models";
pub const RUNTIME_PROXY_ANTHROPIC_HEALTH_PATH: &str = "/health";
pub const PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER: &str = "X-Prodex-Internal-Request-Origin";
pub const PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES: &str = "anthropic_messages";

mod input;
mod models;
mod output;
mod tools;

pub use input::*;
pub use models::*;
pub use output::*;
pub use tools::*;

#[derive(Debug, Clone)]
pub struct RuntimeProxyRequest {
    pub method: String,
    pub path_and_query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RuntimeProxyRequest {
    pub fn from_parts(
        method: impl Into<String>,
        path_and_query: impl Into<String>,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            method: method.into(),
            path_and_query: path_and_query.into(),
            headers,
            body,
        }
    }

    pub fn into_parts(self) -> (String, String, Vec<(String, String)>, Vec<u8>) {
        (self.method, self.path_and_query, self.headers, self.body)
    }
}

pub struct RuntimeBufferedResponseParts {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

pub type RuntimeBufferedResponsePartsTuple = (u16, Vec<(String, Vec<u8>)>, Vec<u8>);

impl RuntimeBufferedResponseParts {
    pub fn from_parts(status: u16, headers: Vec<(String, Vec<u8>)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn from_body_slice(status: u16, headers: Vec<(String, Vec<u8>)>, body: &[u8]) -> Self {
        Self::from_parts(status, headers, body.to_vec())
    }

    pub fn into_parts(self) -> RuntimeBufferedResponsePartsTuple {
        (self.status, self.headers, self.body)
    }
}

#[allow(clippy::large_enum_variant)]
pub enum RuntimeResponsesReply {
    Buffered(RuntimeBufferedResponseParts),
    Streaming(RuntimeStreamingResponse),
}

pub struct RuntimeStreamingResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

#[derive(Debug, Clone)]
pub struct RuntimeAnthropicMessagesRequest {
    pub translated_request: RuntimeProxyRequest,
    pub requested_model: String,
    pub stream: bool,
    pub want_thinking: bool,
    pub server_tools: RuntimeAnthropicServerTools,
    pub carried_web_search_requests: u64,
    pub carried_web_fetch_requests: u64,
    pub carried_code_execution_requests: u64,
    pub carried_tool_search_requests: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProxyClaudeModelAlias {
    Opus,
    Sonnet,
    Haiku,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProxyResponsesModelDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub claude_alias: Option<RuntimeProxyClaudeModelAlias>,
    pub legacy_claude_picker_model: Option<&'static str>,
    pub supports_xhigh: bool,
}

impl RuntimeProxyResponsesModelDescriptor {
    fn claude_picker_value(self) -> &'static str {
        self.claude_alias
            .map(runtime_proxy_claude_alias_picker_value)
            .unwrap_or(self.id)
    }

    fn matches_claude_picker_value(self, picker_model: &str) -> bool {
        self.legacy_claude_picker_model
            .is_some_and(|value| value.eq_ignore_ascii_case(picker_model))
            || self
                .claude_picker_value()
                .eq_ignore_ascii_case(picker_model)
    }
}

pub fn runtime_proxy_claude_model_override() -> Option<String> {
    env::var("PRODEX_CLAUDE_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn runtime_proxy_normalize_responses_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "none" => Some("none"),
        "max" => Some("xhigh"),
        _ => None,
    }
}

pub fn runtime_proxy_claude_reasoning_effort_override() -> Option<String> {
    env::var("PRODEX_CLAUDE_REASONING_EFFORT")
        .ok()
        .and_then(|value| {
            runtime_proxy_normalize_responses_reasoning_effort(value.trim()).map(str::to_string)
        })
}

pub fn runtime_proxy_claude_native_client_tool_enabled(enabled_tokens: &[&str]) -> bool {
    let Some(value) = env::var("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS").ok() else {
        return false;
    };
    let mut enabled = false;
    for token in value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        match token.to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" | "none" => return false,
            "1" | "true" | "yes" | "on" | "all" => enabled = true,
            value if enabled_tokens.contains(&value) => enabled = true,
            _ => {}
        }
    }
    enabled
}

pub fn runtime_proxy_claude_native_shell_enabled() -> bool {
    runtime_proxy_claude_native_client_tool_enabled(&["shell", "bash"])
}

pub fn runtime_proxy_claude_native_computer_enabled() -> bool {
    runtime_proxy_claude_native_client_tool_enabled(&["computer"])
}

pub fn runtime_proxy_responses_model_descriptor(
    model_id: &str,
) -> Option<&'static RuntimeProxyResponsesModelDescriptor> {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .find(|descriptor| descriptor.id.eq_ignore_ascii_case(model_id))
}

pub fn runtime_proxy_responses_model_supported_effort_levels(
    model_id: &str,
) -> &'static [&'static str] {
    if runtime_proxy_responses_model_supports_xhigh(model_id) {
        &["low", "medium", "high", "max"]
    } else {
        &["low", "medium", "high"]
    }
}

pub fn runtime_proxy_responses_model_capabilities(model_id: &str) -> &'static str {
    if runtime_proxy_responses_model_supports_xhigh(model_id) {
        "effort,max_effort,thinking,adaptive_thinking,interleaved_thinking"
    } else {
        "effort,thinking,adaptive_thinking,interleaved_thinking"
    }
}

pub fn runtime_proxy_responses_model_supports_xhigh(model_id: &str) -> bool {
    runtime_proxy_responses_model_descriptor(model_id)
        .map(|descriptor| descriptor.supports_xhigh)
        .unwrap_or_else(|| {
            matches!(
                model_id.trim().to_ascii_lowercase().as_str(),
                value
                    if value.starts_with("gpt-5.2")
                        || value.starts_with("gpt-5.3")
                        || value.starts_with("gpt-5.4")
            )
        })
}

pub fn runtime_proxy_responses_model_supports_native_computer_tool(model_id: &str) -> bool {
    model_id.trim().eq_ignore_ascii_case("gpt-5.4")
}

pub fn runtime_proxy_claude_use_foundry_compat() -> bool {
    true
}

pub fn runtime_proxy_claude_alias_picker_value(
    alias: RuntimeProxyClaudeModelAlias,
) -> &'static str {
    match alias {
        RuntimeProxyClaudeModelAlias::Opus => "opus",
        RuntimeProxyClaudeModelAlias::Sonnet => "sonnet",
        RuntimeProxyClaudeModelAlias::Haiku => "haiku",
    }
}

pub fn runtime_proxy_claude_alias_model(
    alias: RuntimeProxyClaudeModelAlias,
) -> &'static RuntimeProxyResponsesModelDescriptor {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .find(|descriptor| descriptor.claude_alias == Some(alias))
        .expect("Claude alias model should exist")
}

pub fn runtime_proxy_claude_alias_env_keys(
    alias: RuntimeProxyClaudeModelAlias,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match alias {
        RuntimeProxyClaudeModelAlias::Opus => (
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
        ),
        RuntimeProxyClaudeModelAlias::Sonnet => (
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
        ),
        RuntimeProxyClaudeModelAlias::Haiku => (
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
        ),
    }
}

pub fn runtime_proxy_claude_picker_model_descriptor(
    picker_model: &str,
) -> Option<&'static RuntimeProxyResponsesModelDescriptor> {
    let normalized = picker_model.trim();
    let without_extended_context = normalized.strip_suffix("[1m]").unwrap_or(normalized);
    runtime_proxy_responses_model_descriptor(without_extended_context).or_else(|| {
        runtime_proxy_responses_model_descriptors()
            .iter()
            .find(|descriptor| descriptor.matches_claude_picker_value(without_extended_context))
    })
}

pub fn runtime_proxy_responses_model_descriptors() -> &'static [RuntimeProxyResponsesModelDescriptor]
{
    &[
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.4",
            display_name: "GPT-5.4",
            description: "Latest frontier agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Opus),
            legacy_claude_picker_model: Some("claude-opus-4-6"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.4-mini",
            display_name: "GPT-5.4 Mini",
            description: "Smaller frontier agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Haiku),
            legacy_claude_picker_model: Some("claude-haiku-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.3-codex",
            display_name: "GPT-5.3 Codex",
            description: "Frontier Codex-optimized agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Sonnet),
            legacy_claude_picker_model: Some("claude-sonnet-4-6"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.2",
            display_name: "GPT-5.2",
            description: "Optimized for professional work and long-running agents.",
            claude_alias: None,
            legacy_claude_picker_model: Some("claude-opus-4-5"),
            supports_xhigh: true,
        },
    ]
}

pub fn runtime_proxy_claude_pinned_alias_env() -> Vec<(&'static str, OsString)> {
    let mut env = Vec::new();
    for alias in [
        RuntimeProxyClaudeModelAlias::Opus,
        RuntimeProxyClaudeModelAlias::Sonnet,
        RuntimeProxyClaudeModelAlias::Haiku,
    ] {
        let descriptor = runtime_proxy_claude_alias_model(alias);
        let (model_key, name_key, description_key, caps_key) =
            runtime_proxy_claude_alias_env_keys(alias);
        env.push((model_key, OsString::from(descriptor.id)));
        env.push((name_key, OsString::from(descriptor.id)));
        env.push((description_key, OsString::from(descriptor.description)));
        env.push((
            caps_key,
            OsString::from(runtime_proxy_responses_model_capabilities(descriptor.id)),
        ));
    }
    env
}

pub fn runtime_proxy_claude_picker_model(target_model: &str) -> String {
    runtime_proxy_responses_model_descriptor(target_model)
        .map(|descriptor| {
            if runtime_proxy_claude_use_foundry_compat() {
                descriptor.claude_picker_value()
            } else {
                descriptor.id
            }
        })
        .unwrap_or(target_model)
        .to_string()
}

pub fn runtime_proxy_claude_custom_model_option_env(
    target_model: &str,
) -> Vec<(&'static str, OsString)> {
    if runtime_proxy_responses_model_descriptor(target_model).is_some() {
        return Vec::new();
    }

    let descriptor = runtime_proxy_responses_model_descriptor(target_model);
    let display_name = descriptor
        .map(|descriptor| descriptor.display_name)
        .unwrap_or(target_model);
    let description = descriptor
        .map(|descriptor| descriptor.description.to_string())
        .unwrap_or_else(|| format!("Custom OpenAI model routed through prodex ({target_model})"));

    vec![
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION",
            OsString::from(target_model),
        ),
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
            OsString::from(display_name),
        ),
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION",
            OsString::from(description),
        ),
    ]
}

pub fn runtime_proxy_claude_additional_model_option_entries() -> Vec<serde_json::Value> {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .filter(|descriptor| {
            !(runtime_proxy_claude_use_foundry_compat() && descriptor.claude_alias.is_some())
        })
        .map(|descriptor| {
            let supported_effort_levels =
                runtime_proxy_responses_model_supported_effort_levels(descriptor.id);
            serde_json::json!({
                "value": descriptor.id,
                "label": descriptor.id,
                "description": descriptor.description,
                "supportsEffort": true,
                "supportedEffortLevels": supported_effort_levels,
            })
        })
        .collect()
}

pub fn runtime_proxy_claude_managed_model_option_value(value: &str) -> bool {
    runtime_proxy_claude_picker_model_descriptor(value).is_some()
}

pub fn runtime_proxy_claude_target_model(requested_model: &str) -> String {
    if let Some(override_model) = runtime_proxy_claude_model_override() {
        return override_model;
    }

    let normalized = requested_model.trim();
    if let Some(descriptor) = runtime_proxy_responses_model_descriptor(normalized) {
        return descriptor.id.to_string();
    }
    if let Some(descriptor) = runtime_proxy_claude_picker_model_descriptor(normalized) {
        return descriptor.id.to_string();
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("gpt-")
        || lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("codex")
    {
        normalized.to_string()
    } else if lower == "best" || lower == "default" || lower.contains("opus") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Opus)
            .id
            .to_string()
    } else if lower.contains("sonnet") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Sonnet)
            .id
            .to_string()
    } else if lower.contains("haiku") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Haiku)
            .id
            .to_string()
    } else {
        DEFAULT_PRODEX_CLAUDE_MODEL.to_string()
    }
}

pub fn runtime_buffered_response_content_type(
    parts: &RuntimeBufferedResponseParts,
) -> Option<&str> {
    parts.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    const RUNTIME_SSE_INVALID_DATA_MARKER: &str = "\u{0}prodex-invalid-sse-data";
    if data_lines.is_empty()
        || matches!(
            data_lines.first().map(String::as_str),
            Some(RUNTIME_SSE_INVALID_DATA_MARKER)
        )
    {
        return None;
    }

    let payload = data_lines.join("\n");
    let payload = payload.trim_start_matches('\u{feff}');
    serde_json::from_str::<serde_json::Value>(payload).ok()
}

pub fn push_runtime_response_id(response_ids: &mut Vec<String>, id: Option<&str>) {
    if let Some(id) = id
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }
}

pub fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut response_ids = Vec::new();

    push_runtime_response_id(
        &mut response_ids,
        value
            .get("response")
            .and_then(|response| response.get("id"))
            .and_then(serde_json::Value::as_str),
    );
    push_runtime_response_id(
        &mut response_ids,
        value.get("response_id").and_then(serde_json::Value::as_str),
    );

    if value
        .get("object")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|object| object == "response" || object.ends_with(".response"))
    {
        push_runtime_response_id(
            &mut response_ids,
            value.get("id").and_then(serde_json::Value::as_str),
        );
    }

    response_ids
}

pub fn runtime_random_token(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::now_v7())
}

pub fn runtime_proxy_claude_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "x-claude-code-session-id")
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "session_id"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_random_token_uses_uuidv7_not_process_local_counter() {
        let token = runtime_random_token("token");
        let id = token
            .strip_prefix("token-")
            .expect("token should keep requested prefix");

        assert_eq!(id.parse::<uuid::Uuid>().unwrap().get_version_num(), 7);
        assert_ne!(token, format!("token-{}-0-0", std::process::id()));
    }
}
