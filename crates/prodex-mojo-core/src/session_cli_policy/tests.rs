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
        prompt_write_queue_plan(1, false).unwrap(),
        SessionPromptWriteQueuePlan {
            retry: true,
            action: SessionPromptWriteQueueAction::NotAddressable,
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

#[test]
fn direct_mojo_prompt_write_policy_covers_roles_resolution_and_line_bounds() {
    assert!(
        prompt_write_process_role_allowed(
            SessionPromptWriteProcessRole::PlainProdex,
            true,
            1,
            false,
            false,
        )
        .unwrap()
    );
    assert!(
        !prompt_write_process_role_allowed(
            SessionPromptWriteProcessRole::CodexWriter,
            true,
            3,
            false,
            false,
        )
        .unwrap()
    );
    assert_eq!(
        prompt_write_resolution_plan(true, true, false).unwrap(),
        SessionPromptWriteResolutionPlan {
            stale: true,
            no_session: false,
            retry: false,
        }
    );
    assert_eq!(
        prompt_write_output_line_plan(SessionPromptWriteOutputLineInput {
            raw_length: 70_000,
            read_limit: 64 * 1024,
            verify_limit: 512 * 1024,
            utf8_valid: true,
            json_valid: true,
            shape_valid: true,
            visible_user_message: true,
            limit_reached: false,
        })
        .unwrap(),
        SessionPromptWriteOutputLineAction::Process
    );
    assert_eq!(
        prompt_write_output_line_plan(SessionPromptWriteOutputLineInput {
            raw_length: 70_000,
            read_limit: 64 * 1024,
            verify_limit: 512 * 1024,
            utf8_valid: true,
            json_valid: true,
            shape_valid: true,
            visible_user_message: false,
            limit_reached: false,
        })
        .unwrap(),
        SessionPromptWriteOutputLineAction::Oversized
    );
    assert_eq!(
        prompt_write_output_line_plan(SessionPromptWriteOutputLineInput {
            raw_length: 4,
            read_limit: 64 * 1024,
            verify_limit: 512 * 1024,
            utf8_valid: true,
            json_valid: false,
            shape_valid: false,
            visible_user_message: false,
            limit_reached: false,
        })
        .unwrap(),
        SessionPromptWriteOutputLineAction::Malformed
    );
    assert_eq!(
        prompt_write_gap_text(2).unwrap(),
        "output gap: malformed_record; record omitted"
    );
    assert!(prompt_write_record_shape(1, true, true, true).unwrap());
    assert!(!prompt_write_record_shape(1, true, true, false).unwrap());
    assert!(prompt_write_record_shape(5, true, false, false).unwrap());
    assert_eq!(
        prompt_write_endpoint_mode(true, true, false, false).unwrap(),
        1
    );
    assert_eq!(
        prompt_write_endpoint_mode(true, false, true, true).unwrap(),
        2
    );
    assert_eq!(
        prompt_write_endpoint_mode(false, true, false, false).unwrap(),
        0
    );
    assert!(prompt_write_user_message_visible(1, true, false, false, false, false).unwrap());
    assert!(prompt_write_user_message_visible(2, true, true, false, false, false).unwrap());
    assert!(!prompt_write_user_message_visible(2, true, true, true, true, false).unwrap());
    for (action, expected) in [
        (
            SessionPromptWriteQueueAction::QueueFailed,
            SessionPromptWriteVerification::QueueFailed,
        ),
        (
            SessionPromptWriteQueueAction::NotAddressable,
            SessionPromptWriteVerification::NotAddressable,
        ),
        (
            SessionPromptWriteQueueAction::Ambiguous,
            SessionPromptWriteVerification::Ambiguous,
        ),
        (
            SessionPromptWriteQueueAction::PendingObserved,
            SessionPromptWriteVerification::PendingObserved,
        ),
        (
            SessionPromptWriteQueueAction::AwaitRollout,
            SessionPromptWriteVerification::AwaitRollout,
        ),
    ] {
        assert_eq!(prompt_write_verification_plan(action).unwrap(), expected);
    }
}

#[test]
fn direct_mojo_prompt_write_policy_rejects_invalid_matrix_values() {
    assert_eq!(
        prompt_write_process_role_allowed(
            SessionPromptWriteProcessRole::PlainProdex,
            true,
            4,
            false,
            false,
        ),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(prompt_write_gap_text(3), Err(MojoError::InvalidInput));
    assert_eq!(
        prompt_write_output_line_plan(SessionPromptWriteOutputLineInput {
            raw_length: 1,
            read_limit: 2,
            verify_limit: 3,
            utf8_valid: true,
            json_valid: true,
            shape_valid: true,
            visible_user_message: false,
            limit_reached: false,
        }),
        Ok(SessionPromptWriteOutputLineAction::Process)
    );
}
