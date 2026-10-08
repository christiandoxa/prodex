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
            unsafe {
                prodex_session_resume_repair_action_v1(ABI_VERSION, 0, 0, 2, output_address)
            },
            1,
        );
        assert_eq!(
            unsafe {
                prodex_session_report_scroll_update_v1(ABI_VERSION, 0, 0, 0, -1, 0, output_address)
            },
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
}
