use super::{RuntimeKiroAcpCost, RuntimeKiroAcpPromptTurnResult, RuntimeKiroAcpSessionUpdate};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn runtime_kiro_acp_collect_turn_state(
    turn: &RuntimeKiroAcpPromptTurnResult,
) -> RuntimeKiroAcpTurnState {
    let mut state = RuntimeKiroAcpTurnStateBuilder::default();

    for envelope in &turn.notifications {
        let Some(update) = runtime_kiro_acp_session_update(envelope) else {
            continue;
        };
        state.apply(update);
    }

    state.finish()
}

#[derive(Default)]
struct RuntimeKiroAcpTurnStateBuilder {
    assistant_text: String,
    reasoning_text: String,
    usage_update: Option<Value>,
    plan_entries: Option<Vec<Value>>,
    available_commands: Option<Vec<Value>>,
    current_mode_id: Option<String>,
    session_title: Option<String>,
    session_updated_at: Option<String>,
    tool_calls: Vec<RuntimeKiroAcpToolCallState>,
    tool_call_indexes: BTreeMap<String, usize>,
}

impl RuntimeKiroAcpTurnStateBuilder {
    fn apply(&mut self, update: RuntimeKiroAcpSessionUpdate) {
        match update {
            RuntimeKiroAcpSessionUpdate::AgentMessageChunk { content, .. } => {
                Self::append_content(&mut self.assistant_text, &content);
            }
            RuntimeKiroAcpSessionUpdate::AgentThoughtChunk { content, .. } => {
                Self::append_content(&mut self.reasoning_text, &content);
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
            } => self.apply_tool_call(RuntimeKiroAcpToolCallState {
                tool_call_id,
                title: Some(title),
                status: Some(status),
                kind,
                raw_input,
                raw_output,
                content,
                locations,
            }),
            RuntimeKiroAcpSessionUpdate::ToolCallUpdate {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
            } => self.apply_tool_call_update(RuntimeKiroAcpToolCallUpdate {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
            }),
            RuntimeKiroAcpSessionUpdate::UsageUpdate { used, size, cost } => {
                self.usage_update = Some(runtime_kiro_acp_usage_update_json(used, size, cost));
            }
            RuntimeKiroAcpSessionUpdate::Plan { entries } => {
                self.plan_entries = Some(
                    entries
                        .into_iter()
                        .map(|entry| {
                            prodex_provider_core::kiro_provider_core_acp_plan_entry(
                                &entry.content,
                                &entry.priority,
                                &entry.status,
                            )
                        })
                        .collect(),
                );
            }
            RuntimeKiroAcpSessionUpdate::AvailableCommandsUpdate { available_commands } => {
                self.available_commands = Some(available_commands)
            }
            RuntimeKiroAcpSessionUpdate::CurrentModeUpdate { current_mode_id } => {
                self.current_mode_id = Some(current_mode_id);
            }
            RuntimeKiroAcpSessionUpdate::SessionInfoUpdate { title, updated_at } => {
                if title.is_some() {
                    self.session_title = title;
                }
                if updated_at.is_some() {
                    self.session_updated_at = updated_at;
                }
            }
            RuntimeKiroAcpSessionUpdate::UserMessageChunk { .. }
            | RuntimeKiroAcpSessionUpdate::Unknown { .. } => {}
        }
    }

    fn append_content(target: &mut String, content: &Value) {
        if let Some(text) = runtime_kiro_acp_content_text(content) {
            target.push_str(&text);
        }
    }

    fn apply_tool_call(&mut self, tool_call: RuntimeKiroAcpToolCallState) {
        let index = self.tool_call_index(&tool_call.tool_call_id);
        if index == self.tool_calls.len() {
            self.tool_calls.push(tool_call);
        } else if let Some(existing) = self.tool_calls.get_mut(index) {
            *existing = tool_call;
        }
    }

    fn apply_tool_call_update(&mut self, update: RuntimeKiroAcpToolCallUpdate) {
        let RuntimeKiroAcpToolCallUpdate {
            tool_call_id,
            title,
            status,
            content,
            kind,
            raw_input,
            raw_output,
            locations,
        } = update;
        let index = self.tool_call_index(&tool_call_id);
        if index == self.tool_calls.len() {
            self.tool_calls.push(RuntimeKiroAcpToolCallState {
                tool_call_id,
                title,
                status,
                kind,
                raw_input,
                raw_output,
                content,
                locations,
            });
            return;
        }
        if let Some(existing) = self.tool_calls.get_mut(index) {
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

    fn tool_call_index(&mut self, tool_call_id: &str) -> usize {
        let next_index = self.tool_calls.len();
        *self
            .tool_call_indexes
            .entry(tool_call_id.to_string())
            .or_insert(next_index)
    }

    fn finish(self) -> RuntimeKiroAcpTurnState {
        RuntimeKiroAcpTurnState {
            assistant_text: self.assistant_text,
            reasoning_text: self.reasoning_text,
            usage_update: self.usage_update,
            plan_entries: self.plan_entries,
            available_commands: self.available_commands,
            current_mode_id: self.current_mode_id,
            session_title: self.session_title,
            session_updated_at: self.session_updated_at,
            tool_calls: self.tool_calls,
        }
    }
}

struct RuntimeKiroAcpToolCallUpdate {
    tool_call_id: String,
    title: Option<String>,
    status: Option<String>,
    kind: Option<String>,
    raw_input: Option<Value>,
    raw_output: Option<Value>,
    content: Option<Vec<Value>>,
    locations: Option<Vec<Value>>,
}

fn runtime_kiro_acp_session_update(
    envelope: &super::RuntimeKiroAcpEnvelope,
) -> Option<RuntimeKiroAcpSessionUpdate> {
    (envelope.method.as_deref() == Some("session/update"))
        .then(|| envelope.parse_session_notification().ok())
        .flatten()
        .map(|notification| notification.update)
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
