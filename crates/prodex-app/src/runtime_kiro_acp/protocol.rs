use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct RuntimeKiroAcpEnvelope {
    pub(crate) jsonrpc: String,
    #[serde(default)]
    pub(crate) id: Option<u64>,
    #[serde(default)]
    pub(crate) method: Option<String>,
    #[serde(default)]
    pub(crate) params: Option<Value>,
    #[serde(default)]
    pub(crate) result: Option<Value>,
    #[serde(default)]
    pub(crate) error: Option<RuntimeKiroAcpError>,
}

impl RuntimeKiroAcpEnvelope {
    pub(crate) fn parse(line: &str) -> Result<Self> {
        let envelope: Self =
            serde_json::from_str(line).context("failed to parse Kiro ACP JSON-RPC line")?;
        if envelope.jsonrpc != "2.0" {
            bail!(
                "unsupported Kiro ACP jsonrpc version '{}'",
                envelope.jsonrpc
            );
        }
        Ok(envelope)
    }

    pub(crate) fn parse_initialize_result(&self) -> Result<RuntimeKiroAcpInitializeResult> {
        serde_json::from_value(
            self.result
                .clone()
                .context("missing Kiro ACP initialize result")?,
        )
        .context("failed to parse Kiro ACP initialize result")
    }

    pub(crate) fn parse_session_new_result(&self) -> Result<RuntimeKiroAcpNewSessionResult> {
        serde_json::from_value(
            self.result
                .clone()
                .context("missing Kiro ACP session/new result")?,
        )
        .context("failed to parse Kiro ACP session/new result")
    }

    pub(crate) fn parse_prompt_response(&self) -> Result<RuntimeKiroAcpPromptResponse> {
        serde_json::from_value(
            self.result
                .clone()
                .context("missing Kiro ACP session/prompt response")?,
        )
        .context("failed to parse Kiro ACP session/prompt response")
    }

