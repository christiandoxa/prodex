use anyhow::{Context, Result, bail};
use prodex_mojo_core::kiro_acp_update_policy::{
    KiroAcpSessionUpdateIssue, KiroAcpSessionUpdateKind, KiroAcpUpdateFieldFacts,
    kiro_acp_session_update_plan,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct RuntimeKiroAcpEnvelope {
    pub(crate) jsonrpc: String,
    #[serde(default)]
    pub(crate) id: Option<Value>,
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
    pub(crate) fn numeric_id(&self) -> Option<u64> {
        self.id.as_ref().and_then(|id| {
            id.as_u64()
                .or_else(|| id.as_str().and_then(|id| id.parse().ok()))
        })
    }

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

    #[cfg(test)]
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
    #[cfg(test)]
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
#[cfg(test)]
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
        status: Option<String>,
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
        let session_update = value.get("sessionUpdate").and_then(Value::as_str);
        let plan = kiro_acp_session_update_plan(
            session_update,
            KiroAcpUpdateFieldFacts {
                content: value.get("content").is_some(),
                tool_call_id: value.get("toolCallId").and_then(Value::as_str).is_some(),
                title: value.get("title").and_then(Value::as_str).is_some(),
                entries: value.get("entries").is_some(),
                used: value.get("used").and_then(Value::as_u64).is_some(),
                size: value.get("size").and_then(Value::as_u64).is_some(),
                available_commands: value
                    .get("availableCommands")
                    .and_then(Value::as_array)
                    .is_some(),
                current_mode_id: value.get("currentModeId").and_then(Value::as_str).is_some(),
            },
        )
        .map_err(|error| {
            anyhow::anyhow!("compiled Mojo Kiro ACP session/update policy failed: {error:?}")
        })?;
        if let Some(issue) = plan.issue {
            let message = match issue {
                KiroAcpSessionUpdateIssue::MissingDiscriminator => {
                    "Kiro ACP session/update is missing sessionUpdate discriminator"
                }
                KiroAcpSessionUpdateIssue::UserChunkMissingContent => {
                    "user_message_chunk is missing content"
                }
                KiroAcpSessionUpdateIssue::AgentChunkMissingContent => {
                    "agent_message_chunk is missing content"
                }
                KiroAcpSessionUpdateIssue::ThoughtChunkMissingContent => {
                    "agent_thought_chunk is missing content"
                }
                KiroAcpSessionUpdateIssue::ToolCallMissingId => "tool_call is missing toolCallId",
                KiroAcpSessionUpdateIssue::ToolCallMissingTitle => "tool_call is missing title",
                KiroAcpSessionUpdateIssue::ToolCallUpdateMissingId => {
                    "tool_call_update is missing toolCallId"
                }
                KiroAcpSessionUpdateIssue::PlanMissingEntries => "plan update is missing entries",
                KiroAcpSessionUpdateIssue::UsageMissingUsed => "usage_update is missing used",
                KiroAcpSessionUpdateIssue::UsageMissingSize => "usage_update is missing size",
                KiroAcpSessionUpdateIssue::AvailableCommandsMissingList => {
                    "available_commands_update is missing availableCommands"
                }
                KiroAcpSessionUpdateIssue::CurrentModeMissingId => {
                    "current_mode_update is missing currentModeId"
                }
            };
            bail!("{message}");
        }
        let session_update = session_update
            .expect("compiled Mojo Kiro ACP update plan validated discriminator")
            .to_owned();
        Ok(match plan.kind {
            KiroAcpSessionUpdateKind::UserMessageChunk => Self::UserMessageChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .expect("Mojo checked user_message_chunk content"),
            },
            KiroAcpSessionUpdateKind::AgentMessageChunk => Self::AgentMessageChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .expect("Mojo checked agent_message_chunk content"),
            },
            KiroAcpSessionUpdateKind::AgentThoughtChunk => Self::AgentThoughtChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .expect("Mojo checked agent_thought_chunk content"),
            },
            KiroAcpSessionUpdateKind::ToolCall => Self::ToolCall {
                tool_call_id: value
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .expect("Mojo checked tool_call ID"),
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .expect("Mojo checked tool_call title"),
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
            KiroAcpSessionUpdateKind::ToolCallUpdate => Self::ToolCallUpdate {
                tool_call_id: value
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .expect("Mojo checked tool_call_update ID"),
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
            KiroAcpSessionUpdateKind::Plan => Self::Plan {
                entries: serde_json::from_value(
                    value
                        .get("entries")
                        .cloned()
                        .expect("Mojo checked plan entries"),
                )
                .context("failed to parse plan entries")?,
            },
            KiroAcpSessionUpdateKind::UsageUpdate => Self::UsageUpdate {
                used: value
                    .get("used")
                    .and_then(Value::as_u64)
                    .expect("Mojo checked usage_update used"),
                size: value
                    .get("size")
                    .and_then(Value::as_u64)
                    .expect("Mojo checked usage_update size"),
                cost: value
                    .get("cost")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .context("failed to parse usage_update cost")?,
            },
            KiroAcpSessionUpdateKind::SessionInfoUpdate => Self::SessionInfoUpdate {
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                updated_at: value
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
            KiroAcpSessionUpdateKind::AvailableCommandsUpdate => Self::AvailableCommandsUpdate {
                available_commands: value
                    .get("availableCommands")
                    .and_then(Value::as_array)
                    .cloned()
                    .expect("Mojo checked available_commands_update list"),
            },
            KiroAcpSessionUpdateKind::CurrentModeUpdate => Self::CurrentModeUpdate {
                current_mode_id: value
                    .get("currentModeId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .expect("Mojo checked current_mode_update ID"),
            },
            KiroAcpSessionUpdateKind::Unknown => Self::Unknown {
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

#[cfg(test)]
mod mojo_session_update_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compiled_kiro_acp_session_update_caller_preserves_protocol_shapes_and_unicode() {
        use RuntimeKiroAcpSessionUpdate as Update;
        let user = Update::parse_value(json!({
            "sessionUpdate":"user_message_chunk",
            "messageId":"msg_東京",
            "content":null
        }))
        .unwrap();
        assert!(matches!(user, Update::UserMessageChunk {
            message_id: Some(ref id), content: Value::Null
        } if id == "msg_東京"));

        for name in ["agent_message_chunk", "agent_thought_chunk"] {
            let update = Update::parse_value(json!({
                "sessionUpdate":name,
                "content":{"type":"text","text":"Snow 雪"}
            }))
            .unwrap();
            assert!(matches!(
                update,
                Update::AgentMessageChunk { .. } | Update::AgentThoughtChunk { .. }
            ));
        }
        let call = Update::parse_value(json!({
            "sessionUpdate":"tool_call",
            "toolCallId":"call_🚀",
            "title":"Read config",
            "rawInput":{"path":"src/main.rs"}
        }))
        .unwrap();
        assert!(
            matches!(call, Update::ToolCall { ref tool_call_id, ref title, .. }
            if tool_call_id == "call_🚀" && title == "Read config")
        );
        let updated = Update::parse_value(json!({
            "sessionUpdate":"tool_call_update",
            "toolCallId":"call_🚀"
        }))
        .unwrap();
        assert!(matches!(updated, Update::ToolCallUpdate { ref title, .. } if title.is_none()));
        let plan = Update::parse_value(json!({
            "sessionUpdate":"plan",
            "entries":[{"content":"Design","priority":"medium","status":"pending"}]
        }))
        .unwrap();
        assert!(matches!(plan, Update::Plan { ref entries } if entries.len() == 1));
        let usage = Update::parse_value(json!({
            "sessionUpdate":"usage_update","used":0,"size":42
        }))
        .unwrap();
        assert!(matches!(
            usage,
            Update::UsageUpdate {
                used: 0,
                size: 42,
                cost: None
            }
        ));
        let info = Update::parse_value(json!({
            "sessionUpdate":"session_info_update","title":"🔍"
        }))
        .unwrap();
        assert!(matches!(info, Update::SessionInfoUpdate { ref title, .. }
            if title.as_deref() == Some("🔍")));
        let commands = Update::parse_value(json!({
            "sessionUpdate":"available_commands_update",
            "availableCommands":[{"name":"test"}]
        }))
        .unwrap();
        assert!(matches!(commands, Update::AvailableCommandsUpdate {
            ref available_commands
        } if available_commands.len() == 1));
        let mode = Update::parse_value(json!({
            "sessionUpdate":"current_mode_update","currentModeId":"mode_🧭"
        }))
        .unwrap();
        assert!(
            matches!(mode, Update::CurrentModeUpdate { ref current_mode_id }
            if current_mode_id == "mode_🧭")
        );
        let unknown = Update::parse_value(json!({
            "sessionUpdate":"future_session_type",
            "unexpected":{"nested":[1,2,3]}
        }))
        .unwrap();
        assert!(
            matches!(unknown, Update::Unknown { ref session_update, ref raw }
            if session_update == "future_session_type" && raw["unexpected"]["nested"][2] == 3)
        );
    }

    #[test]
    fn compiled_kiro_acp_session_update_caller_preserves_required_field_failure_precedence() {
        let cases = [
            (json!({}), "missing sessionUpdate discriminator"),
            (
                json!({"sessionUpdate":"user_message_chunk"}),
                "user_message_chunk is missing content",
            ),
            (
                json!({"sessionUpdate":"agent_message_chunk"}),
                "agent_message_chunk is missing content",
            ),
            (
                json!({"sessionUpdate":"agent_thought_chunk"}),
                "agent_thought_chunk is missing content",
            ),
            (
                json!({"sessionUpdate":"tool_call","title":"title"}),
                "tool_call is missing toolCallId",
            ),
            (
                json!({"sessionUpdate":"tool_call","toolCallId":"id"}),
                "tool_call is missing title",
            ),
            (
                json!({"sessionUpdate":"tool_call_update"}),
                "tool_call_update is missing toolCallId",
            ),
            (
                json!({"sessionUpdate":"plan"}),
                "plan update is missing entries",
            ),
            (
                json!({"sessionUpdate":"usage_update"}),
                "usage_update is missing used",
            ),
            (
                json!({"sessionUpdate":"usage_update","used":0}),
                "usage_update is missing size",
            ),
            (
                json!({"sessionUpdate":"available_commands_update","availableCommands":{}}),
                "available_commands_update is missing availableCommands",
            ),
            (
                json!({"sessionUpdate":"current_mode_update","currentModeId":123}),
                "current_mode_update is missing currentModeId",
            ),
        ];
        for (value, expected) in cases {
            let error = RuntimeKiroAcpSessionUpdate::parse_value(value.clone()).unwrap_err();
            assert!(error.to_string().contains(expected), "{value:?}: {error:#}");
        }
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session_1","update":{"sessionUpdate":"usage_update","used":1,"size":5}}}"#
        ).unwrap();
        let notification = envelope.parse_session_notification().unwrap();
        assert_eq!(notification.session_id, "session_1");
        assert!(matches!(
            notification.update,
            RuntimeKiroAcpSessionUpdate::UsageUpdate {
                used: 1,
                size: 5,
                ..
            }
        ));
    }
}
