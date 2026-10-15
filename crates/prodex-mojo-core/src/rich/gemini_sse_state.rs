use crate::MojoError;

pub const GEMINI_RESPONSE_STATE_ABI_VERSION: i64 = 1;
const GEMINI_RESPONSE_PART_ACTIONS: u8 = 255;
pub const GEMINI_RESPONSE_PART_BATCH_MAX: usize = 65_536;
const GEMINI_RESPONSE_PART_BATCH_TAIL_GROUNDING: u8 = 1;
const GEMINI_RESPONSE_PART_BATCH_TAIL_CITATIONS: u8 = 2;

const GEMINI_RESPONSE_PART_FLAG_HAS_TEXT: i64 = 1;
const GEMINI_RESPONSE_PART_FLAG_IS_THOUGHT: i64 = 2;
const GEMINI_RESPONSE_PART_FLAG_HAS_VISIBLE_TEXT: i64 = 4;
const GEMINI_RESPONSE_PART_FLAG_HAS_SPECIAL_TEXT: i64 = 8;
const GEMINI_RESPONSE_PART_FLAG_HAS_MEDIA: i64 = 16;
const GEMINI_RESPONSE_PART_FLAG_HAS_VIDEO_METADATA: i64 = 32;
const GEMINI_RESPONSE_PART_FLAG_HAS_IMAGE_GENERATION: i64 = 64;
const GEMINI_RESPONSE_PART_FLAG_HAS_FUNCTION_CALL: i64 = 128;
const GEMINI_RESPONSE_PART_FLAG_COMMAND_OUTPUT_ONLY: i64 = 256;
const GEMINI_RESPONSE_PART_FLAG_FORCED_OUTPUT: i64 = 512;
const GEMINI_RESPONSE_PART_FLAG_INTERNAL_ECHO: i64 = 1024;
const GEMINI_RESPONSE_PART_FLAG_SUPPRESS_VISIBLE_TEXT: i64 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiResponsePartInput {
    pub has_text: bool,
    pub is_thought: bool,
    pub has_visible_text: bool,
    pub has_special_text: bool,
    pub has_media: bool,
    pub has_video_metadata: bool,
    pub has_image_generation: bool,
    pub has_function_call: bool,
    pub command_output_only: bool,
    pub forced_output: bool,
    pub internal_instruction_echo: bool,
    pub suppress_visible_text: bool,
}

