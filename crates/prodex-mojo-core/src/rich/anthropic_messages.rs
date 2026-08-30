use super::*;

const ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS: usize = 65_536;

/// A validated Anthropic Messages response content block classified by Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum AnthropicResponseBlockKind {
    Text = 0,
    ToolUse = 1,
    WebSearchCall = 2,
    WebSearchResult = 3,
    Thinking = 4,
}

/// Input record for the bounded Anthropic response-content planner.
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

unsafe extern "C" {
    fn prodex_mojo_rich_anthropic_response_plan_v1(
        abi_version: i64,
        input_kinds: u64,
        input_has_text: u64,
        output_kinds: u64,
        output_starts: u64,
        output_counts: u64,
        output_indices: u64,
        output_capacity: i64,
        output_count: u64,
        input_count: i64,
    ) -> i64;
}

/// Plans normalized response content without crossing the JSON boundary.
pub fn plan_anthropic_response_blocks(
    input: &[AnthropicResponseBlock],
) -> Result<Vec<AnthropicResponsePlanItem>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > ANTHROPIC_RESPONSE_PLAN_MAX_BLOCKS {
        return Err(MojoError::InvalidInput);
    }

    let kinds = input
        .iter()
        .map(|block| block.kind as i64)
        .collect::<Vec<_>>();
    let has_text = input
        .iter()
        .map(|block| i64::from(block.has_text))
        .collect::<Vec<_>>();
    let capacity = input.len();
    let mut output_kinds = vec![0_i64; capacity];
    let mut output_starts = vec![0_i64; capacity];
    let mut output_counts = vec![0_i64; capacity];
    let mut output_indices = vec![0_i64; capacity];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_mojo_rich_anthropic_response_plan_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(kinds.as_ptr()),
            mojo_pointer_address(has_text.as_ptr()),
            mojo_mut_pointer_address(output_kinds.as_mut_ptr()),
            mojo_mut_pointer_address(output_starts.as_mut_ptr()),
            mojo_mut_pointer_address(output_counts.as_mut_ptr()),
            mojo_mut_pointer_address(output_indices.as_mut_ptr()),
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output_count),
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(status_error(status, 3, 0, 0, 0));
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > capacity {
        return Err(MojoError::InvalidOutput);
    }

    let mut covered = vec![false; input.len()];
    let mut last_input = 0;
    let mut plan = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let kind = AnthropicResponsePlanKind::try_from(output_kinds[index])?;
        let start = usize::try_from(output_starts[index]).map_err(|_| MojoError::InvalidOutput)?;
        let count = usize::try_from(output_counts[index]).map_err(|_| MojoError::InvalidOutput)?;
        let input_index =
            usize::try_from(output_indices[index]).map_err(|_| MojoError::InvalidOutput)?;
        validate_anthropic_plan_item(
            input,
            &mut covered,
            &mut last_input,
            kind,
            start,
            count,
            input_index,
        )?;
        plan.push(AnthropicResponsePlanItem {
            kind,
            start,
            count,
            input_index,
        });
    }
    validate_anthropic_plan_coverage(input, &covered)?;
    Ok(plan)
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
