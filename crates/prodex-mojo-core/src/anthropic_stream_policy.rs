//! Compiled Anthropic SSE event and finish-reason planner.
//! Rust retains protocol I/O, JSON acquisition, state maps, and event output.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_anthropic_stream_action_v1(
        version: i64,
        event_address: u64,
        event_length: i64,
        block_address: u64,
        block_length: i64,
        delta_address: u64,
        delta_length: i64,
        name_address: u64,
        name_length: i64,
        block_present: i64,
        block_type_present: i64,
        delta_type_present: i64,
        ignored_block: i64,
        active_web_search: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_anthropic_web_search_completion_v1(
        version: i64,
        id_views_address: u64,
        state_address: u64,
        candidate_count: i64,
        target_present: i64,
        target_address: u64,
        target_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_anthropic_stream_finish_reason_v1(
        version: i64,
        present: i64,
        address: u64,
        length: i64,
        result_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicStreamFacts<'a> {
    pub event_type: Option<&'a str>,
    pub block_present: bool,
    pub block_type: Option<&'a str>,
    pub block_name: Option<&'a str>,
    pub delta_type: Option<&'a str>,
    pub ignored_block: bool,
    pub active_web_search: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicStreamAction {
    Ignore,
    MessageStart,
    StartToolUse,
    StartWebSearch,
    WebSearchResult,
    MissingBlock,
    MissingBlockType,
    DeltaText,
    DeltaArguments,
    DeltaThinking,
    DeltaSearchInput,
    MissingDeltaType,
    MessageDelta,
    MessageStop,
    Error,
    BlockStop,
}

fn check_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Compiled policy determines event dispatch and ignored/search block
/// precedence; host cannot silently recalculate it.
pub fn anthropic_stream_action(
    facts: AnthropicStreamFacts<'_>,
) -> Result<AnthropicStreamAction, MojoError> {
    let event = facts.event_type.unwrap_or("");
    let block = facts.block_type.unwrap_or("");
    let delta = facts.delta_type.unwrap_or("");
    let name = facts.block_name.unwrap_or("");
    let mut result = -1_i64;
    check_status(unsafe {
        prodex_anthropic_stream_action_v1(
            1,
            event.as_ptr() as usize as u64,
            i64::try_from(event.len()).map_err(|_| MojoError::InvalidInput)?,
            block.as_ptr() as usize as u64,
            i64::try_from(block.len()).map_err(|_| MojoError::InvalidInput)?,
            delta.as_ptr() as usize as u64,
            i64::try_from(delta.len()).map_err(|_| MojoError::InvalidInput)?,
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(facts.block_present),
            i64::from(facts.block_type.is_some()),
            i64::from(facts.delta_type.is_some()),
            i64::from(facts.ignored_block),
            i64::from(facts.active_web_search),
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    use AnthropicStreamAction::*;
    match result {
        0 => Ok(Ignore),
        1 => Ok(MessageStart),
        2 => Ok(StartToolUse),
        3 => Ok(StartWebSearch),
        4 => Ok(WebSearchResult),
        5 => Ok(MissingBlock),
        6 => Ok(MissingBlockType),
        7 => Ok(DeltaText),
        8 => Ok(DeltaArguments),
        9 => Ok(DeltaThinking),
        10 => Ok(DeltaSearchInput),
        11 => Ok(MissingDeltaType),
        12 => Ok(MessageDelta),
        13 => Ok(MessageStop),
        14 => Ok(Error),
        15 => Ok(BlockStop),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Absent input stays absent; an unknown finish reason is a normal stop.
pub fn anthropic_stream_finish_reason(
    reason: Option<&str>,
) -> Result<Option<&'static str>, MojoError> {
    let input = reason.unwrap_or("");
    let mut result = -1_i64;
    check_status(unsafe {
        prodex_anthropic_stream_finish_reason_v1(
            1,
            i64::from(reason.is_some()),
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    match result {
        0 => Ok(None),
        1 => Ok(Some("stop")),
        2 => Ok(Some("tool_calls")),
        3 => Ok(Some("length")),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ordered, borrowed web-search facts from the host's BTreeMap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicWebSearchEntry<'a> {
    pub id: &'a str,
    pub done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicWebSearchChoice {
    /// Position into the original ordered entry slice, not a tool output index.
    pub position: usize,
    /// Whether to emit the terminal item. Completed search IDs still match
    /// a repeated result so the host can validate/store that result as before.
    pub emit: bool,
}

/// The compiled planner owns first-match ID precedence and end-of-message
/// unfinished-search selection. Rust retains BTreeMap storage and JSON events.
pub fn anthropic_web_search_completion_plan(
    entries: &[AnthropicWebSearchEntry<'_>],
    target: Option<&str>,
) -> Result<Vec<AnthropicWebSearchChoice>, MojoError> {
    let count = entries.len();
    let count_twice = count.checked_mul(2).ok_or(MojoError::Capacity)?;
    let mut views = Vec::with_capacity(count_twice);
    let mut states = Vec::with_capacity(count);
    for entry in entries {
        views.push(entry.id.as_ptr() as usize as u64);
        views.push(u64::try_from(entry.id.len()).map_err(|_| MojoError::InvalidInput)?);
        states.push(i64::from(entry.done));
    }
    let target_text = target.unwrap_or("");
    let mut output = vec![u64::MAX; count_twice];
    let mut written = -1_i64;
    check_status(unsafe {
        prodex_anthropic_web_search_completion_v1(
            1,
            views.as_ptr() as usize as u64,
            states.as_ptr() as usize as u64,
            i64::try_from(count).map_err(|_| MojoError::InvalidInput)?,
            i64::from(target.is_some()),
            target_text.as_ptr() as usize as u64,
            i64::try_from(target_text.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let selected = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if selected > count || (target.is_some() && selected > 1) {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; count];
    let mut plan = Vec::with_capacity(selected);
    for pair in output[..selected * 2].chunks_exact(2) {
        let position = usize::try_from(pair[0]).map_err(|_| MojoError::InvalidOutput)?;
        let flag = pair[1];
        let entry = entries.get(position).ok_or(MojoError::InvalidOutput)?;
        if seen[position]
            || flag > 1
            || (flag == 1 && entry.done)
            || (target.is_none() && flag == 0)
        {
            return Err(MojoError::InvalidOutput);
        }
        seen[position] = true;
        plan.push(AnthropicWebSearchChoice {
            position,
            emit: flag == 1,
        });
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use AnthropicStreamAction::*;

    fn fact(event_type: &'static str) -> AnthropicStreamFacts<'static> {
        AnthropicStreamFacts {
            event_type: Some(event_type),
            block_present: false,
            block_type: None,
            block_name: None,
            delta_type: None,
            ignored_block: false,
            active_web_search: false,
        }
    }

    #[test]
    fn compiled_anthropic_web_search_selection_matches_first_id_and_pending_index_order() {
        let entries = [
            AnthropicWebSearchEntry {
                id: "a",
                done: true,
            },
            AnthropicWebSearchEntry {
                id: "b",
                done: false,
            },
            AnthropicWebSearchEntry {
                id: "a",
                done: false,
            },
            AnthropicWebSearchEntry {
                id: "東京🔥",
                done: false,
            },
        ];
        assert_eq!(
            anthropic_web_search_completion_plan(&entries, Some("a")),
            Ok(vec![AnthropicWebSearchChoice {
                position: 0,
                emit: false
            }]),
        );
        assert_eq!(
            anthropic_web_search_completion_plan(&entries, Some("b")),
            Ok(vec![AnthropicWebSearchChoice {
                position: 1,
                emit: true
            }]),
        );
        assert_eq!(
            anthropic_web_search_completion_plan(&entries, Some("東京🔥")),
            Ok(vec![AnthropicWebSearchChoice {
                position: 3,
                emit: true
            }]),
        );
        assert_eq!(
            anthropic_web_search_completion_plan(&entries, Some("A")),
            Ok(vec![])
        );
        assert_eq!(
            anthropic_web_search_completion_plan(&entries, None),
            Ok(vec![
                AnthropicWebSearchChoice {
                    position: 1,
                    emit: true
                },
                AnthropicWebSearchChoice {
                    position: 2,
                    emit: true
                },
                AnthropicWebSearchChoice {
                    position: 3,
                    emit: true
                },
            ])
        );
        assert_eq!(anthropic_web_search_completion_plan(&[], None), Ok(vec![]));
        assert_eq!(
            anthropic_web_search_completion_plan(&[], Some("a")),
            Ok(vec![])
        );
    }

    #[test]
    fn compiled_anthropic_web_search_abi_rejects_invalid_flags_without_writes() {
        let id = b"search";
        let views = [id.as_ptr() as usize as u64, 6_u64];
        let invalid = [0xff_u8];
        let invalid_views = [invalid.as_ptr() as usize as u64, 1_u64];
        let valid_state = [0_i64];
        let bad_state = [2_i64];
        let mut output = [71_u64; 2];
        let mut written = 83_i64;
        let ptr = views.as_ptr() as usize as u64;
        let state = valid_state.as_ptr() as usize as u64;
        let out = output.as_mut_ptr() as usize as u64;
        let wrote = (&mut written as *mut i64) as usize as u64;
        let call = |version, views, state, count, present, capacity, result| unsafe {
            prodex_anthropic_web_search_completion_v1(
                version,
                views,
                state,
                count,
                present,
                id.as_ptr() as usize as u64,
                6,
                out,
                capacity,
                result,
            )
        };
        assert_eq!(call(2, ptr, state, 1, 1, 2, wrote), 4);
        assert_eq!(call(1, 0, state, 1, 1, 2, wrote), 1);
        assert_eq!(call(1, ptr, 0, 1, 1, 2, wrote), 1);
        assert_eq!(call(1, ptr, state, -1, 1, 2, wrote), 1);
        assert_eq!(call(1, ptr, state, 1, 2, 2, wrote), 1);
        assert_eq!(call(1, ptr, state, 1, 1, 1, wrote), 1);
        assert_eq!(call(1, ptr, state, 1, 1, 2, 0), 1);
        assert_eq!(
            call(1, ptr, bad_state.as_ptr() as usize as u64, 1, 1, 2, wrote),
            1
        );
        assert_eq!(
            call(
                1,
                invalid_views.as_ptr() as usize as u64,
                state,
                1,
                1,
                2,
                wrote
            ),
            1
        );
        assert_eq!(output, [71; 2]);
        assert_eq!(written, 83);
    }

    #[test]
    fn compiled_anthropic_sse_dispatch_preserves_block_and_delta_precedence() {
        assert_eq!(
            anthropic_stream_action(fact("message_start")),
            Ok(MessageStart)
        );
        assert_eq!(
            anthropic_stream_action(fact("message_delta")),
            Ok(MessageDelta)
        );
        assert_eq!(
            anthropic_stream_action(fact("message_stop")),
            Ok(MessageStop)
        );
        assert_eq!(anthropic_stream_action(fact("error")), Ok(Error));
        assert_eq!(
            anthropic_stream_action(fact("content_block_stop")),
            Ok(BlockStop)
        );
        assert_eq!(anthropic_stream_action(fact("ping")), Ok(Ignore));
        assert_eq!(anthropic_stream_action(fact("🚀")), Ok(Ignore));
        assert_eq!(
            anthropic_stream_action(fact("content_block_start")),
            Ok(MissingBlock)
        );
        let block = AnthropicStreamFacts {
            block_present: true,
            ..fact("content_block_start")
        };
        assert_eq!(anthropic_stream_action(block), Ok(MissingBlockType));
        for (block_type, name, expected) in [
            ("text", None, Ignore),
            ("thinking", None, Ignore),
            ("tool_use", None, StartToolUse),
            ("server_tool_use", Some("web_search"), StartWebSearch),
            ("server_tool_use", Some("Web_Search"), Ignore),
            ("web_search_tool_result", None, WebSearchResult),
            ("nonstandard", None, Ignore),
        ] {
            assert_eq!(
                anthropic_stream_action(AnthropicStreamFacts {
                    block_type: Some(block_type),
                    block_name: name,
                    ..block
                }),
                Ok(expected),
                "block={block_type} name={name:?}"
            );
        }
        let delta = fact("content_block_delta");
        assert_eq!(anthropic_stream_action(delta), Ok(MissingDeltaType));
        for (delta_type, expected) in [
            ("text_delta", DeltaText),
            ("input_json_delta", DeltaArguments),
            ("thinking_delta", DeltaThinking),
            ("signature_delta", Ignore),
            ("unknown", Ignore),
        ] {
            assert_eq!(
                anthropic_stream_action(AnthropicStreamFacts {
                    delta_type: Some(delta_type),
                    ..delta
                }),
                Ok(expected),
                "delta={delta_type}"
            );
        }
        assert_eq!(
            anthropic_stream_action(AnthropicStreamFacts {
                delta_type: Some("input_json_delta"),
                active_web_search: true,
                ..delta
            }),
            Ok(DeltaSearchInput)
        );
        assert_eq!(
            anthropic_stream_action(AnthropicStreamFacts {
                delta_type: Some("text_delta"),
                active_web_search: true,
                ..delta
            }),
            Ok(Ignore)
        );
        assert_eq!(
            anthropic_stream_action(AnthropicStreamFacts {
                ignored_block: true,
                delta_type: None,
                ..delta
            }),
            Ok(Ignore)
        );
        assert_eq!(
            anthropic_stream_action(AnthropicStreamFacts {
                ignored_block: true,
                active_web_search: true,
                delta_type: Some("input_json_delta"),
                ..delta
            }),
            Ok(Ignore)
        );
    }

    #[test]
    fn compiled_anthropic_sse_finish_reason_preserves_unknown_and_absent() {
        for (reason, expected) in [
            (None, None),
            (Some("tool_use"), Some("tool_calls")),
            (Some("max_tokens"), Some("length")),
            (Some("end_turn"), Some("stop")),
            (Some(""), Some("stop")),
            (Some("TOOL_USE"), Some("stop")),
            (Some("😺"), Some("stop")),
        ] {
            assert_eq!(anthropic_stream_finish_reason(reason), Ok(expected));
        }
    }

    #[test]
    fn compiled_anthropic_sse_policy_rejects_invalid_abi_without_mutating_output() {
        let text = b"message_start";
        let ptr = text.as_ptr() as usize as u64;
        let mut result = 91_i64;
        let out = (&mut result as *mut i64) as usize as u64;
        let action = |version, event_ptr, length, ignored, dest| unsafe {
            prodex_anthropic_stream_action_v1(
                version, event_ptr, length, 0, 0, 0, 0, 0, 0, 0, 0, 0, ignored, 0, dest,
            )
        };
        assert_eq!(action(2, ptr, 13, 0, out), 4);
        assert_eq!(action(1, 0, 13, 0, out), 1);
        assert_eq!(action(1, ptr, -1, 0, out), 1);
        assert_eq!(action(1, ptr, 13, 2, out), 1);
        assert_eq!(action(1, ptr, 13, 0, 0), 1);
        let bad = [0xff_u8];
        assert_eq!(action(1, bad.as_ptr() as usize as u64, 1, 0, out), 1);
        assert_eq!(result, 91);
        let finish = |version, present, input_ptr, length, dest| unsafe {
            prodex_anthropic_stream_finish_reason_v1(version, present, input_ptr, length, dest)
        };
        assert_eq!(finish(2, 1, ptr, 13, out), 4);
        assert_eq!(finish(1, 2, ptr, 13, out), 1);
        assert_eq!(finish(1, 1, 0, 13, out), 1);
        assert_eq!(finish(1, 1, ptr, -1, out), 1);
        assert_eq!(finish(1, 1, ptr, 13, 0), 1);
        assert_eq!(finish(1, 1, bad.as_ptr() as usize as u64, 1, out), 1);
        assert_eq!(result, 91);
    }
}
