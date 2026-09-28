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
    fn prodex_mojo_gemini_guardrail_slice_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
        output_start_address: u64,
        output_length_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_guardrail_marker_match_v1(
        abi_version: i64,
        required_address: u64,
        required_length: i64,
        text_address: u64,
        text_length: i64,
    ) -> i64;
    fn prodex_mojo_gemini_guardrail_command_empty_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        scratch_address: u64,
        scratch_capacity: i64,
    ) -> i64;
    fn prodex_mojo_gemini_guardrail_command_match_v1(
        abi_version: i64,
        required_address: u64,
        required_length: i64,
        command_address: u64,
        command_length: i64,
        required_scratch_address: u64,
        required_scratch_capacity: i64,
        command_scratch_address: u64,
        command_scratch_capacity: i64,
    ) -> i64;
    fn prodex_mojo_gemini_guardrail_text_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn bool_direct_result(result: i64) -> Result<bool, MojoError> {
    match result {
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn slice_result(operation: i64, text: &str) -> Result<Option<&str>, MojoError> {
    let mut start = -1_i64;
    let mut length = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_guardrail_slice_v1(
            GEMINI_GUARDRAIL_ABI_VERSION,
            operation,
            text.as_ptr() as usize as u64,
            signed_len(text)?,
            (&mut start as *mut i64) as usize as u64,
            (&mut length as *mut i64) as usize as u64,
        )
    };
    if status == -2 {
        return Err(MojoError::InvalidInput);
    }
    if status != 0 {
        return Err(MojoError::InvalidOutput);
    }
    if start == -1 {
        return if length == 0 {
            Ok(None)
        } else {
            Err(MojoError::InvalidOutput)
        };
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    let end = start
        .checked_add(length)
        .filter(|end| *end <= text.len())
        .ok_or(MojoError::InvalidOutput)?;
    text.get(start..end)
        .map(Some)
        .ok_or(MojoError::InvalidOutput)
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

pub fn gemini_required_exact_output_command(text: &str) -> Result<Option<&str>, MojoError> {
    slice_result(1, text)
}

pub fn gemini_extract_command_output(text: &str) -> Result<Option<&str>, MojoError> {
    slice_result(2, text)
}

pub fn gemini_exact_output_marker_match(required: &str, text: &str) -> Result<bool, MojoError> {
    bool_direct_result(unsafe {
        prodex_mojo_gemini_guardrail_marker_match_v1(
            GEMINI_GUARDRAIL_ABI_VERSION,
            required.as_ptr() as usize as u64,
            signed_len(required)?,
            text.as_ptr() as usize as u64,
            signed_len(text)?,
        )
    })
}

pub fn gemini_normalized_command_is_empty(command: &str) -> Result<bool, MojoError> {
    let mut scratch = vec![0_u8; command.len().max(1)];
    bool_direct_result(unsafe {
        prodex_mojo_gemini_guardrail_command_empty_v1(
            GEMINI_GUARDRAIL_ABI_VERSION,
            command.as_ptr() as usize as u64,
            signed_len(command)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    })
}

pub fn gemini_command_matches_required(required: &str, command: &str) -> Result<bool, MojoError> {
    let mut required_scratch = vec![0_u8; required.len().max(1)];
    let mut command_scratch = vec![0_u8; command.len().max(1)];
    bool_direct_result(unsafe {
        prodex_mojo_gemini_guardrail_command_match_v1(
            GEMINI_GUARDRAIL_ABI_VERSION,
            required.as_ptr() as usize as u64,
            signed_len(required)?,
            command.as_ptr() as usize as u64,
            signed_len(command)?,
            required_scratch.as_mut_ptr() as usize as u64,
            i64::try_from(required_scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            command_scratch.as_mut_ptr() as usize as u64,
            i64::try_from(command_scratch.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    })
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
        assert_eq!(
            gemini_required_exact_output_command("Please RUN EXACTLY:\n  cargo   test  \n")
                .unwrap(),
            Some("cargo   test")
        );
        assert_eq!(
            gemini_required_exact_output_command("Do this, then run  cargo check. Continue.")
                .unwrap(),
            Some("cargo check")
        );
        assert_eq!(
            gemini_extract_command_output("prefix\nOutput:\n  ok\n\ndiff --git a/a b/a\nignored")
                .unwrap(),
            Some("ok")
        );
        assert_eq!(
            gemini_extract_command_output("Output:\nSuccess. Updated the following files:\na.rs")
                .unwrap(),
            None
        );
        assert!(
            gemini_exact_output_marker_match(
                "echo PRODEX_VERIFY_12345",
                "done PRODEX_VERIFY_12345"
            )
            .unwrap()
        );
        assert!(
            !gemini_exact_output_marker_match(
                "echo PRODEX_VERIFY_12345",
                "done prodex_verify_12345"
            )
            .unwrap()
        );
        assert!(gemini_normalized_command_is_empty("  \"''\"  ").unwrap());
        assert!(gemini_command_matches_required("\"cargo    test\"", "cargo test").unwrap());
        assert!(
            gemini_command_matches_required(
                "echo PRODEX_VERIFY_12345",
                "printf x && echo PRODEX_VERIFY_12345"
            )
            .unwrap()
        );
    }
}
