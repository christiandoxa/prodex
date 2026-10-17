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
                status.as_deref(),
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
        let previous = self.tool_activity_labels.get(tool_call_id).cloned();
        let plan = prodex_mojo_core::rich::kiro_acp_tool_activity_plan(
            prodex_mojo_core::rich::KiroAcpToolActivityFacts {
                tool_call_id,
                already_truncated: self.tool_activities_truncated,
                activity_count: self.tool_activities.len(),
                label_count: self.tool_activity_labels.len(),
                previous_present: previous.is_some(),
                title_present: title.is_some(),
                kind_present: kind.is_some(),
            },
        )
        .expect("compiled Mojo Kiro ACP tool-activity ledger returned invalid output");
        let resolved_title = if plan.use_previous_title {
            previous.as_ref().map(|(name, _)| name.clone())
        } else {
            title.map(str::to_string)
        };
        let resolved_kind = if plan.use_previous_kind {
            previous.as_ref().and_then(|(_, kind)| kind.clone())
        } else {
            kind.map(str::to_string)
        };
        let activity = match plan.action {
            prodex_mojo_core::rich::KiroAcpToolActivityAction::Ignore => return,
            prodex_mojo_core::rich::KiroAcpToolActivityAction::Append => {
                prodex_provider_core::kiro_provider_core_tool_activity_item(
                    resolved_title.as_deref(),
                    status,
                    resolved_kind.as_deref(),
                    initial,
                    details_omitted,
                )
            }
            prodex_mojo_core::rich::KiroAcpToolActivityAction::AppendTruncated => {
                self.tool_activities_truncated = true;
                prodex_provider_core::kiro_provider_core_truncated_tool_activity_item()
            }
        };
        if plan.record_label {
            self.tool_activity_labels.insert(
                tool_call_id.to_string(),
                (
                    activity["name"]
                        .as_str()
                        .expect("compiled Mojo Kiro tool activity omitted name")
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

#[cfg(test)]
mod tool_activity_ledger_tests {
    use super::*;

    #[test]
    fn kiro_acp_tool_activity_ledger_uses_compiled_mojo_cap_and_title_inheritance() {
        let mut state = RuntimeKiroAcpTurnStateBuilder::default();
        state.apply_tool_activity(
            "call-1",
            Some("Read file"),
            Some("in_progress"),
            Some("read"),
            true,
            true,
        );
        state.apply_tool_activity("call-1", None, Some("completed"), None, false, true);
        assert_eq!(state.tool_activities.len(), 2);
        assert_eq!(state.tool_activities[0]["name"], "Read file");
        assert_eq!(state.tool_activities[1]["name"], "Read file");
        assert_eq!(state.tool_activities[1]["kind"], "read");
        assert_eq!(state.tool_activities[1]["phase"], "completed");
        assert_eq!(state.tool_activity_labels.len(), 1);

        for index in 0..130 {
            state.apply_tool_activity(
                &format!("call-{index}"),
                Some("Tool"),
                None,
                None,
                true,
                false,
            );
        }
        assert_eq!(state.tool_activities.len(), 128);
        assert!(state.tool_activities_truncated);
        assert_eq!(
            state.tool_activities[127]["name"],
            "Additional Kiro activities omitted",
        );
        // The generated call-1 updates the existing label rather than adding a new ID.
        assert_eq!(state.tool_activity_labels.len(), 125);
    }

    #[test]
    fn kiro_acp_tool_activity_ledger_never_caches_oversized_unicode_ids() {
        let mut state = RuntimeKiroAcpTurnStateBuilder::default();
        let oversized_id = "東京".repeat(86);
        assert_eq!(oversized_id.len(), 516);
        state.apply_tool_activity(
            &oversized_id,
            Some("Private activity"),
            Some("in_progress"),
            Some("read"),
            true,
            true,
        );
        state.apply_tool_activity(&oversized_id, None, Some("completed"), None, false, false);
        assert_eq!(state.tool_activities.len(), 2);
        assert!(state.tool_activity_labels.is_empty());
        assert_ne!(state.tool_activities[1]["name"], "Private activity");
        assert!(!state.tool_activities_truncated);
    }
}
