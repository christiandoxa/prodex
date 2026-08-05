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
    tool_activities: Vec<Value>,
    tool_activities_truncated: bool,
    tool_activity_labels: BTreeMap<String, (String, Option<String>)>,
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
                ..
            } => self.apply_tool_activity(
                &tool_call_id,
                Some(title.as_str()),
                Some(status.as_str()),
                kind.as_deref(),
                true,
                raw_input.is_some()
                    || raw_output.is_some()
                    || content.as_ref().is_some_and(|items| !items.is_empty())
                    || locations.as_ref().is_some_and(|items| !items.is_empty()),
            ),
            RuntimeKiroAcpSessionUpdate::ToolCallUpdate {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
                ..
            } => self.apply_tool_activity(
                &tool_call_id,
                title.as_deref(),
                status.as_deref(),
                kind.as_deref(),
                false,
                raw_input.is_some()
                    || raw_output.is_some()
                    || content.as_ref().is_some_and(|items| !items.is_empty())
                    || locations.as_ref().is_some_and(|items| !items.is_empty()),
            ),
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

    fn apply_tool_activity(
        &mut self,
        tool_call_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        kind: Option<&str>,
        initial: bool,
        details_omitted: bool,
    ) {
        if self.tool_activities_truncated {
            return;
        }
        let bounded_id = (tool_call_id.len()
            <= prodex_provider_core::KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_ID_BYTES)
            .then_some(tool_call_id);
        let previous = bounded_id
            .and_then(|tool_call_id| self.tool_activity_labels.get(tool_call_id))
            .cloned();
        let resolved_title = title
            .map(str::to_string)
            .or_else(|| previous.as_ref().map(|(name, _)| name.clone()));
        let resolved_kind = kind
            .map(str::to_string)
            .or_else(|| previous.as_ref().and_then(|(_, kind)| kind.clone()));
        let activity = if self.tool_activities.len()
            < prodex_provider_core::KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS.saturating_sub(1)
        {
            prodex_provider_core::kiro_provider_core_tool_activity_item(
                resolved_title.as_deref(),
                status,
                resolved_kind.as_deref(),
                initial,
                details_omitted,
            )
        } else {
            self.tool_activities_truncated = true;
            prodex_provider_core::kiro_provider_core_truncated_tool_activity_item()
        };
        if !self.tool_activities_truncated
            && bounded_id.is_some()
            && (self.tool_activity_labels.contains_key(tool_call_id)
                || self.tool_activity_labels.len()
                    < prodex_provider_core::KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS)
        {
            self.tool_activity_labels.insert(
                tool_call_id.to_string(),
                (
                    activity["name"]
                        .as_str()
                        .unwrap_or("Kiro internal activity")
                        .to_string(),
                    activity["kind"].as_str().map(str::to_string),
                ),
            );
        }
        self.assistant_text
            .push_str(&prodex_provider_core::kiro_provider_core_tool_activity_text(&activity));
        self.tool_activities.push(activity);
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
            tool_activities: self.tool_activities,
        }
    }
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
pub(super) struct RuntimeKiroAcpTurnState {
    pub(super) assistant_text: String,
    pub(super) reasoning_text: String,
    pub(super) usage_update: Option<Value>,
    pub(super) plan_entries: Option<Vec<Value>>,
    pub(super) available_commands: Option<Vec<Value>>,
    pub(super) current_mode_id: Option<String>,
    pub(super) session_title: Option<String>,
    pub(super) session_updated_at: Option<String>,
    pub(super) tool_activities: Vec<Value>,
}
