use super::*;

const ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS: usize = 65_536;

/// Unclassified fields acquired from one Anthropic response content block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicResponseBlockClassificationInput<'a> {
    pub type_name: Option<&'a str>,
    pub has_text_field: bool,
    pub has_thinking_field: bool,
    pub has_id_field: bool,
    pub has_name_field: bool,
    pub name_is_web_search: bool,
}

/// A provider-level rejection reported by Anthropic response classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicResponseBlockClassificationError {
    MissingType { index: usize },
    UnsupportedType { index: usize },
    MissingText { index: usize },
    MissingToolUseId { index: usize },
    MissingToolUseName { index: usize },
    MissingServerToolId { index: usize },
    UnsupportedServerTool { index: usize },
}

/// A validated Anthropic Messages response content block classified by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum AnthropicResponseBlockKind {
    Text = 0,
    ToolUse = 1,
    WebSearchCall = 2,
    WebSearchResult = 3,
    Thinking = 4,
}

/// Classified record for the bounded Anthropic response-content planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicResponseBlock {
    pub kind: AnthropicResponseBlockKind,
    pub has_text: bool,
}

/// An ordered output item planned from one or more response content blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum AnthropicResponsePlanKind {
    Message = 0,
    ToolUse = 1,
    WebSearchCall = 2,
    WebSearchResult = 3,
    Reasoning = 4,
}

/// A plan item contains either a text range or one source block index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicResponsePlanItem {
    pub kind: AnthropicResponsePlanKind,
    pub start: usize,
    pub count: usize,
    pub input_index: usize,
}

/// Classified blocks and their ordered Responses output plan.
///
/// If `issue` is set, `blocks` contains only the valid prefix before that issue
/// and `items` is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicResponsePlan {
    pub blocks: Vec<AnthropicResponseBlock>,
    pub items: Vec<AnthropicResponsePlanItem>,
    pub issue: Option<AnthropicResponseBlockClassificationError>,
}

impl TryFrom<i64> for AnthropicResponsePlanKind {
    type Error = MojoError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Message,
            1 => Self::ToolUse,
            2 => Self::WebSearchCall,
            3 => Self::WebSearchResult,
            4 => Self::Reasoning,
            _ => return Err(MojoError::InvalidOutput),
        })
    }
}

const ANTHROPIC_RESPONSE_PLAN_ABI_VERSION: i64 = 7;

unsafe extern "C" {
    fn prodex_mojo_rich_anthropic_response_plan_v2(
        abi_version: i64,
        input_types: u64,
        input_flags: u64,
        output_block_kinds: u64,
        output_block_has_text: u64,
        output_kinds: u64,
        output_starts: u64,
        output_counts: u64,
        output_indices: u64,
        output_capacity: i64,
        output_count: u64,
        input_count: i64,
        issue_index: u64,
    ) -> i64;
}

