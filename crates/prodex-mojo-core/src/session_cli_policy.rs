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
}

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
mod tests {
    use super::*;

    #[test]
    fn mojo_session_ffi_rejects_malformed_inputs() {
        let mut output = [-1_i64; 2];
        let output_address = output.as_mut_ptr() as usize as u64;
        assert_eq!(
            unsafe { prodex_session_cli_output_mode_v1(0, 0, 0, 0, output_address) },
            1,
        );
        assert_eq!(
            unsafe { prodex_session_cli_output_mode_v1(ABI_VERSION, 2, 0, 0, output_address) },
            1,
        );
        assert_eq!(
            unsafe { prodex_session_resume_repair_action_v1(ABI_VERSION, 0, 0, 2, output_address) },
            1,
        );
        assert_eq!(
            unsafe {
                prodex_session_report_scroll_update_v1(ABI_VERSION, 0, 0, 0, -1, 0, output_address)
            },
            1,
        );
        assert_eq!(
            unsafe { prodex_session_prompt_write_queue_plan_v1(ABI_VERSION, 4, 0, output_address) },
            1,
        );
        assert_eq!(
            unsafe { prodex_session_prompt_write_queue_plan_v1(ABI_VERSION, 2, 2, output_address) },
            1,
        );
    }

    #[test]
    fn direct_mojo_session_policy_distinguishes_conflict_and_repair_transition() {
        assert_eq!(
            output_mode(true, false, false),
            Ok(Some(SessionOutputPlan::Json))
        );
        assert_eq!(output_mode(true, true, false), Ok(None));
        assert_eq!(
            resume_repair_action(false, false, false),
            Ok(SessionResumeRepairAction::InspectUnrepairable),
        );
        assert_eq!(
            resume_repair_action(false, true, true),
            Ok(SessionResumeRepairAction::Reject),
        );
        assert_eq!(
            resume_repair_action(true, false, true),
            Ok(SessionResumeRepairAction::Continue),
        );
    }

    #[test]
    fn direct_mojo_session_scroll_has_bounded_transitions() {
        assert_eq!(
            scroll_update(-3, false, 4, 6, 8),
            Ok(SessionScrollPlan {
                offset: 8,
                exit: false
            }),
        );
        assert_eq!(
            scroll_update(i64::from('q' as u32), false, 4, 6, 8),
            Ok(SessionScrollPlan {
                offset: 4,
                exit: true
            }),
        );
        assert_eq!(
            scroll_update(-3, false, 9, 6, 8),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn direct_mojo_prompt_write_queue_plan_preserves_retry_and_verification_states() {
        assert_eq!(
            prompt_write_queue_plan(0, false).unwrap(),
            SessionPromptWriteQueuePlan {
                retry: true,
                action: SessionPromptWriteQueueAction::QueueFailed,
            }
        );
        assert_eq!(
            prompt_write_queue_plan(2, true).unwrap(),
            SessionPromptWriteQueuePlan {
                retry: false,
                action: SessionPromptWriteQueueAction::PendingObserved,
            }
        );
        assert_eq!(
            prompt_write_queue_plan(2, false).unwrap(),
            SessionPromptWriteQueuePlan {
                retry: false,
                action: SessionPromptWriteQueueAction::AwaitRollout,
            }
        );
        assert_eq!(
            prompt_write_queue_plan(4, false),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn transcript_output_projection_and_bounds_are_mojo_owned() {
        assert_eq!(
            transcript_output_plan("tool-call:search").unwrap(),
            TranscriptOutputPlan {
                visible: true,
                kind: TranscriptOutputKind::Tool,
                status: TranscriptOutputStatus::Started,
                name: Some((10, 16)),
            }
        );
        assert!(!transcript_output_plan("reasoning").unwrap().visible);
        assert_eq!(
            transcript_output_bounded("ééé", TranscriptOutputTextMode::Timestamp).unwrap(),
            "ééé"
        );
        let bounded = transcript_output_bounded(
            &format!("{}tail", "x".repeat(8_192)),
            TranscriptOutputTextMode::Text,
        )
        .unwrap();
        assert_eq!(bounded.len(), 8_192);
        assert!(bounded.ends_with("[text_truncated]"));
        assert_eq!(
            transcript_output_bounded(&"x".repeat(8_192), TranscriptOutputTextMode::Text)
                .unwrap()
                .len(),
            8_192
        );
        assert_eq!(
            transcript_output_bounded(&"é".repeat(256), TranscriptOutputTextMode::Name)
                .unwrap()
                .chars()
                .count(),
            256
        );
    }

    #[test]
    fn transcript_output_abi_rejects_wrong_version_and_small_buffers() {
        let source = "tool-call:x";
        let mut plan = [0_i64; 5];
        assert_eq!(
            unsafe {
                prodex_mojo_transcript_output_event_plan_v1(
                    0,
                    source.as_ptr() as u64,
                    source.len() as i64,
                    plan.as_mut_ptr() as u64,
                )
            },
            1
        );
        let value = "x".repeat(8_193);
        let mut output = [0_u8; 1];
        let mut written = -1_i64;
        assert_eq!(
            unsafe {
                prodex_mojo_transcript_output_text_v1(
                    1,
                    TranscriptOutputTextMode::Text as i64,
                    value.as_ptr() as u64,
                    value.len() as i64,
                    output.as_mut_ptr() as u64,
                    output.len() as i64,
                    (&mut written as *mut i64) as u64,
                )
            },
            3
        );
    }
}
