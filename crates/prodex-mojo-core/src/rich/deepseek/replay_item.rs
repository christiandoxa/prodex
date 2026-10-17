//! Compiled replay/history item classification and emission policy.
//! Rust owns history maps, JSON facts, and applying the returned emission plan.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_deepseek_replay_item_kind_v1(
        abi_version: i64,
        item_type_address: u64,
        item_type_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_deepseek_replay_item_action_v1(
        abi_version: i64,
        kind: i64,
        replayed_call: i64,
        replayed_output: i64,
        output_field_mask: i64,
        text_blank: i64,
        text_duplicate: i64,
        result_address: u64,
        result_capacity: i64,
    ) -> i64;
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekReplayItemKind {
    Text = 0,
    Call = 1,
    Output = 2,
    McpCall = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekReplayItemFacts {
    pub replayed_call: bool,
    pub replayed_output: bool,
    /// Bits 0..3 indicate the presence of output, content, result, and error.
    pub output_field_mask: u8,
    pub text_blank: bool,
    pub text_duplicate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekReplayItemAction {
    pub stream: bool,
    pub emit_output: bool,
}

pub fn deepseek_replay_item_kind(
    item_type: Option<&str>,
) -> Result<DeepSeekReplayItemKind, MojoError> {
    let value = item_type.unwrap_or("");
    let mut tag = -1_i64;
    let status = unsafe {
        prodex_deepseek_replay_item_kind_v1(
            1,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut tag as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match tag {
        0 => Ok(DeepSeekReplayItemKind::Text),
        1 => Ok(DeepSeekReplayItemKind::Call),
        2 => Ok(DeepSeekReplayItemKind::Output),
        3 => Ok(DeepSeekReplayItemKind::McpCall),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Returns None only when the compiled replay policy suppresses this item.
/// Host code does not repeat its call/output and duplicate-text decision tree.
pub fn deepseek_replay_item_action(
    kind: DeepSeekReplayItemKind,
    facts: DeepSeekReplayItemFacts,
) -> Result<Option<DeepSeekReplayItemAction>, MojoError> {
    let mut result = [-1_i64; 3];
    let status = unsafe {
        prodex_deepseek_replay_item_action_v1(
            1,
            kind as i64,
            i64::from(facts.replayed_call),
            i64::from(facts.replayed_output),
            i64::from(facts.output_field_mask),
            i64::from(facts.text_blank),
            i64::from(facts.text_duplicate),
            result.as_mut_ptr() as usize as u64,
            3,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let flag = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let include = flag(result[0])?;
    let stream = flag(result[1])?;
    let emit_output = flag(result[2])?;
    Ok(include.then_some(DeepSeekReplayItemAction {
        stream,
        emit_output,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_replay_item_classifies_call_output_and_text_exactly() {
        for (kind, expected) in [
            (None, DeepSeekReplayItemKind::Text),
            (Some(""), DeepSeekReplayItemKind::Text),
            (Some("mcp_call"), DeepSeekReplayItemKind::McpCall),
            (Some("function_call"), DeepSeekReplayItemKind::Call),
            (Some("custom_tool_call"), DeepSeekReplayItemKind::Call),
            (Some("local_shell_call"), DeepSeekReplayItemKind::Call),
            (Some("function_call_output"), DeepSeekReplayItemKind::Output),
            (
                Some("custom_tool_call_output"),
                DeepSeekReplayItemKind::Output,
            ),
            (Some("mcp_tool_result"), DeepSeekReplayItemKind::Output),
            (Some("mcp_call_output"), DeepSeekReplayItemKind::Output),
            (Some("mcp_call_output_東京"), DeepSeekReplayItemKind::Text),
            (Some("message"), DeepSeekReplayItemKind::Text),
        ] {
            assert_eq!(
                deepseek_replay_item_kind(kind).unwrap(),
                expected,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn compiled_replay_item_actions_preserve_mcp_output_and_deduplication() {
        let facts = DeepSeekReplayItemFacts {
            replayed_call: false,
            replayed_output: false,
            output_field_mask: 0,
            text_blank: false,
            text_duplicate: false,
        };
        let stream = Some(DeepSeekReplayItemAction {
            stream: true,
            emit_output: false,
        });
        let only_output = Some(DeepSeekReplayItemAction {
            stream: false,
            emit_output: true,
        });
        let both = Some(DeepSeekReplayItemAction {
            stream: true,
            emit_output: true,
        });
        assert_eq!(
            deepseek_replay_item_action(DeepSeekReplayItemKind::Text, facts),
            Ok(stream)
        );
        assert_eq!(
            deepseek_replay_item_action(DeepSeekReplayItemKind::Call, facts),
            Ok(stream)
        );
        assert_eq!(
            deepseek_replay_item_action(DeepSeekReplayItemKind::Output, facts),
            Ok(stream)
        );
        for kind in [
            DeepSeekReplayItemKind::Text,
            DeepSeekReplayItemKind::Call,
            DeepSeekReplayItemKind::Output,
            DeepSeekReplayItemKind::McpCall,
        ] {
            let replayed = DeepSeekReplayItemFacts {
                replayed_call: true,
                replayed_output: true,
                text_duplicate: true,
                ..facts
            };
            assert_eq!(
                deepseek_replay_item_action(kind, replayed),
                Ok(None),
                "{kind:?}"
            );
        }
        assert_eq!(
            deepseek_replay_item_action(
                DeepSeekReplayItemKind::Text,
                DeepSeekReplayItemFacts {
                    text_blank: true,
                    ..facts
                },
            ),
            Ok(None)
        );
        assert_eq!(
            deepseek_replay_item_action(
                DeepSeekReplayItemKind::McpCall,
                DeepSeekReplayItemFacts {
                    output_field_mask: 1,
                    ..facts
                },
            ),
            Ok(both)
        );
        assert_eq!(
            deepseek_replay_item_action(
                DeepSeekReplayItemKind::McpCall,
                DeepSeekReplayItemFacts {
                    replayed_call: true,
                    output_field_mask: 8,
                    ..facts
                },
            ),
            Ok(only_output)
        );
        assert_eq!(
            deepseek_replay_item_action(
                DeepSeekReplayItemKind::McpCall,
                DeepSeekReplayItemFacts {
                    replayed_output: true,
                    output_field_mask: 3,
                    ..facts
                },
            ),
            Ok(stream)
        );
        assert_eq!(
            deepseek_replay_item_action(
                DeepSeekReplayItemKind::McpCall,
                DeepSeekReplayItemFacts {
                    replayed_call: true,
                    replayed_output: true,
                    output_field_mask: 15,
                    ..facts
                },
            ),
            Ok(None)
        );
    }

    #[test]
    fn compiled_replay_item_abi_rejects_invalid_flags_without_writing() {
        let kind = b"mcp_call";
        let mut classification = 71_i64;
        let mut output = [88_i64; 3];
        let ptr = kind.as_ptr() as usize as u64;
        let classification_ptr = (&mut classification as *mut i64) as usize as u64;
        let target = output.as_mut_ptr() as usize as u64;
        let invalid = [0xff_u8];
        let class = |version, src, len, dst| unsafe {
            prodex_deepseek_replay_item_kind_v1(version, src, len, dst)
        };
        let action = |version,
                      kind,
                      replay_call,
                      replay_output,
                      mask,
                      blank,
                      duplicate,
                      dst,
                      count| unsafe {
            prodex_deepseek_replay_item_action_v1(
                version,
                kind,
                replay_call,
                replay_output,
                mask,
                blank,
                duplicate,
                dst,
                count,
            )
        };
        assert_eq!(class(2, ptr, 8, classification_ptr), 4);
        assert_eq!(class(1, 0, 8, classification_ptr), 1);
        assert_eq!(class(1, ptr, -1, classification_ptr), 1);
        assert_eq!(class(1, ptr, 8, 0), 1);
        assert_eq!(
            class(1, invalid.as_ptr() as usize as u64, 1, classification_ptr),
            1
        );
        assert_eq!(action(2, 0, 0, 0, 0, 0, 0, target, 3), 4);
        assert_eq!(action(1, 4, 0, 0, 0, 0, 0, target, 3), 1);
        assert_eq!(action(1, 0, 2, 0, 0, 0, 0, target, 3), 1);
        assert_eq!(action(1, 0, 0, -1, 0, 0, 0, target, 3), 1);
        assert_eq!(action(1, 0, 0, 0, 16, 0, 0, target, 3), 1);
        assert_eq!(action(1, 0, 0, 0, 0, 2, 0, target, 3), 1);
        assert_eq!(action(1, 0, 0, 0, 0, 0, 2, target, 3), 1);
        assert_eq!(action(1, 0, 0, 0, 0, 0, 0, target, 2), 1);
        assert_eq!(action(1, 0, 0, 0, 0, 0, 0, 0, 3), 1);
        assert_eq!(classification, 71);
        assert_eq!(output, [88; 3]);
    }
}
