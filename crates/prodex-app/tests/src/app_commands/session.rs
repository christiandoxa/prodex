use super::*;

fn test_session_report(id: &str) -> SessionReport {
    let mut report = SessionReport::from_path(Path::new(&format!("/tmp/{id}.jsonl")), 0);
    prodex_session_store::apply_session_json_line(
        &mut report,
        r#"{"timestamp":"2026-06-26T10:00:00Z","type":"session_meta","payload":{"thread_name":"Build UI","cwd":"/tmp/prodex"}}"#,
    );
    report.set_profile(Some("main".to_string()));
    report.set_model_provider(Some("openai".to_string()));
    report
}

#[test]
fn session_output_mode_comes_from_mojo_and_preserves_conflict_error() {
    assert_eq!(
        session_output_mode(false, false, false).unwrap(),
        SessionOutputMode::Text
    );
    assert_eq!(
        session_output_mode(true, false, false).unwrap(),
        SessionOutputMode::Json
    );
    assert_eq!(
        session_output_mode(false, true, false).unwrap(),
        SessionOutputMode::IdOnly
    );
    assert_eq!(
        session_output_mode(false, false, true).unwrap(),
        SessionOutputMode::ResumeCommand
    );
    assert_eq!(
        session_output_mode(true, true, false)
            .unwrap_err()
            .to_string(),
        "--json, --id-only, and --resume-command cannot be combined"
    );
}

#[test]
fn session_resume_repair_plan_keeps_repair_precedence() {
    assert_eq!(
        session_resume_repair_action(true, false, false).unwrap(),
        SessionResumeRepairAction::Continue
    );
    assert_eq!(
        session_resume_repair_action(true, true, true).unwrap(),
        SessionResumeRepairAction::Continue
    );
    assert_eq!(
        session_resume_repair_action(false, false, false).unwrap(),
        SessionResumeRepairAction::InspectUnrepairable
    );
    assert_eq!(
        session_resume_repair_action(false, true, true).unwrap(),
        SessionResumeRepairAction::Reject
    );
    assert_eq!(
        session_resume_repair_action(false, true, false).unwrap(),
        SessionResumeRepairAction::Continue
    );
}

#[test]
fn session_scroll_plan_keeps_key_and_viewport_boundaries() {
    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(
        session_scroll_update(Some(&page_down), 8, 4, 10).unwrap(),
        (10, false)
    );

    let page_up = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
    assert_eq!(
        session_scroll_update(Some(&page_up), 2, 4, 10).unwrap(),
        (0, false)
    );

    let end = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(
        session_scroll_update(Some(&end), 2, 4, 10).unwrap(),
        (10, false)
    );

    let ctrl_z = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(
        session_scroll_update(Some(&ctrl_z), 10, 4, 10).unwrap(),
        (10, true)
    );

    let other = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(
        session_scroll_update(Some(&other), 5, 4, 10).unwrap(),
        (5, false)
    );
    let down_at_end = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(
        session_scroll_update(Some(&down_at_end), 10, 4, 10).unwrap(),
        (10, false)
    );
}

#[test]
fn session_mojo_abis_reject_malformed_inputs() {
    let mut output = [-1_i64; 2];
    let output_address = output.as_mut_ptr() as u64;
    assert_eq!(
        unsafe { prodex_session_cli_output_mode_v1(0, 0, 0, 0, output_address) },
        1
    );
    assert_eq!(
        unsafe {
            prodex_session_cli_output_mode_v1(SESSION_CLI_ABI_VERSION, 2, 0, 0, output_address)
        },
        1
    );
    assert_eq!(
        unsafe {
            prodex_session_resume_repair_action_v1(SESSION_CLI_ABI_VERSION, 0, 0, 2, output_address)
        },
        1
    );
    assert_eq!(
        unsafe {
            prodex_session_report_scroll_update_v1(
                SESSION_CLI_ABI_VERSION,
                0,
                0,
                0,
                -1,
                0,
                output_address,
            )
        },
        1
    );
}

#[test]
fn session_report_tui_height_scales_with_reports() {
    assert!(session_report_tui_height(&[] as &[SessionReport]) >= 1);
    let reports = vec![test_session_report("a"), test_session_report("b")];
    assert!(usize::from(session_report_tui_height(&reports)) >= 8);
    assert_eq!(session_tui_item_count(&reports), 8);
}

#[test]
fn session_report_tui_item_contains_key_fields() {
    let report = test_session_report("session-1");
    let item = session_report_tui_item(&report);
    let text = format!("{item:?}");
    assert!(text.contains("session-1"));
    assert!(text.contains("Build UI"));
    assert!(text.contains("main"));
    assert!(text.contains("openai"));
}
