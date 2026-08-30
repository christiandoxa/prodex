use std::fmt;

const ACTION_REASONING: u8 = 1;
const ACTION_VISIBLE_TEXT: u8 = 2;
const ACTION_SPECIAL_TEXT: u8 = 4;
const ACTION_MEDIA: u8 = 8;
const ACTION_NATIVE: u8 = 16;
const ACTION_IMAGE: u8 = 32;
const ACTION_FUNCTION: u8 = 64;
const ACTION_FLUSH_PENDING: u8 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeminiProviderCoreResponsePartInput {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeminiProviderCoreResponsePartPlan {
    pub emit_reasoning: bool,
    pub emit_visible_text: bool,
    pub emit_special_text: bool,
    pub record_media: bool,
    pub record_native: bool,
    pub record_image: bool,
    pub emit_function: bool,
    pub flush_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeminiProviderCoreResponsePartPlanError {
    InvalidInput,
    InvalidOutput,
    AbiMismatch,
}

impl fmt::Display for GeminiProviderCoreResponsePartPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid Gemini response part input",
            Self::InvalidOutput => "invalid Gemini response part plan output",
            Self::AbiMismatch => "Gemini response part planner ABI mismatch",
        })
    }
}

impl std::error::Error for GeminiProviderCoreResponsePartPlanError {}

pub fn gemini_provider_core_response_part_plan(
    input: GeminiProviderCoreResponsePartInput,
) -> Result<GeminiProviderCoreResponsePartPlan, GeminiProviderCoreResponsePartPlanError> {
    #[cfg(feature = "mojo")]
    let actions = {
        let input = prodex_mojo_core::rich::GeminiResponsePartInput {
            has_text: input.has_text,
            is_thought: input.is_thought,
            has_visible_text: input.has_visible_text,
            has_special_text: input.has_special_text,
            has_media: input.has_media,
            has_video_metadata: input.has_video_metadata,
            has_image_generation: input.has_image_generation,
            has_function_call: input.has_function_call,
            command_output_only: input.command_output_only,
            forced_output: input.forced_output,
            internal_instruction_echo: input.internal_instruction_echo,
            suppress_visible_text: input.suppress_visible_text,
        };
        prodex_mojo_core::rich::plan_gemini_response_part(input).map_err(|error| match error {
            prodex_mojo_core::MojoError::InvalidInput => {
                GeminiProviderCoreResponsePartPlanError::InvalidInput
            }
            prodex_mojo_core::MojoError::AbiMismatch => {
                GeminiProviderCoreResponsePartPlanError::AbiMismatch
            }
            prodex_mojo_core::MojoError::InvalidOutput
            | prodex_mojo_core::MojoError::Capacity
            | prodex_mojo_core::MojoError::Structured(_) => {
                GeminiProviderCoreResponsePartPlanError::InvalidOutput
            }
        })?
    };

    #[cfg(not(feature = "mojo"))]
    let actions = gemini_provider_core_response_part_plan_rust(input);

    Ok(GeminiProviderCoreResponsePartPlan {
        emit_reasoning: actions & ACTION_REASONING != 0,
        emit_visible_text: actions & ACTION_VISIBLE_TEXT != 0,
        emit_special_text: actions & ACTION_SPECIAL_TEXT != 0,
        record_media: actions & ACTION_MEDIA != 0,
        record_native: actions & ACTION_NATIVE != 0,
        record_image: actions & ACTION_IMAGE != 0,
        emit_function: actions & ACTION_FUNCTION != 0,
        flush_pending: actions & ACTION_FLUSH_PENDING != 0,
    })
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn gemini_provider_core_response_part_plan_rust(
    input: GeminiProviderCoreResponsePartInput,
) -> u8 {
    let mut actions = 0;
    if input.has_text && input.is_thought {
        actions |= ACTION_REASONING;
    } else if input.has_visible_text
        && !input.command_output_only
        && !input.forced_output
        && !input.internal_instruction_echo
        && !input.suppress_visible_text
    {
        actions |= ACTION_VISIBLE_TEXT;
    }
    if input.has_special_text && !input.command_output_only && !input.forced_output {
        actions |= ACTION_SPECIAL_TEXT;
    }
    if input.has_media {
        actions |= ACTION_MEDIA | ACTION_NATIVE;
    }
    if input.has_video_metadata {
        actions |= ACTION_NATIVE;
    }
    if input.has_image_generation {
        actions |= ACTION_IMAGE;
    }
    if input.has_function_call && !input.forced_output {
        actions |= ACTION_FUNCTION | ACTION_FLUSH_PENDING;
    }
    actions
}

#[cfg(test)]
#[path = "response_state_tests.rs"]
mod tests;