/// Classifies Anthropic response blocks and plans their normalized output in Mojo.
///
/// On a provider-level classification issue, the result carries the valid
/// prefix so callers can preserve per-block materialization error order.
pub fn plan_anthropic_response_blocks(
    input: &[AnthropicResponseBlockClassificationInput<'_>],
) -> Result<AnthropicResponsePlan, MojoError> {
    ensure_rich_abi()?;
    if input.len() > ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS {
        return Err(MojoError::InvalidInput);
    }
    if input.is_empty() {
        return Ok(AnthropicResponsePlan {
            blocks: Vec::new(),
            items: Vec::new(),
            issue: None,
        });
    }

    let types = input
        .iter()
        .map(|block| block.type_name.map(super::view).unwrap_or_default())
        .collect::<Vec<_>>();
    let flags = input
        .iter()
        .map(|block| {
            i64::from(block.type_name.is_some())
                + 2 * i64::from(block.has_text_field)
                + 4 * i64::from(block.has_thinking_field)
                + 8 * i64::from(block.has_id_field)
                + 16 * i64::from(block.has_name_field)
                + 32 * i64::from(block.name_is_web_search)
        })
        .collect::<Vec<_>>();
    let capacity = i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output_block_kinds = vec![0_i64; input.len()];
    let mut output_block_has_text = vec![0_i64; input.len()];
    let mut output_kinds = vec![0_i64; input.len()];
    let mut output_starts = vec![0_i64; input.len()];
    let mut output_counts = vec![0_i64; input.len()];
    let mut output_indices = vec![0_i64; input.len()];
    let mut output_count = 0_i64;
    let mut issue_index = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_anthropic_response_plan_v2(
            ANTHROPIC_RESPONSE_PLAN_ABI_VERSION,
            mojo_pointer_address(types.as_ptr()),
            mojo_pointer_address(flags.as_ptr()),
            mojo_mut_pointer_address(output_block_kinds.as_mut_ptr()),
            mojo_mut_pointer_address(output_block_has_text.as_mut_ptr()),
            mojo_mut_pointer_address(output_kinds.as_mut_ptr()),
            mojo_mut_pointer_address(output_starts.as_mut_ptr()),
            mojo_mut_pointer_address(output_counts.as_mut_ptr()),
            mojo_mut_pointer_address(output_indices.as_mut_ptr()),
            capacity,
            mojo_mut_pointer_address(&mut output_count),
            capacity,
            mojo_mut_pointer_address(&mut issue_index),
        )
    };
    let issue_kind = match status {
        5 => Some(0),
        6 => Some(1),
        7 => Some(2),
        8 => Some(3),
        9 => Some(4),
        10 => Some(5),
        11 => Some(6),
        _ if status != 0 => return Err(status_error(status, 3, 0, 0, 0)),
        _ => None,
    };
    let (issue, classified_count) = if let Some(issue_kind) = issue_kind {
        let index = usize::try_from(issue_index).map_err(|_| MojoError::InvalidOutput)?;
        if index >= input.len() {
            return Err(MojoError::InvalidOutput);
        }
        let issue = match issue_kind {
            0 => AnthropicResponseBlockClassificationError::MissingType { index },
            1 => AnthropicResponseBlockClassificationError::UnsupportedType { index },
            2 => AnthropicResponseBlockClassificationError::MissingText { index },
            3 => AnthropicResponseBlockClassificationError::MissingToolUseId { index },
            4 => AnthropicResponseBlockClassificationError::MissingToolUseName { index },
            5 => AnthropicResponseBlockClassificationError::MissingServerToolId { index },
            _ => AnthropicResponseBlockClassificationError::UnsupportedServerTool { index },
        };
        (Some(issue), index)
    } else {
        (None, input.len())
    };
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if (issue.is_some() && output_count != 0) || (issue.is_none() && issue_index != -1) {
        return Err(MojoError::InvalidOutput);
    }

    let blocks = output_block_kinds
        .into_iter()
        .take(classified_count)
        .zip(output_block_has_text)
        .map(|(kind, has_text)| {
            let kind = match kind {
                0 => AnthropicResponseBlockKind::Text,
                1 => AnthropicResponseBlockKind::ToolUse,
                2 => AnthropicResponseBlockKind::WebSearchCall,
                3 => AnthropicResponseBlockKind::WebSearchResult,
                4 => AnthropicResponseBlockKind::Thinking,
                _ => return Err(MojoError::InvalidOutput),
            };
            let has_text = match has_text {
                0 => false,
                1 => true,
                _ => return Err(MojoError::InvalidOutput),
            };
            if (kind == AnthropicResponseBlockKind::Text && !has_text)
                || (matches!(
                    kind,
                    AnthropicResponseBlockKind::ToolUse
                        | AnthropicResponseBlockKind::WebSearchCall
                        | AnthropicResponseBlockKind::WebSearchResult
                ) && has_text)
            {
                return Err(MojoError::InvalidOutput);
            }
            Ok(AnthropicResponseBlock { kind, has_text })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(issue) = issue {
        return Ok(AnthropicResponsePlan {
            blocks,
            items: Vec::new(),
            issue: Some(issue),
        });
    }
    if output_count > input.len() {
        return Err(MojoError::InvalidOutput);
    }

    let mut covered = vec![false; input.len()];
    let mut last_input = 0;
    let mut items = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let kind = AnthropicResponsePlanKind::try_from(output_kinds[index])?;
        let start = usize::try_from(output_starts[index]).map_err(|_| MojoError::InvalidOutput)?;
        let count = usize::try_from(output_counts[index]).map_err(|_| MojoError::InvalidOutput)?;
        let input_index =
            usize::try_from(output_indices[index]).map_err(|_| MojoError::InvalidOutput)?;
        validate_anthropic_plan_item(
            &blocks,
            &mut covered,
            &mut last_input,
            kind,
            start,
            count,
            input_index,
        )?;
        items.push(AnthropicResponsePlanItem {
            kind,
            start,
            count,
            input_index,
        });
    }
    validate_anthropic_plan_coverage(&blocks, &covered)?;
    Ok(AnthropicResponsePlan {
        blocks,
        items,
        issue: None,
    })
}

fn validate_anthropic_plan_item(
    input: &[AnthropicResponseBlock],
    covered: &mut [bool],
    last_input: &mut usize,
    kind: AnthropicResponsePlanKind,
    start: usize,
    count: usize,
    input_index: usize,
) -> Result<(), MojoError> {
    match kind {
        AnthropicResponsePlanKind::Message => {
            validate_anthropic_text_message(input, covered, last_input, start, count, input_index)
        }
        AnthropicResponsePlanKind::ToolUse
        | AnthropicResponsePlanKind::WebSearchCall
        | AnthropicResponsePlanKind::WebSearchResult
        | AnthropicResponsePlanKind::Reasoning => validate_anthropic_structured_item(
            input,
            covered,
            last_input,
            kind,
            start,
            count,
            input_index,
        ),
    }
}

fn validate_anthropic_text_message(
    input: &[AnthropicResponseBlock],
    covered: &mut [bool],
    last_input: &mut usize,
    start: usize,
    count: usize,
    input_index: usize,
) -> Result<(), MojoError> {
    let end = start.checked_add(count).ok_or(MojoError::InvalidOutput)?;
    if count == 0 || start < *last_input || end > input.len() || input_index != 0 {
        return Err(MojoError::InvalidOutput);
    }
    for block_index in start..end {
        let block = input[block_index];
        if covered[block_index] || block.kind != AnthropicResponseBlockKind::Text || !block.has_text
        {
            return Err(MojoError::InvalidOutput);
        }
        covered[block_index] = true;
    }
    *last_input = end;
    Ok(())
}

fn validate_anthropic_structured_item(
    input: &[AnthropicResponseBlock],
    covered: &mut [bool],
    last_input: &mut usize,
    kind: AnthropicResponsePlanKind,
    start: usize,
    count: usize,
    input_index: usize,
) -> Result<(), MojoError> {
    if start != 0 || count != 0 || input_index < *last_input || input_index >= input.len() {
        return Err(MojoError::InvalidOutput);
    }
    let input_block = input[input_index];
    let expected = match kind {
        AnthropicResponsePlanKind::ToolUse => {
            input_block.kind == AnthropicResponseBlockKind::ToolUse
        }
        AnthropicResponsePlanKind::WebSearchCall => {
            input_block.kind == AnthropicResponseBlockKind::WebSearchCall
        }
        AnthropicResponsePlanKind::WebSearchResult => {
            input_block.kind == AnthropicResponseBlockKind::WebSearchResult
        }
        AnthropicResponsePlanKind::Reasoning => {
            input_block.kind == AnthropicResponseBlockKind::Thinking && input_block.has_text
        }
        AnthropicResponsePlanKind::Message => false,
    };
    if !expected || covered[input_index] {
        return Err(MojoError::InvalidOutput);
    }
    covered[input_index] = true;
    *last_input = input_index.saturating_add(1);
    Ok(())
}

fn validate_anthropic_plan_coverage(
    input: &[AnthropicResponseBlock],
    covered: &[bool],
) -> Result<(), MojoError> {
    for (index, block) in input.iter().enumerate() {
        if !covered[index]
            && !(block.kind == AnthropicResponseBlockKind::Thinking && !block.has_text)
        {
            return Err(MojoError::InvalidOutput);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_plan_abi_rejects_stale_version_and_bad_bounds() {
        let mut output_count = 99_i64;
        let mut issue_index = 99_i64;
        {
            let mut call = |version, capacity, count| unsafe {
                prodex_mojo_rich_anthropic_response_plan_v2(
                    version,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    capacity,
                    mojo_mut_pointer_address(&mut output_count),
                    count,
                    mojo_mut_pointer_address(&mut issue_index),
                )
            };
            assert_eq!(call(ANTHROPIC_RESPONSE_PLAN_ABI_VERSION - 1, 0, 0), 4);
            assert_eq!(call(ANTHROPIC_RESPONSE_PLAN_ABI_VERSION, 0, -1), 1);
            assert_eq!(call(ANTHROPIC_RESPONSE_PLAN_ABI_VERSION, 0, 1), 3);
            assert_eq!(call(ANTHROPIC_RESPONSE_PLAN_ABI_VERSION, 0, 0), 0);
        }
        assert_eq!(output_count, 0);
        assert_eq!(issue_index, -1);
    }
}