    pub(crate) fn parse_session_notification(&self) -> Result<RuntimeKiroAcpSessionNotification> {
        if self.method.as_deref() != Some("session/update") {
            bail!(
                "expected Kiro ACP session/update notification, got {:?}",
                self.method
            );
        }
        let params = self
            .params
            .clone()
            .context("missing Kiro ACP session/update params")?;
        RuntimeKiroAcpSessionNotification::parse_value(params)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpError {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpInitializeResult {
    pub(crate) protocol_version: u64,
    pub(crate) agent_capabilities: RuntimeKiroAcpAgentCapabilities,
    #[serde(default)]
    pub(crate) auth_methods: Vec<RuntimeKiroAcpAuthMethod>,
    pub(crate) agent_info: RuntimeKiroAcpAgentInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpAgentCapabilities {
    pub(crate) load_session: bool,
    pub(crate) prompt_capabilities: RuntimeKiroAcpPromptCapabilities,
    pub(crate) mcp_capabilities: RuntimeKiroAcpMcpCapabilities,
    #[serde(default)]
    pub(crate) session_capabilities: Value,
    #[serde(default)]
    pub(crate) auth: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpPromptCapabilities {
    pub(crate) image: bool,
    pub(crate) audio: bool,
    pub(crate) embedded_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpMcpCapabilities {
    pub(crate) http: bool,
    pub(crate) sse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpAuthMethod {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpAgentInfo {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpNewSessionResult {
    pub(crate) session_id: String,
    #[serde(default)]
    pub(crate) modes: Option<RuntimeKiroAcpModeState>,
    #[serde(default)]
    pub(crate) models: Option<RuntimeKiroAcpModelState>,
}

impl RuntimeKiroAcpNewSessionResult {
    pub(crate) fn model_ids(&self) -> Vec<&str> {
        self.models
            .as_ref()
            .map(|models| {
                models
                    .available_models
                    .iter()
                    .map(|model| model.model_id.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpPromptResponse {
    #[serde(alias = "stop_reason")]
    pub(crate) stop_reason: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpSessionNotification {
    pub(crate) session_id: String,
    pub(crate) update: RuntimeKiroAcpSessionUpdate,
}

impl RuntimeKiroAcpSessionNotification {
    fn parse_value(value: Value) -> Result<Self> {
        let session_id = value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("Kiro ACP session/update is missing sessionId")?;
        let update_value = value
            .get("update")
            .cloned()
            .context("Kiro ACP session/update is missing update payload")?;
        let update = RuntimeKiroAcpSessionUpdate::parse_value(update_value)?;
        Ok(Self { session_id, update })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) enum RuntimeKiroAcpSessionUpdate {
    UserMessageChunk {
        message_id: Option<String>,
        content: Value,
    },
    AgentMessageChunk {
        message_id: Option<String>,
        content: Value,
    },
    AgentThoughtChunk {
        message_id: Option<String>,
        content: Value,
    },
    ToolCall {
        tool_call_id: String,
        title: String,
        status: String,
        content: Option<Vec<Value>>,
        kind: Option<String>,
        raw_input: Option<Value>,
        raw_output: Option<Value>,
        locations: Option<Vec<Value>>,
    },
    ToolCallUpdate {
        tool_call_id: String,
        title: Option<String>,
        status: Option<String>,
        content: Option<Vec<Value>>,
        kind: Option<String>,
        raw_input: Option<Value>,
        raw_output: Option<Value>,
        locations: Option<Vec<Value>>,
    },
    Plan {
        entries: Vec<RuntimeKiroAcpPlanEntry>,
    },
    UsageUpdate {
        used: u64,
        size: u64,
        cost: Option<RuntimeKiroAcpCost>,
    },
    SessionInfoUpdate {
        title: Option<String>,
        updated_at: Option<String>,
    },
    AvailableCommandsUpdate {
        available_commands: Vec<Value>,
    },
    CurrentModeUpdate {
        current_mode_id: String,
    },
    Unknown {
        session_update: String,
        raw: Value,
    },
}

impl RuntimeKiroAcpSessionUpdate {
    fn parse_value(value: Value) -> Result<Self> {
        let session_update = value
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("Kiro ACP session/update is missing sessionUpdate discriminator")?;
        Ok(match session_update.as_str() {
            "user_message_chunk" => Self::UserMessageChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .context("user_message_chunk is missing content")?,
            },
            "agent_message_chunk" => Self::AgentMessageChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .context("agent_message_chunk is missing content")?,
            },
            "agent_thought_chunk" => Self::AgentThoughtChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .context("agent_thought_chunk is missing content")?,
            },
            "tool_call" => Self::ToolCall {
                tool_call_id: value
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call is missing toolCallId")?,
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call is missing title")?,
                status: value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call is missing status")?,
                content: value.get("content").and_then(Value::as_array).cloned(),
                kind: value
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                raw_input: value.get("rawInput").cloned(),
                raw_output: value.get("rawOutput").cloned(),
                locations: value.get("locations").and_then(Value::as_array).cloned(),
            },
            "tool_call_update" => Self::ToolCallUpdate {
                tool_call_id: value
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call_update is missing toolCallId")?,
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                status: value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value.get("content").and_then(Value::as_array).cloned(),
                kind: value
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                raw_input: value.get("rawInput").cloned(),
                raw_output: value.get("rawOutput").cloned(),
                locations: value.get("locations").and_then(Value::as_array).cloned(),
            },
            "plan" => Self::Plan {
                entries: serde_json::from_value(
                    value
                        .get("entries")
                        .cloned()
                        .context("plan update is missing entries")?,
                )
                .context("failed to parse plan entries")?,
            },
            "usage_update" => Self::UsageUpdate {
                used: value
                    .get("used")
                    .and_then(Value::as_u64)
                    .context("usage_update is missing used")?,
                size: value
                    .get("size")
                    .and_then(Value::as_u64)
                    .context("usage_update is missing size")?,
                cost: value
                    .get("cost")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .context("failed to parse usage_update cost")?,
            },
            "session_info_update" => Self::SessionInfoUpdate {
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                updated_at: value
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
            "available_commands_update" => Self::AvailableCommandsUpdate {
                available_commands: value
                    .get("availableCommands")
                    .and_then(Value::as_array)
                    .cloned()
                    .context("available_commands_update is missing availableCommands")?,
            },
            "current_mode_update" => Self::CurrentModeUpdate {
                current_mode_id: value
                    .get("currentModeId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("current_mode_update is missing currentModeId")?,
            },
            _ => Self::Unknown {
                session_update,
                raw: value,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpPlanEntry {
    pub(crate) content: String,
    pub(crate) priority: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct RuntimeKiroAcpCost {
    pub(crate) amount: f64,
    pub(crate) currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpModeState {
    pub(crate) current_mode_id: String,
    #[serde(default)]
    pub(crate) available_modes: Vec<RuntimeKiroAcpMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpMode {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default, rename = "_meta")]
    pub(crate) meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpModelState {
    pub(crate) current_model_id: String,
    #[serde(default)]
    pub(crate) available_models: Vec<RuntimeKiroAcpModelInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpModelInfo {
    pub(crate) model_id: String,
    pub(crate) name: String,
}
