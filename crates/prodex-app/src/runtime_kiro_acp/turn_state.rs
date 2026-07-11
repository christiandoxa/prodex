use super::{RuntimeKiroAcpCost, RuntimeKiroAcpPromptTurnResult, RuntimeKiroAcpSessionUpdate};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn runtime_kiro_acp_collect_turn_state(
    turn: &RuntimeKiroAcpPromptTurnResult,
) -> RuntimeKiroAcpTurnState {
    let mut assistant_text = String::new();
    let mut reasoning_text = String::new();
    let mut usage_update = None;
    let mut plan_entries = None;
    let mut available_commands = None;
    let mut current_mode_id = None;
    let mut session_title = None;
    let mut session_updated_at = None;
    let mut tool_calls = Vec::<RuntimeKiroAcpToolCallState>::new();
    let mut tool_call_indexes = BTreeMap::<String, usize>::new();

    for envelope in &turn.notifications {
        let Some(method) = envelope.method.as_deref() else {
            continue;
        };
        if method != "session/update" {
            continue;
        }
        let Ok(notification) = envelope.parse_session_notification() else {
            continue;
        };
        match notification.update {
            RuntimeKiroAcpSessionUpdate::AgentMessageChunk { content, .. } => {
                if let Some(text) = runtime_kiro_acp_content_text(&content) {
                    assistant_text.push_str(&text);
                }
            }
            RuntimeKiroAcpSessionUpdate::AgentThoughtChunk { content, .. } => {
                if let Some(text) = runtime_kiro_acp_content_text(&content) {
                    reasoning_text.push_str(&text);
                }
            }
            RuntimeKiroAcpSessionUpdate::ToolCall {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
            } => {
                let next_index = tool_calls.len();
                let index = *tool_call_indexes
                    .entry(tool_call_id.clone())
                    .or_insert(next_index);
                if index == next_index {
                    tool_calls.push(RuntimeKiroAcpToolCallState {
                        tool_call_id,
                        title: Some(title),
                        status: Some(status),
                        kind,
                        raw_input,
                        raw_output,
                        content,
                        locations,
                    });
                } else if let Some(existing) = tool_calls.get_mut(index) {
                    existing.title = Some(title);
                    existing.status = Some(status);
                    existing.kind = kind;
                    existing.raw_input = raw_input;
                    existing.raw_output = raw_output;
                    existing.content = content;
                    existing.locations = locations;
                }
            }
            RuntimeKiroAcpSessionUpdate::ToolCallUpdate {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
            } => {
                let next_index = tool_calls.len();
                let index = *tool_call_indexes
                    .entry(tool_call_id.clone())
                    .or_insert(next_index);
                if index == next_index {
                    tool_calls.push(RuntimeKiroAcpToolCallState {
                        tool_call_id,
                        title,
                        status,
                        kind,
                        raw_input,
                        raw_output,
                        content,
                        locations,
                    });
                } else if let Some(existing) = tool_calls.get_mut(index) {
                    if title.is_some() {
                        existing.title = title;
                    }
                    if status.is_some() {
                        existing.status = status;
                    }
                    if kind.is_some() {
                        existing.kind = kind;
                    }
                    if raw_input.is_some() {
                        existing.raw_input = raw_input;
                    }
                    if raw_output.is_some() {
                        existing.raw_output = raw_output;
                    }
                    if content.is_some() {
                        existing.content = content;
                    }
                    if locations.is_some() {
                        existing.locations = locations;
                    }
                }
            }
            RuntimeKiroAcpSessionUpdate::UsageUpdate { used, size, cost } => {
                usage_update = Some(runtime_kiro_acp_usage_update_json(used, size, cost));
            }
            RuntimeKiroAcpSessionUpdate::Plan { entries } => {
                plan_entries = Some(
                    entries
                        .into_iter()
                        .map(|entry| {
                            prodex_provider_core::kiro_provider_core_acp_plan_entry(
                                &entry.content,
                                &entry.priority,
                                &entry.status,
                            )
                        })
                        .collect::<Vec<_>>(),
                );
            }
            RuntimeKiroAcpSessionUpdate::AvailableCommandsUpdate {
                available_commands: commands,
            } => {
                available_commands = Some(commands);
            }
            RuntimeKiroAcpSessionUpdate::CurrentModeUpdate {
                current_mode_id: mode_id,
            } => {
                current_mode_id = Some(mode_id);
            }
            RuntimeKiroAcpSessionUpdate::SessionInfoUpdate { title, updated_at } => {
                if title.is_some() {
                    session_title = title;
                }
                if updated_at.is_some() {
                    session_updated_at = updated_at;
                }
            }
            RuntimeKiroAcpSessionUpdate::UserMessageChunk { .. }
            | RuntimeKiroAcpSessionUpdate::Unknown { .. } => {}
        }
    }

    RuntimeKiroAcpTurnState {
        assistant_text,
        reasoning_text,
        usage_update,
        plan_entries,
        available_commands,
        current_mode_id,
        session_title,
        session_updated_at,
        tool_calls,
    }
}

fn runtime_kiro_acp_content_text(value: &Value) -> Option<String> {
    prodex_provider_core::kiro_provider_core_stream_content_text(value)
}

fn runtime_kiro_acp_usage_update_json(
    used: u64,
    size: u64,
    cost: Option<RuntimeKiroAcpCost>,
) -> Value {
    prodex_provider_core::kiro_provider_core_acp_usage_update_json(
        used,
        size,
        cost.as_ref()
            .map(|cost| (cost.amount, cost.currency.as_str())),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RuntimeKiroAcpToolCallState {
    pub(super) tool_call_id: String,
    pub(super) title: Option<String>,
    pub(super) status: Option<String>,
    pub(super) kind: Option<String>,
    pub(super) raw_input: Option<Value>,
    pub(super) raw_output: Option<Value>,
    pub(super) content: Option<Vec<Value>>,
    pub(super) locations: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RuntimeKiroAcpTurnState {
    pub(super) assistant_text: String,
    pub(super) reasoning_text: String,
    pub(super) usage_update: Option<Value>,
    pub(super) plan_entries: Option<Vec<Value>>,
    pub(super) available_commands: Option<Vec<Value>>,
    pub(super) current_mode_id: Option<String>,
    pub(super) session_title: Option<String>,
    pub(super) session_updated_at: Option<String>,
    pub(super) tool_calls: Vec<RuntimeKiroAcpToolCallState>,
}
