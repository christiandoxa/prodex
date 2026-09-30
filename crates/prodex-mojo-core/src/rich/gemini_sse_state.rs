use crate::MojoError;

pub const GEMINI_RESPONSE_STATE_ABI_VERSION: i64 = 1;
const GEMINI_RESPONSE_PART_ACTIONS: u8 = 255;

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