unsafe extern "C" {
    fn prodex_mojo_rich_gemini_once_event_plan_v1(
        abi_version: i64,
        already_emitted: i64,
        content_present: i64,
        requires_content: i64,
        output_emit: *mut i64,
    ) -> i64;
    fn prodex_mojo_rich_gemini_completion_guardrail_plan_v1(
        abi_version: i64,
        output_is_empty: i64,
        reasoning_is_empty: i64,
        tool_calls_empty: i64,
        tool_intent_present: i64,
        wait_or_poll_present: i64,
        unverified_success: i64,
        output_action: *mut i64,
    ) -> i64;
    fn prodex_mojo_rich_gemini_response_part_plan_v1(
        abi_version: i64,
        has_text: i64,
        is_thought: i64,
        has_visible_text: i64,
        has_special_text: i64,
        has_media: i64,
        has_video_metadata: i64,
        has_image_generation: i64,
        has_function_call: i64,
        command_output_only: i64,
        forced_output: i64,
        internal_instruction_echo: i64,
        suppress_visible_text: i64,
        output_actions: *mut i64,
    ) -> i64;
    fn prodex_mojo_rich_gemini_response_parts_plan_v1(
        abi_version: i64,
        inputs: u64,
        count: i64,
        actions: u64,
        function_flags: u64,
        grounding_present: i64,
        citations_present: i64,
        tail_actions: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCompletionGuardrailAction {
    None,
    EmptyResponse,
    ToolIntentWithoutCall,
    NonActionableWait,
    UnverifiedSuccess,
}

pub fn plan_gemini_once_event(
    already_emitted: bool,
    content_present: bool,
    requires_content: bool,
) -> Result<bool, MojoError> {
    super::ensure_rich_abi()?;
    let mut emit = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_gemini_once_event_plan_v1(
            GEMINI_RESPONSE_STATE_ABI_VERSION,
            i64::from(already_emitted),
            i64::from(content_present),
            i64::from(requires_content),
            &mut emit,
        )
    };
    match (status, emit) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (1, _) => Err(MojoError::InvalidInput),
        (4, _) => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn plan_gemini_completion_guardrail(
    output_is_empty: bool,
    reasoning_is_empty: bool,
    tool_calls_empty: bool,
    tool_intent_present: bool,
    wait_or_poll_present: bool,
    unverified_success: bool,
) -> Result<GeminiCompletionGuardrailAction, MojoError> {
    super::ensure_rich_abi()?;
    let mut action = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_gemini_completion_guardrail_plan_v1(
            GEMINI_RESPONSE_STATE_ABI_VERSION,
            i64::from(output_is_empty),
            i64::from(reasoning_is_empty),
            i64::from(tool_calls_empty),
            i64::from(tool_intent_present),
            i64::from(wait_or_poll_present),
            i64::from(unverified_success),
            &mut action,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match action {
        0 => Ok(GeminiCompletionGuardrailAction::None),
        1 => Ok(GeminiCompletionGuardrailAction::EmptyResponse),
        2 => Ok(GeminiCompletionGuardrailAction::ToolIntentWithoutCall),
        3 => Ok(GeminiCompletionGuardrailAction::NonActionableWait),
        4 => Ok(GeminiCompletionGuardrailAction::UnverifiedSuccess),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_gemini_response_part(input: GeminiResponsePartInput) -> Result<u8, MojoError> {
    super::ensure_rich_abi()?;
    let mut actions = 0_i64;
    let status = unsafe {
        prodex_mojo_rich_gemini_response_part_plan_v1(
            GEMINI_RESPONSE_STATE_ABI_VERSION,
            i64::from(input.has_text),
            i64::from(input.is_thought),
            i64::from(input.has_visible_text),
            i64::from(input.has_special_text),
            i64::from(input.has_media),
            i64::from(input.has_video_metadata),
            i64::from(input.has_image_generation),
            i64::from(input.has_function_call),
            i64::from(input.command_output_only),
            i64::from(input.forced_output),
            i64::from(input.internal_instruction_echo),
            i64::from(input.suppress_visible_text),
            &mut actions,
        )
    };
    match status {
        0 if (0..=i64::from(GEMINI_RESPONSE_PART_ACTIONS)).contains(&actions) => Ok(actions as u8),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// The ordered actions for a complete buffered Gemini response-part batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiResponsePartsPlan {
    pub actions: Vec<u8>,
    pub function_call_flags: String,
    pub append_grounding: bool,
    pub append_citations: bool,
}

fn gemini_response_part_flags(input: GeminiResponsePartInput) -> i64 {
    let mut flags = 0_i64;
    for (present, flag) in [
        (input.has_text, GEMINI_RESPONSE_PART_FLAG_HAS_TEXT),
        (input.is_thought, GEMINI_RESPONSE_PART_FLAG_IS_THOUGHT),
        (
            input.has_visible_text,
            GEMINI_RESPONSE_PART_FLAG_HAS_VISIBLE_TEXT,
        ),
        (
            input.has_special_text,
            GEMINI_RESPONSE_PART_FLAG_HAS_SPECIAL_TEXT,
        ),
        (input.has_media, GEMINI_RESPONSE_PART_FLAG_HAS_MEDIA),
        (
            input.has_video_metadata,
            GEMINI_RESPONSE_PART_FLAG_HAS_VIDEO_METADATA,
        ),
        (
            input.has_image_generation,
            GEMINI_RESPONSE_PART_FLAG_HAS_IMAGE_GENERATION,
        ),
        (
            input.has_function_call,
            GEMINI_RESPONSE_PART_FLAG_HAS_FUNCTION_CALL,
        ),
        (
            input.command_output_only,
            GEMINI_RESPONSE_PART_FLAG_COMMAND_OUTPUT_ONLY,
        ),
        (input.forced_output, GEMINI_RESPONSE_PART_FLAG_FORCED_OUTPUT),
        (
            input.internal_instruction_echo,
            GEMINI_RESPONSE_PART_FLAG_INTERNAL_ECHO,
        ),
        (
            input.suppress_visible_text,
            GEMINI_RESPONSE_PART_FLAG_SUPPRESS_VISIBLE_TEXT,
        ),
    ] {
        if present {
            flags |= flag;
        }
    }
    flags
}

/// Plans all response-part actions and the fixed grounding/citation tail in one Mojo call.
pub fn plan_gemini_response_parts(
    inputs: &[GeminiResponsePartInput],
    grounding_present: bool,
    citations_present: bool,
) -> Result<GeminiResponsePartsPlan, MojoError> {
    super::ensure_rich_abi()?;
    if inputs.len() > GEMINI_RESPONSE_PART_BATCH_MAX {
        return Err(MojoError::InvalidInput);
    }
    let packed = inputs
        .iter()
        .copied()
        .map(gemini_response_part_flags)
        .collect::<Vec<_>>();
    let mut actions = vec![0_i64; inputs.len()];
    let mut function_flags = vec![0_u8; inputs.len()];
    let mut tail_actions = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_gemini_response_parts_plan_v1(
            GEMINI_RESPONSE_STATE_ABI_VERSION,
            if packed.is_empty() {
                0
            } else {
                super::mojo_pointer_address(packed.as_ptr())
            },
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            if actions.is_empty() {
                0
            } else {
                super::mojo_mut_pointer_address(actions.as_mut_ptr())
            },
            if function_flags.is_empty() {
                0
            } else {
                super::mojo_mut_pointer_address(function_flags.as_mut_ptr())
            },
            i64::from(grounding_present),
            i64::from(citations_present),
            super::mojo_mut_pointer_address(&mut tail_actions),
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let actions = actions
        .into_iter()
        .map(|action| {
            if (0..=i64::from(GEMINI_RESPONSE_PART_ACTIONS)).contains(&action) {
                Ok(action as u8)
            } else {
                Err(MojoError::InvalidOutput)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if function_flags
        .iter()
        .any(|flag| *flag != b'0' && *flag != b'1')
    {
        return Err(MojoError::InvalidOutput);
    }
    let tail_actions = u8::try_from(tail_actions).map_err(|_| MojoError::InvalidOutput)?;
    if tail_actions
        & !(GEMINI_RESPONSE_PART_BATCH_TAIL_GROUNDING | GEMINI_RESPONSE_PART_BATCH_TAIL_CITATIONS)
        != 0
        || (tail_actions & GEMINI_RESPONSE_PART_BATCH_TAIL_GROUNDING != 0) != grounding_present
        || (tail_actions & GEMINI_RESPONSE_PART_BATCH_TAIL_CITATIONS != 0) != citations_present
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(GeminiResponsePartsPlan {
        actions,
        function_call_flags: String::from_utf8(function_flags)
            .map_err(|_| MojoError::InvalidOutput)?,
        append_grounding: tail_actions & GEMINI_RESPONSE_PART_BATCH_TAIL_GROUNDING != 0,
        append_citations: tail_actions & GEMINI_RESPONSE_PART_BATCH_TAIL_CITATIONS != 0,
    })
}
