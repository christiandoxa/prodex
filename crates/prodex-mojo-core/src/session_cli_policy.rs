//! Caller-owned typed ABI for Mojo canonical session-CLI decisions.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionOutputPlan {
    Text,
    Json,
    IdOnly,
    ResumeCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionResumeRepairAction {
    InspectUnrepairable,
    Continue,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionScrollPlan {
    pub offset: usize,
    pub exit: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPromptWriteQueueAction {
    QueueFailed,
    NotAddressable,
    Ambiguous,
    PendingObserved,
    AwaitRollout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPromptWriteQueuePlan {
    pub retry: bool,
    pub action: SessionPromptWriteQueueAction,
}

/// Deterministic projection of one already-parsed transcript event source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptOutputPlan {
    pub visible: bool,
    pub kind: TranscriptOutputKind,
    pub status: TranscriptOutputStatus,
    pub name: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptOutputKind {
    Assistant,
    User,
    Tool,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptOutputStatus {
    None,
    Started,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptOutputTextMode {
    Text,
    Timestamp,
    Name,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub enum SessionPromptWriteProcessRole {
    PlainProdex = 0,
    CodexWriter = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPromptWriteResolutionPlan {
    pub stale: bool,
    pub no_session: bool,
    pub retry: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPromptWriteOutputLineAction {
    Process,
    Limit,
    Oversized,
    InvalidUtf8,
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPromptWriteOutputLineInput {
    pub raw_length: usize,
    pub read_limit: usize,
    pub verify_limit: usize,
    pub utf8_valid: bool,
    pub json_valid: bool,
    pub shape_valid: bool,
    pub visible_user_message: bool,
    pub limit_reached: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPromptWriteVerification {
    QueueFailed,
    NotAddressable,
    Ambiguous,
    PendingObserved,
    AwaitRollout,
}

unsafe extern "C" {
    fn prodex_session_cli_output_mode_v1(
        abi_version: i64,
        json: i64,
        id_only: i64,
        resume_command: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_resume_repair_action_v1(
        abi_version: i64,
        repaired: i64,
        inspected_unrepairable: i64,
        unrepairable_found: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_report_scroll_update_v1(
        abi_version: i64,
        key_code: i64,
        control: i64,
        offset: i64,
        visible: i64,
        max_scroll: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_prompt_write_queue_plan_v1(
        abi_version: i64,
        outcome: i64,
        queued: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_output_event_plan_v1(
        abi_version: i64,
        source_address: u64,
        source_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_output_text_v1(
        abi_version: i64,
        mode: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_session_prompt_write_policy_v1(
        abi_version: i64,
        operation: i64,
        first: i64,
        second: i64,
        third: i64,
        fourth: i64,
        fifth: i64,
        sixth: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_prompt_write_gap_text_v1(
        abi_version: i64,
        reason: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

const PROMPT_WRITE_POLICY_ABI_VERSION: i64 = 1;
const PROMPT_WRITE_POLICY_PROCESS_ROLE: i64 = 1;
const PROMPT_WRITE_POLICY_RESOLUTION: i64 = 2;
const PROMPT_WRITE_POLICY_OUTPUT_LINE: i64 = 3;
const PROMPT_WRITE_POLICY_VERIFICATION: i64 = 5;
const PROMPT_WRITE_POLICY_RECORD_SHAPE: i64 = 6;
const PROMPT_WRITE_POLICY_ENDPOINT_ARGS: i64 = 7;
const PROMPT_WRITE_POLICY_USER_VISIBILITY: i64 = 8;

fn valid_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn output_mode(
    json: bool,
    id_only: bool,
    resume_command: bool,
) -> Result<Option<SessionOutputPlan>, MojoError> {
    let mut result = [-1_i64; 1];
    let status = unsafe {
        prodex_session_cli_output_mode_v1(
            ABI_VERSION,
            i64::from(json),
            i64::from(id_only),
            i64::from(resume_command),
            result.as_mut_ptr() as usize as u64,
        )
    };
    if status == 2 {
        return Ok(None);
    }
    valid_status(status)?;
    let selected = match result[0] {
        0 => SessionOutputPlan::Text,
        1 => SessionOutputPlan::Json,
        2 => SessionOutputPlan::IdOnly,
        3 => SessionOutputPlan::ResumeCommand,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(Some(selected))
}

pub fn resume_repair_action(
    repaired: bool,
    inspected_unrepairable: bool,
    unrepairable_found: bool,
) -> Result<SessionResumeRepairAction, MojoError> {
    let mut result = [-1_i64; 1];
    valid_status(unsafe {
        prodex_session_resume_repair_action_v1(
            ABI_VERSION,
            i64::from(repaired),
            i64::from(inspected_unrepairable),
            i64::from(unrepairable_found),
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    match result[0] {
        0 => Ok(SessionResumeRepairAction::InspectUnrepairable),
        1 => Ok(SessionResumeRepairAction::Continue),
        2 => Ok(SessionResumeRepairAction::Reject),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn scroll_update(
    key_code: i64,
    control: bool,
    offset: usize,
    visible: usize,
    max_scroll: usize,
) -> Result<SessionScrollPlan, MojoError> {
    let mut result = [-1_i64; 2];
    valid_status(unsafe {
        prodex_session_report_scroll_update_v1(
            ABI_VERSION,
            key_code,
            i64::from(control),
            i64::try_from(offset).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(visible).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_scroll).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    let exit = match result[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let offset = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    if offset > max_scroll {
        return Err(MojoError::InvalidOutput);
    }
    Ok(SessionScrollPlan { offset, exit })
}

pub fn prompt_write_queue_plan(
    outcome: u8,
    queued: bool,
) -> Result<SessionPromptWriteQueuePlan, MojoError> {
    if outcome > 3 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 2];
    valid_status(unsafe {
        prodex_session_prompt_write_queue_plan_v1(
            ABI_VERSION,
            i64::from(outcome),
            i64::from(queued),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let retry = match output[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let action = match output[1] {
        0 => SessionPromptWriteQueueAction::QueueFailed,
        1 => SessionPromptWriteQueueAction::NotAddressable,
        2 => SessionPromptWriteQueueAction::Ambiguous,
        3 => SessionPromptWriteQueueAction::PendingObserved,
        4 => SessionPromptWriteQueueAction::AwaitRollout,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(SessionPromptWriteQueuePlan { retry, action })
}

pub fn prompt_write_process_role_allowed(
    role: SessionPromptWriteProcessRole,
    executable_matches: bool,
    command: u8,
    forbidden: bool,
    remote: bool,
) -> Result<bool, MojoError> {
    if command > 3 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 8];
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_PROCESS_ROLE,
            role as i64,
            i64::from(executable_matches),
            i64::from(command),
            i64::from(forbidden),
            i64::from(remote),
            0,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn prompt_write_resolution_plan(
    targeted: bool,
    no_session: bool,
    retryable: bool,
) -> Result<SessionPromptWriteResolutionPlan, MojoError> {
    let mut output = [-1_i64; 8];
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_RESOLUTION,
            i64::from(targeted),
            i64::from(no_session),
            i64::from(retryable),
            0,
            0,
            0,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let bool_output = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    Ok(SessionPromptWriteResolutionPlan {
        stale: bool_output(output[0])?,
        no_session: bool_output(output[1])?,
        retry: bool_output(output[2])?,
    })
}

pub fn prompt_write_output_line_plan(
    input: SessionPromptWriteOutputLineInput,
) -> Result<SessionPromptWriteOutputLineAction, MojoError> {
    let mut output = [-1_i64; 8];
    output[7] = i64::from(input.visible_user_message) + 2 * i64::from(input.limit_reached);
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_OUTPUT_LINE,
            i64::try_from(input.raw_length).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(input.read_limit).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(input.verify_limit).map_err(|_| MojoError::InvalidInput)?,
            i64::from(input.utf8_valid),
            i64::from(input.json_valid),
            i64::from(input.shape_valid),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(SessionPromptWriteOutputLineAction::Process),
        1 => Ok(SessionPromptWriteOutputLineAction::Limit),
        2 => Ok(SessionPromptWriteOutputLineAction::Oversized),
        3 => Ok(SessionPromptWriteOutputLineAction::InvalidUtf8),
        4 => Ok(SessionPromptWriteOutputLineAction::Malformed),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn prompt_write_gap_text(reason: u8) -> Result<String, MojoError> {
    if reason > 2 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![0_u8; 96];
    let mut written = -1_i64;
    valid_status(unsafe {
        prodex_session_prompt_write_gap_text_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            i64::from(reason),
            output.as_mut_ptr() as usize as u64,
            output.len() as i64,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

pub fn prompt_write_verification_plan(
    action: SessionPromptWriteQueueAction,
) -> Result<SessionPromptWriteVerification, MojoError> {
    let mut output = [-1_i64; 8];
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_VERIFICATION,
            action as i64,
            0,
            0,
            0,
            0,
            0,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(SessionPromptWriteVerification::QueueFailed),
        1 => Ok(SessionPromptWriteVerification::NotAddressable),
        2 => Ok(SessionPromptWriteVerification::Ambiguous),
        3 => Ok(SessionPromptWriteVerification::PendingObserved),
        4 => Ok(SessionPromptWriteVerification::AwaitRollout),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn prompt_write_record_shape(
    record_type: u8,
    payload_present: bool,
    payload_object: bool,
    payload_type_string: bool,
) -> Result<bool, MojoError> {
    if record_type > 5 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 8];
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_RECORD_SHAPE,
            i64::from(record_type),
            i64::from(payload_present),
            i64::from(payload_object),
            i64::from(payload_type_string),
            0,
            0,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn prompt_write_endpoint_mode(
    app_server_command: bool,
    inline_listen: bool,
    separate_listen: bool,
    separate_value_present: bool,
) -> Result<u8, MojoError> {
    let mut output = [-1_i64; 8];
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_ENDPOINT_ARGS,
            i64::from(app_server_command),
            i64::from(inline_listen),
            i64::from(separate_listen),
            i64::from(separate_value_present),
            0,
            0,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    u8::try_from(output[0])
        .ok()
        .filter(|mode| *mode <= 2)
        .ok_or(MojoError::InvalidOutput)
}

pub fn prompt_write_user_message_visible(
    record_type: u8,
    payload_user_message: bool,
    role_user: bool,
    metadata_present: bool,
    kinds_nonempty: bool,
    kinds_all_user_text: bool,
) -> Result<bool, MojoError> {
    if record_type > 2 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 8];
    valid_status(unsafe {
        prodex_session_prompt_write_policy_v1(
            PROMPT_WRITE_POLICY_ABI_VERSION,
            PROMPT_WRITE_POLICY_USER_VISIBILITY,
            i64::from(record_type),
            i64::from(payload_user_message),
            i64::from(role_user),
            i64::from(metadata_present),
            i64::from(kinds_nonempty),
            i64::from(kinds_all_user_text),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn transcript_output_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn transcript_input_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

/// Select visibility, labels, status, and source-name bounds for one transcript event.
pub fn transcript_output_plan(source: &str) -> Result<TranscriptOutputPlan, MojoError> {
    let mut output = [-1_i64; 5];
    transcript_output_status(unsafe {
        prodex_mojo_transcript_output_event_plan_v1(
            1,
            source.as_ptr() as usize as u64,
            transcript_input_len(source)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let visible = match output[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let kind = match output[1] {
        0 => TranscriptOutputKind::Assistant,
        1 => TranscriptOutputKind::User,
        2 => TranscriptOutputKind::Tool,
        3 => TranscriptOutputKind::Other,
        _ => return Err(MojoError::InvalidOutput),
    };
    let status = match output[2] {
        0 => TranscriptOutputStatus::None,
        1 => TranscriptOutputStatus::Started,
        2 => TranscriptOutputStatus::Completed,
        _ => return Err(MojoError::InvalidOutput),
    };
    let name = match (output[3], output[4]) {
        (-1, -1) => None,
        (start, end) if start >= 0 && end >= start => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            Some(
                source
                    .get(start..end)
                    .map(|_| (start, end))
                    .ok_or(MojoError::InvalidOutput)?,
            )
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(TranscriptOutputPlan {
        visible,
        kind,
        status,
        name,
    })
}

/// Apply the bounded output text policy without reimplementing its UTF-8 decisions in Rust.
pub fn transcript_output_bounded(
    value: &str,
    mode: TranscriptOutputTextMode,
) -> Result<String, MojoError> {
    let mut output = vec![0_u8; value.len().max(1)];
    let mut written = -1_i64;
    transcript_output_status(unsafe {
        prodex_mojo_transcript_output_text_v1(
            1,
            mode as i64,
            value.as_ptr() as usize as u64,
            transcript_input_len(value)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "session_cli_policy/tests.rs"]
mod tests;
