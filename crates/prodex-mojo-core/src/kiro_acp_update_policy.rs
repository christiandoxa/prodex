//! Compiled Kiro ACP session/update discriminant and required-field policy.
//! Serde and typed event materialization remain host responsibilities.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_kiro_acp_session_update_plan_v1(
        abi_version: i64,
        discriminator_present: i64,
        discriminator_address: u64,
        discriminator_length: i64,
        field_mask: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroAcpSessionUpdateKind {
    Unknown,
    UserMessageChunk,
    AgentMessageChunk,
    AgentThoughtChunk,
    ToolCall,
    ToolCallUpdate,
    Plan,
    UsageUpdate,
    SessionInfoUpdate,
    AvailableCommandsUpdate,
    CurrentModeUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroAcpSessionUpdateIssue {
    MissingDiscriminator,
    UserChunkMissingContent,
    AgentChunkMissingContent,
    ThoughtChunkMissingContent,
    ToolCallMissingId,
    ToolCallMissingTitle,
    ToolCallUpdateMissingId,
    PlanMissingEntries,
    UsageMissingUsed,
    UsageMissingSize,
    AvailableCommandsMissingList,
    CurrentModeMissingId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KiroAcpUpdateFieldFacts {
    pub content: bool,
    pub tool_call_id: bool,
    pub title: bool,
    pub entries: bool,
    pub used: bool,
    pub size: bool,
    pub available_commands: bool,
    pub current_mode_id: bool,
}

impl KiroAcpUpdateFieldFacts {
    fn mask(self) -> i64 {
        i64::from(self.content)
            | (i64::from(self.tool_call_id) << 1)
            | (i64::from(self.title) << 2)
            | (i64::from(self.entries) << 3)
            | (i64::from(self.used) << 4)
            | (i64::from(self.size) << 5)
            | (i64::from(self.available_commands) << 6)
            | (i64::from(self.current_mode_id) << 7)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KiroAcpSessionUpdatePlan {
    pub kind: KiroAcpSessionUpdateKind,
    pub issue: Option<KiroAcpSessionUpdateIssue>,
}

/// Mojo owns the update kind, required-field gate and first missing-field
/// precedence. No Rust classifier or fallback is retained.
pub fn kiro_acp_session_update_plan(
    discriminator: Option<&str>,
    facts: KiroAcpUpdateFieldFacts,
) -> Result<KiroAcpSessionUpdatePlan, MojoError> {
    let value = discriminator.unwrap_or("");
    let mut result = [-1_i64; 2];
    let status = unsafe {
        prodex_kiro_acp_session_update_plan_v1(
            1,
            i64::from(discriminator.is_some()),
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            facts.mask(),
            result.as_mut_ptr() as usize as u64,
            result.len() as i64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    use KiroAcpSessionUpdateKind as Kind;
    let kind = match result[0] {
        0 => Kind::Unknown,
        1 => Kind::UserMessageChunk,
        2 => Kind::AgentMessageChunk,
        3 => Kind::AgentThoughtChunk,
        4 => Kind::ToolCall,
        5 => Kind::ToolCallUpdate,
        6 => Kind::Plan,
        7 => Kind::UsageUpdate,
        8 => Kind::SessionInfoUpdate,
        9 => Kind::AvailableCommandsUpdate,
        10 => Kind::CurrentModeUpdate,
        _ => return Err(MojoError::InvalidOutput),
    };
    use KiroAcpSessionUpdateIssue as Issue;
    let issue = match result[1] {
        0 => None,
        1 => Some(Issue::MissingDiscriminator),
        2 => Some(Issue::UserChunkMissingContent),
        3 => Some(Issue::AgentChunkMissingContent),
        4 => Some(Issue::ThoughtChunkMissingContent),
        5 => Some(Issue::ToolCallMissingId),
        6 => Some(Issue::ToolCallMissingTitle),
        7 => Some(Issue::ToolCallUpdateMissingId),
        8 => Some(Issue::PlanMissingEntries),
        9 => Some(Issue::UsageMissingUsed),
        10 => Some(Issue::UsageMissingSize),
        11 => Some(Issue::AvailableCommandsMissingList),
        12 => Some(Issue::CurrentModeMissingId),
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(KiroAcpSessionUpdatePlan { kind, issue })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_kiro_acp_update_kind_distinguishes_all_variants_and_unknown_extensions() {
        use KiroAcpSessionUpdateKind as Kind;
        for (value, kind) in [
            (None, Kind::Unknown),
            (Some("user_message_chunk"), Kind::UserMessageChunk),
            (Some("agent_message_chunk"), Kind::AgentMessageChunk),
            (Some("agent_thought_chunk"), Kind::AgentThoughtChunk),
            (Some("tool_call"), Kind::ToolCall),
            (Some("tool_call_update"), Kind::ToolCallUpdate),
            (Some("plan"), Kind::Plan),
            (Some("usage_update"), Kind::UsageUpdate),
            (Some("session_info_update"), Kind::SessionInfoUpdate),
            (
                Some("available_commands_update"),
                Kind::AvailableCommandsUpdate,
            ),
            (Some("current_mode_update"), Kind::CurrentModeUpdate),
            (Some("unknown_東京🧭"), Kind::Unknown),
            (Some("TOOL_CALL"), Kind::Unknown),
        ] {
            let plan =
                kiro_acp_session_update_plan(value, KiroAcpUpdateFieldFacts::default()).unwrap();
            assert_eq!(plan.kind, kind, "type {value:?}");
        }
    }

    #[test]
    fn compiled_kiro_acp_update_required_field_checks_preserve_original_error_order() {
        use KiroAcpSessionUpdateIssue as Issue;
        let complete = KiroAcpUpdateFieldFacts {
            content: true,
            tool_call_id: true,
            title: true,
            entries: true,
            used: true,
            size: true,
            available_commands: true,
            current_mode_id: true,
        };
        for (name, expected_issue) in [
            (None, Issue::MissingDiscriminator),
            (Some("user_message_chunk"), Issue::UserChunkMissingContent),
            (Some("agent_message_chunk"), Issue::AgentChunkMissingContent),
            (
                Some("agent_thought_chunk"),
                Issue::ThoughtChunkMissingContent,
            ),
            (Some("tool_call"), Issue::ToolCallMissingId),
            (Some("tool_call_update"), Issue::ToolCallUpdateMissingId),
            (Some("plan"), Issue::PlanMissingEntries),
            (Some("usage_update"), Issue::UsageMissingUsed),
            (
                Some("available_commands_update"),
                Issue::AvailableCommandsMissingList,
            ),
            (Some("current_mode_update"), Issue::CurrentModeMissingId),
        ] {
            let plan =
                kiro_acp_session_update_plan(name, KiroAcpUpdateFieldFacts::default()).unwrap();
            assert_eq!(plan.issue, Some(expected_issue), "{name:?}");
        }
        for name in [
            "user_message_chunk",
            "agent_message_chunk",
            "agent_thought_chunk",
            "tool_call",
            "tool_call_update",
            "plan",
            "usage_update",
            "session_info_update",
            "available_commands_update",
            "current_mode_update",
            "future_message_update",
        ] {
            assert_eq!(
                kiro_acp_session_update_plan(Some(name), complete)
                    .unwrap()
                    .issue,
                None,
                "{name}",
            );
        }
        assert_eq!(
            kiro_acp_session_update_plan(
                Some("tool_call"),
                KiroAcpUpdateFieldFacts {
                    tool_call_id: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .issue,
            Some(Issue::ToolCallMissingTitle),
        );
        assert_eq!(
            kiro_acp_session_update_plan(
                Some("usage_update"),
                KiroAcpUpdateFieldFacts {
                    used: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .issue,
            Some(Issue::UsageMissingSize),
        );
    }

    #[test]
    fn compiled_kiro_acp_update_abi_rejects_invalid_inputs_before_writing() {
        let text = b"tool_call";
        let pointer = text.as_ptr() as usize as u64;
        let mut output = [73_i64; 2];
        let dest = output.as_mut_ptr() as usize as u64;
        let call = |version, present, address, length, mask, result, capacity| unsafe {
            prodex_kiro_acp_session_update_plan_v1(
                version, present, address, length, mask, result, capacity,
            )
        };
        assert_eq!(call(2, 1, pointer, 9, 255, dest, 2), 4);
        assert_eq!(call(1, 2, pointer, 9, 255, dest, 2), 1);
        assert_eq!(call(1, 1, 0, 9, 255, dest, 2), 1);
        assert_eq!(call(1, 1, pointer, -1, 255, dest, 2), 1);
        assert_eq!(call(1, 1, pointer, 9, 256, dest, 2), 1);
        assert_eq!(call(1, 1, pointer, 9, -1, dest, 2), 1);
        assert_eq!(call(1, 1, pointer, 9, 255, 0, 2), 1);
        assert_eq!(call(1, 1, pointer, 9, 255, dest, 1), 1);
        let invalid = [0xff_u8];
        assert_eq!(
            call(1, 1, invalid.as_ptr() as usize as u64, 1, 255, dest, 2),
            1
        );
        assert_eq!(output, [73; 2]);
    }
}
