use super::*;

#[test]
fn session_reports_stream_large_jsonl_session_files() {
    let root = test_temp_dir("session-large-jsonl");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "01900000-0000-7000-8000-000000000029";
    let path = sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    let mut raw = format!(
        "{{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"timestamp\":\"2026-06-13T02:04:31Z\",\"cwd\":\"/tmp/workspace\",\"originator\":\"codex-tui\",\"cli_version\":\"0.144.3\"}}}}\n"
    );
    while raw.len() as u64 <= SESSION_STORE_FILE_MAX_BYTES {
        raw.push_str("{\"timestamp\":\"2026-06-13T02:05:00Z\",\"type\":\"event\"}\n");
    }
    fs::write(&path, raw).expect("large session should be written");

    assert_eq!(
        repair_resume_session_metadata_prefix(&root, session_id)
            .expect("valid large session should not need repair"),
        None
    );
    assert_eq!(
        find_unrepairable_resume_session(&root, session_id)
            .expect("valid large session should remain resumable"),
        None
    );
    let reports = collect_session_reports(&root, None, &AppState::default())
        .expect("large session should stream");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, session_id);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_reject_oversized_session_line() {
    let root = test_temp_dir("session-oversized-line");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let path = sessions.join("oversized.jsonl");
    fs::write(&path, vec![b' '; SESSION_STORE_FILE_MAX_BYTES as usize + 1])
        .expect("oversized line should be written");

    let err = collect_session_reports(&root, None, &AppState::default())
        .expect_err("oversized session line should be rejected");
    assert!(format!("{err:#}").contains("exceeds safe size limit"));

    let _ = fs::remove_dir_all(root);
}
