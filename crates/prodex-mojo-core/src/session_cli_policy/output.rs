use crate::MojoError;

const ABI_VERSION: i64 = 1;

/// Facts from one already-parsed transcript event used to admit it to an output page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPromptWriteOutputEventInput {
    pub visible: bool,
    pub cursor_index: usize,
    pub start_index: usize,
    pub event_count: usize,
    pub limit: usize,
    pub total_text_bytes: usize,
    pub event_text_bytes: usize,
    pub total_text_limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPromptWriteOutputEventAction {
    Ignore,
    AdvanceCursor,
    Include,
    Limit,
    TextLimit,
}

unsafe extern "C" {
    fn prodex_session_prompt_write_output_event_plan_v1(
        abi_version: i64,
        visible: i64,
        cursor_index: i64,
        start_index: i64,
        event_count: i64,
        limit: i64,
        total_text_bytes: i64,
        event_text_bytes: i64,
        total_text_limit: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_session_prompt_write_exact_user_message_match_v1(
        abi_version: i64,
        record_type: i64,
        payload_user_message: i64,
        role_user: i64,
        visible: i64,
        message_matches: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn input_length(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn bool_input(value: bool) -> i64 {
    i64::from(value)
}

fn bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ask Mojo whether one parsed output event advances, stops, or enters the page.
pub fn output_event_plan(
    input: SessionPromptWriteOutputEventInput,
) -> Result<SessionPromptWriteOutputEventAction, MojoError> {
    let mut output = [-1_i64; 1];
    status(unsafe {
        prodex_session_prompt_write_output_event_plan_v1(
            ABI_VERSION,
            bool_input(input.visible),
            input_length(input.cursor_index)?,
            input_length(input.start_index)?,
            input_length(input.event_count)?,
            input_length(input.limit)?,
            input_length(input.total_text_bytes)?,
            input_length(input.event_text_bytes)?,
            input_length(input.total_text_limit)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(SessionPromptWriteOutputEventAction::Ignore),
        1 => Ok(SessionPromptWriteOutputEventAction::AdvanceCursor),
        2 => Ok(SessionPromptWriteOutputEventAction::Include),
        3 => Ok(SessionPromptWriteOutputEventAction::Limit),
        4 => Ok(SessionPromptWriteOutputEventAction::TextLimit),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Match one parsed rollout record without passing user text into the policy ABI.
pub fn exact_user_message_match(
    record_type: u8,
    payload_user_message: bool,
    role_user: bool,
    visible: bool,
    message_matches: bool,
) -> Result<bool, MojoError> {
    if record_type > 2 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 1];
    status(unsafe {
        prodex_session_prompt_write_exact_user_message_match_v1(
            ABI_VERSION,
            i64::from(record_type),
            bool_input(payload_user_message),
            bool_input(role_user),
            bool_input(visible),
            bool_input(message_matches),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    bool_output(output[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_event_plan_preserves_cursor_and_budget_boundaries() {
        assert_eq!(
            output_event_plan(SessionPromptWriteOutputEventInput {
                visible: false,
                cursor_index: 0,
                start_index: 0,
                event_count: 0,
                limit: 1,
                total_text_bytes: 0,
                event_text_bytes: 4,
                total_text_limit: 8,
            }),
            Ok(SessionPromptWriteOutputEventAction::Ignore)
        );
        assert_eq!(
            output_event_plan(SessionPromptWriteOutputEventInput {
                visible: true,
                cursor_index: 0,
                start_index: 1,
                event_count: 0,
                limit: 1,
                total_text_bytes: 0,
                event_text_bytes: 4,
                total_text_limit: 8,
            }),
            Ok(SessionPromptWriteOutputEventAction::AdvanceCursor)
        );
        assert_eq!(
            output_event_plan(SessionPromptWriteOutputEventInput {
                visible: true,
                cursor_index: 1,
                start_index: 0,
                event_count: 1,
                limit: 1,
                total_text_bytes: 0,
                event_text_bytes: 4,
                total_text_limit: 8,
            }),
            Ok(SessionPromptWriteOutputEventAction::Limit)
        );
        assert_eq!(
            output_event_plan(SessionPromptWriteOutputEventInput {
                visible: true,
                cursor_index: 0,
                start_index: 0,
                event_count: 0,
                limit: 1,
                total_text_bytes: 8,
                event_text_bytes: 1,
                total_text_limit: 8,
            }),
            Ok(SessionPromptWriteOutputEventAction::TextLimit)
        );
        assert_eq!(
            output_event_plan(SessionPromptWriteOutputEventInput {
                visible: true,
                cursor_index: 0,
                start_index: 0,
                event_count: 0,
                limit: 1,
                total_text_bytes: 3,
                event_text_bytes: 4,
                total_text_limit: 8,
            }),
            Ok(SessionPromptWriteOutputEventAction::Include)
        );
    }

    #[test]
    fn exact_user_message_match_keeps_event_and_visible_response_rules() {
        assert!(exact_user_message_match(1, true, false, false, true).unwrap());
        assert!(!exact_user_message_match(1, false, false, false, true).unwrap());
        assert!(exact_user_message_match(2, true, true, true, true).unwrap());
        assert!(!exact_user_message_match(2, true, true, false, true).unwrap());
        assert!(!exact_user_message_match(2, true, true, true, false).unwrap());
        assert_eq!(
            exact_user_message_match(3, true, true, true, true),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn output_abi_rejects_wrong_version_and_invalid_boolean_facts() {
        let mut output = [-1_i64; 1];
        let address = output.as_mut_ptr() as usize as u64;
        assert_eq!(
            unsafe {
                prodex_session_prompt_write_output_event_plan_v1(0, 1, 0, 0, 0, 1, 0, 0, 1, address)
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_session_prompt_write_output_event_plan_v1(
                    ABI_VERSION,
                    2,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    1,
                    address,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                prodex_session_prompt_write_exact_user_message_match_v1(
                    ABI_VERSION,
                    2,
                    1,
                    1,
                    1,
                    1,
                    address,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                prodex_session_prompt_write_exact_user_message_match_v1(
                    ABI_VERSION,
                    3,
                    1,
                    1,
                    1,
                    1,
                    address,
                )
            },
            1
        );
    }
}
