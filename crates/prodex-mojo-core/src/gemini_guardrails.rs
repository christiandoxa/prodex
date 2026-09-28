use crate::MojoError;

const GEMINI_GUARDRAIL_ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeminiGuardrailTextOperation {
    WaitReason = 1,
    ToolIntent = 2,
    SuccessClaim = 3,
    ToolFailure = 4,
    VersionLines = 5,
    VerificationMarker = 6,
    ProcessExitedZero = 7,
    CommandOutputOnly = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiWaitOrPollReason {
    IWillPoll,
    IllPoll,
    INeedToWait,
    LetsWait,
    StillRunning,
    IsStillRunning,
    IWillWait,
    IllWait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiToolIntent {
    ExecCommand,
    WriteStdin,
    ApplyPatch,
    SqzGrep,
    SqzReadFile,
    SqzListDir,
    ReadMcpResource,
    ListMcpResources,
    ToolSearch,
    Rg,
    Grep,
}

unsafe extern "C" {
    fn prodex_mojo_gemini_guardrail_text_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;
}

fn text_result(operation: GeminiGuardrailTextOperation, text: &str) -> Result<i64, MojoError> {
    let length = i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_mojo_gemini_guardrail_text_v1(
            GEMINI_GUARDRAIL_ABI_VERSION,
            operation as i64,
            text.as_ptr() as usize as u64,
            length,
        )
    };
    if result == -2 {
        Err(MojoError::InvalidInput)
    } else {
        Ok(result)
    }
}

fn bool_result(operation: GeminiGuardrailTextOperation, text: &str) -> Result<bool, MojoError> {
    match text_result(operation, text)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn gemini_wait_or_poll_reason(text: &str) -> Result<Option<GeminiWaitOrPollReason>, MojoError> {
    Ok(
        match text_result(GeminiGuardrailTextOperation::WaitReason, text)? {
            -1 => None,
            0 => Some(GeminiWaitOrPollReason::IWillPoll),
            1 => Some(GeminiWaitOrPollReason::IllPoll),
            2 => Some(GeminiWaitOrPollReason::INeedToWait),
            3 => Some(GeminiWaitOrPollReason::LetsWait),
            4 => Some(GeminiWaitOrPollReason::StillRunning),
            5 => Some(GeminiWaitOrPollReason::IsStillRunning),
            6 => Some(GeminiWaitOrPollReason::IWillWait),
            7 => Some(GeminiWaitOrPollReason::IllWait),
            _ => return Err(MojoError::InvalidOutput),
        },
    )
}

pub fn gemini_tool_intent(text: &str) -> Result<Option<GeminiToolIntent>, MojoError> {
    Ok(
        match text_result(GeminiGuardrailTextOperation::ToolIntent, text)? {
            -1 => None,
            0 => Some(GeminiToolIntent::ExecCommand),
            1 => Some(GeminiToolIntent::WriteStdin),
            2 => Some(GeminiToolIntent::ApplyPatch),
            3 => Some(GeminiToolIntent::SqzGrep),
            4 => Some(GeminiToolIntent::SqzReadFile),
            5 => Some(GeminiToolIntent::SqzListDir),
            6 => Some(GeminiToolIntent::ReadMcpResource),
            7 => Some(GeminiToolIntent::ListMcpResources),
            8 => Some(GeminiToolIntent::ToolSearch),
            9 => Some(GeminiToolIntent::Rg),
            10 => Some(GeminiToolIntent::Grep),
            _ => return Err(MojoError::InvalidOutput),
        },
    )
}

pub fn gemini_success_claim(text: &str) -> Result<bool, MojoError> {
    bool_result(GeminiGuardrailTextOperation::SuccessClaim, text)
}

pub fn gemini_tool_text_has_failure(text: &str) -> Result<bool, MojoError> {
    bool_result(GeminiGuardrailTextOperation::ToolFailure, text)
}

pub fn gemini_tool_text_has_version_lines(text: &str) -> Result<bool, MojoError> {
    bool_result(GeminiGuardrailTextOperation::VersionLines, text)
}

pub fn gemini_verification_marker(text: &str) -> Result<bool, MojoError> {
    bool_result(GeminiGuardrailTextOperation::VerificationMarker, text)
}

pub fn gemini_process_exited_zero(text: &str) -> Result<bool, MojoError> {
    bool_result(GeminiGuardrailTextOperation::ProcessExitedZero, text)
}

pub fn gemini_command_output_only(text: &str) -> Result<bool, MojoError> {
    bool_result(GeminiGuardrailTextOperation::CommandOutputOnly, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_guardrail_text_kernel_preserves_policy_tables() {
        assert_eq!(
            gemini_wait_or_poll_reason("  I'LL POLL once  ").unwrap(),
            Some(GeminiWaitOrPollReason::IllPoll)
        );
        assert_eq!(gemini_wait_or_poll_reason("short").unwrap(), None);

        assert_eq!(
            gemini_tool_intent("Next, I'll use EXEC_COMMAND now").unwrap(),
            Some(GeminiToolIntent::ExecCommand)
        );
        assert_eq!(
            gemini_tool_intent("Next, I'll use my_exec_command_extra now").unwrap(),
            None
        );
        assert_eq!(
            gemini_tool_intent("I will inspect with RG.").unwrap(),
            Some(GeminiToolIntent::Rg)
        );

        assert!(gemini_success_claim("Everything is COMPLETE").unwrap());
        assert!(gemini_tool_text_has_failure("ERROR: command failed").unwrap());
        assert!(!gemini_tool_text_has_failure("process exited with code 0").unwrap());
        assert!(gemini_tool_text_has_version_lines("Prodex 0.433.0\n").unwrap());
        assert!(gemini_verification_marker("verification: ok").unwrap());
        assert!(gemini_process_exited_zero("Process exited with code 0").unwrap());
        assert!(gemini_command_output_only("Answer only with the command output").unwrap());
        assert!(!gemini_command_output_only("Summarize the command output").unwrap());
    }
}
