use super::*;

#[test]
fn repair_codex_session_metadata_prefix_preserves_subagent_metadata() {
    let root = test_temp_dir("session-preserve-subagent-metadata");
    let session_id = "01900000-0000-7000-8000-000000000001";
    let session_path = root.join(format!("rollout-2026-07-11T11-17-19-{session_id}.jsonl"));
    let raw = format!(
        "{{\"timestamp\":\"2026-07-11T11:17:19Z\",\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"01900000-0000-7000-8000-000000000002\",\"id\":\"{session_id}\",\"timestamp\":\"2026-07-11T11:17:19Z\",\"cwd\":\"/tmp/workspace\",\"originator\":\"codex-tui\",\"cli_version\":\"0.144.1\",\"source\":{{\"subagent\":{{\"thread_spawn\":{{\"parent_thread_id\":\"01900000-0000-7000-8000-000000000002\",\"depth\":1}}}}}}}}}}\n"
    );
    fs::write(&session_path, &raw).expect("subagent session should write");

    assert!(
        !repair_codex_session_metadata_prefix(&session_path, &raw)
            .expect("valid subagent metadata should be accepted")
    );
    assert_eq!(
        fs::read_to_string(&session_path).expect("subagent session should read"),
        raw
    );
    assert!(
        !session_path
            .with_extension("jsonl.prodex-repair-bak")
            .exists()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_parse_codex_jsonl_metadata() {
    let root = test_temp_dir("session-jsonl");
    let sessions = root.join("sessions/2026/04/29");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let cwd = root.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace should be created");
    fs::write(
        sessions.join("session-a.jsonl"),
        format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"sess-a\",\"thread_name\":\"Issue triage\",\"cwd\":\"{}\",\"model_provider\":\"prodex-gemini\"}}}}\n{{\"timestamp\":\"2026-04-29T12:30:00Z\",\"type\":\"event\"}}\n",
                cwd.display()
            ),
        )
        .expect("session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "sess-a");
    assert_eq!(reports[0].thread_name.as_deref(), Some("Issue triage"));
    assert_eq!(
        reports[0].cwd.as_deref(),
        Some(cwd.to_string_lossy().as_ref())
    );
    assert_eq!(
        reports[0].updated_at.as_deref(),
        Some("2026-04-29T12:30:00Z")
    );
    assert_eq!(reports[0].model_provider.as_deref(), Some("prodex-gemini"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_include_archived_sessions() {
    let root = test_temp_dir("session-archived-jsonl");
    let sessions = root.join("archived_sessions/2026/04/29");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("archived.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"archived-session\",\"cwd\":\"/tmp/workspace\",\"model_provider\":\"prodex-gemini\"}}\n",
    )
    .expect("archived session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "archived-session");
    assert_eq!(reports[0].model_provider.as_deref(), Some("prodex-gemini"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_skip_jsonl_without_resume_metadata_at_start() {
    let root = test_temp_dir("session-corrupt-jsonl");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("rollout-2026-06-13T02-04-31-019ebd01.jsonl"),
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial after restart\"}}\n{\"timestamp\":\"2026-06-13T02:04:32Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"019ebd01-c881-74c0-b01d-7fdf5bd4dd32\"}}\n",
    )
    .expect("corrupt session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");

    assert!(reports.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_keep_legacy_jsonl_when_first_line_has_id_without_type() {
    let root = test_temp_dir("session-legacy-jsonl");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("legacy.jsonl"),
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"payload\":{\"id\":\"legacy-session\",\"cwd\":\"/tmp/workspace\"}}\n{\"timestamp\":\"2026-06-13T02:05:00Z\",\"type\":\"event\"}\n",
    )
    .expect("legacy session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "legacy-session");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_moves_late_metadata_to_start() {
    let root = test_temp_dir("session-repair-prefix");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_path = sessions.join("rollout-2026-06-13T02-04-31-019ebd01.jsonl");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial after restart\"}}\n{\"timestamp\":\"2026-06-13T02:04:32Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"019ebd01-c881-74c0-b01d-7fdf5bd4dd32\",\"cwd\":\"/tmp/workspace\"}}\n{\"timestamp\":\"2026-06-13T02:04:33Z\",\"type\":\"event\",\"payload\":{\"message\":\"after metadata\"}}\n",
    )
    .expect("session should be written");

    let repaired =
        repair_resume_session_metadata_prefix(&root, "019ebd01-c881-74c0-b01d-7fdf5bd4dd32")
            .expect("session repair should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    assert!(
        repaired_raw
            .lines()
            .next()
            .expect("session should have first line")
            .contains(r#""type":"session_meta""#)
    );
    assert!(
        session_path
            .with_extension("jsonl.prodex-repair-bak")
            .is_file()
    );

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "019ebd01-c881-74c0-b01d-7fdf5bd4dd32");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_prefers_exact_rollout_path() {
    let root = test_temp_dir("session-repair-prefers-exact-path");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "019ebd01-c881-74c0-b01d-7fdf5bd4dd32";
    let target = sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    let unrelated = sessions.join("000-legacy.jsonl");
    fs::write(
        &target,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\"}\n",
    )
    .expect("target should be written");
    let unrelated_raw = format!(
        "{{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\"}}\n{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\"}}}}\n"
    );
    fs::write(&unrelated, &unrelated_raw).expect("unrelated session should be written");

    let repaired = repair_resume_session_metadata_prefix(&root, session_id)
        .expect("exact session repair should succeed");

    assert_eq!(repaired.as_deref(), Some(target.as_path()));
    assert_eq!(
        fs::read_to_string(&unrelated).expect("unrelated session should remain readable"),
        unrelated_raw
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_transaction_aborts_and_cleans_up_after_concurrent_append() {
    let root = test_temp_dir("session-repair-concurrent-append");
    let sessions = root.join("sessions/2026/07/16");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let path = sessions.join("concurrent.jsonl");
    let raw = "{\"timestamp\":\"2026-07-16T01:00:00Z\",\"type\":\"event\"}\n";
    let appended = "{\"timestamp\":\"2026-07-16T01:00:01Z\",\"type\":\"event\"}\n";
    fs::write(&path, raw).expect("session should be written");

    let transaction = repair_transaction::SessionRepairTransaction::begin(
        &root,
        &path,
        SESSION_STORE_FILE_MAX_BYTES,
    )
    .expect("repair transaction should start");
    let competing_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".prodex-session-repair.lock"))
        .expect("repository lock should open");
    assert!(matches!(
        competing_lock.try_lock(),
        Err(fs::TryLockError::WouldBlock)
    ));
    drop(competing_lock);
    let error = transaction
        .commit_after_prepare(b"{\"type\":\"session_meta\"}\n", || {
            std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        let mut source = fs::OpenOptions::new()
                            .append(true)
                            .open(&path)
                            .expect("session should open for append");
                        source
                            .write_all(appended.as_bytes())
                            .expect("concurrent append should succeed");
                        source.sync_all().expect("concurrent append should sync");
                    })
                    .join()
                    .expect("concurrent append thread should finish");
            });
        })
        .expect_err("stale repair must be rejected");

    assert!(format!("{error:#}").contains("changed during repair"));
    assert_eq!(
        fs::read_to_string(&path).expect("session should remain readable"),
        format!("{raw}{appended}")
    );
    assert!(!path.with_extension("jsonl.prodex-repair-bak").exists());
    assert_no_repair_temporary_files(&sessions);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_failure_keeps_source_and_leaves_no_temporary_file() {
    let root = test_temp_dir("session-repair-backup-failure");
    let sessions = root.join("sessions/2026/07/16");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "01900000-0000-7000-8000-000000000016";
    let path = sessions.join(format!("rollout-2026-07-16T01-00-00-{session_id}.jsonl"));
    let raw = "{\"timestamp\":\"2026-07-16T01:00:00Z\",\"type\":\"event\"}\n";
    fs::write(&path, raw).expect("session should be written");
    fs::create_dir(path.with_extension("jsonl.prodex-repair-bak"))
        .expect("conflicting backup directory should be created");

    let error = repair_resume_session_metadata_prefix(&root, session_id)
        .expect_err("invalid backup must stop repair");

    assert!(format!("{error:#}").contains("not a regular file"));
    assert_eq!(
        fs::read_to_string(&path).expect("source should remain readable"),
        raw
    );
    assert_no_repair_temporary_files(&sessions);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn repair_rejects_symlink_source_and_backup_without_touching_targets() {
    use std::os::unix::fs::symlink;

    let root = test_temp_dir("session-repair-symlinks");
    let sessions = root.join("sessions/2026/07/16");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let source_id = "01900000-0000-7000-8000-000000000017";
    let outside_source = root.join("outside-source.jsonl");
    let outside_source_raw = "{\"type\":\"event\",\"payload\":{\"safe\":true}}\n";
    fs::write(&outside_source, outside_source_raw).expect("outside source should be written");
    let source_link = sessions.join(format!("rollout-{source_id}.jsonl"));
    symlink(&outside_source, &source_link).expect("source symlink should be created");

    let source_error = repair_codex_session_metadata_prefix(&source_link, outside_source_raw)
        .expect_err("source symlink must be rejected");
    assert!(format!("{source_error:#}").contains("symlink"));
    assert_eq!(
        fs::read_to_string(&outside_source).expect("outside source should read"),
        outside_source_raw
    );

    let backup_id = "01900000-0000-7000-8000-000000000018";
    let path = sessions.join(format!("rollout-{backup_id}.jsonl"));
    let raw = "{\"timestamp\":\"2026-07-16T01:00:00Z\",\"type\":\"event\"}\n";
    fs::write(&path, raw).expect("session should be written");
    let outside_backup = root.join("outside-backup");
    fs::write(&outside_backup, "outside-safe").expect("outside backup should be written");
    symlink(
        &outside_backup,
        path.with_extension("jsonl.prodex-repair-bak"),
    )
    .expect("backup symlink should be created");

    let backup_error = repair_resume_session_metadata_prefix(&root, backup_id)
        .expect_err("backup symlink must be rejected");
    assert!(format!("{backup_error:#}").contains("symlink"));
    assert_eq!(
        fs::read_to_string(&outside_backup).expect("outside backup should read"),
        "outside-safe"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("source should remain readable"),
        raw
    );
    assert_no_repair_temporary_files(&sessions);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn repaired_source_backup_and_repository_lock_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let root = test_temp_dir("session-repair-private-mode");
    let sessions = root.join("sessions/2026/07/16");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "01900000-0000-7000-8000-000000000019";
    let path = sessions.join(format!("rollout-{session_id}.jsonl"));
    fs::write(
        &path,
        "{\"timestamp\":\"2026-07-16T01:00:00Z\",\"type\":\"event\"}\n",
    )
    .expect("session should be written");
    let mut permissions = fs::metadata(&path)
        .expect("source metadata should read")
        .permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&path, permissions).expect("source mode should update");

    assert!(
        repair_resume_session_metadata_prefix(&root, session_id)
            .expect("repair should succeed")
            .is_some()
    );

    for private_path in [
        path.clone(),
        path.with_extension("jsonl.prodex-repair-bak"),
        root.join(".prodex-session-repair.lock"),
    ] {
        assert_eq!(
            fs::metadata(&private_path)
                .expect("private file metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "{} should be private",
            private_path.display()
        );
    }
    assert_no_repair_temporary_files(&sessions);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_codex_session_metadata_prefix_rechecks_stale_caller_contents() {
    let root = test_temp_dir("session-repair-stale-caller-contents");
    let session_id = "01900000-0000-7000-8000-000000000020";
    let path = root.join(format!("rollout-{session_id}.jsonl"));
    let current = "{\"timestamp\":\"2026-07-16T01:00:00Z\",\"type\":\"event\"}\n";
    let stale = format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\"}}}}\n");
    fs::write(&path, current).expect("current session should be written");

    assert!(
        repair_codex_session_metadata_prefix(&path, &stale)
            .expect("current source should be repaired")
    );
    assert!(
        fs::read_to_string(&path)
            .expect("repaired session should read")
            .lines()
            .next()
            .is_some_and(session_meta::line_starts_codex_rollout_metadata)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_synthesizes_missing_rollout_metadata() {
    let root = test_temp_dir("session-synthetic-rollout");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_path =
        sessions.join("rollout-2026-06-13T02-04-31-019ebd01-c881-74c0-b01d-7fdf5bd4dd32.jsonl");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial only\"}}\n",
    )
    .expect("session should be written");

    let repaired =
        repair_resume_session_metadata_prefix(&root, "019ebd01-c881-74c0-b01d-7fdf5bd4dd32")
            .expect("repair check should succeed");
    let unrepairable =
        find_unrepairable_resume_session(&root, "019ebd01-c881-74c0-b01d-7fdf5bd4dd32")
            .expect("unrepairable check should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    assert_eq!(unrepairable, None);
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    assert_codex_session_meta_line(
        repaired_raw
            .lines()
            .next()
            .expect("session should have first line"),
        "019ebd01-c881-74c0-b01d-7fdf5bd4dd32",
        Some("2026-06-13T02:04:31Z"),
    );
    assert!(
        session_path
            .with_extension("jsonl.prodex-repair-bak")
            .is_file()
    );

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "019ebd01-c881-74c0-b01d-7fdf5bd4dd32");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_synthesizes_archived_metadata() {
    let root = test_temp_dir("session-synthetic-archived-rollout");
    let sessions = root.join("archived_sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "019ebd01-c881-74c0-b01d-7fdf5bd4dd32";
    let session_path = sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"archived partial\"}}\n",
    )
    .expect("archived session should be written");

    repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");
    let unrepairable =
        find_unrepairable_resume_session(&root, session_id).expect("unrepairable check");

    assert_eq!(unrepairable, None);
    assert_codex_session_meta_line(
        fs::read_to_string(&session_path)
            .expect("session should read")
            .lines()
            .next()
            .expect("session should have first line"),
        session_id,
        Some("2026-06-13T02:04:31Z"),
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_recovers_readable_chat_after_corrupt_lines() {
    let root = test_temp_dir("session-repair-readable-chat");
    let sessions = root.join("sessions/2026/06/14");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let session_path = sessions.join(format!("rollout-2026-06-14T23-32-19-{session_id}.jsonl"));
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"event\",\"payload\":{\"message\":\"first readable chat\"}}\n{not-json\n{\"timestamp\":\"2026-06-14T23:32:20Z\",\"type\":\"event\",\"payload\":{\"message\":\"second readable chat\"}}\n",
    )
    .expect("session should be written");

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    let lines = repaired_raw.lines().collect::<Vec<_>>();
    assert_codex_session_meta_line(
        lines
            .first()
            .copied()
            .expect("session should have first line"),
        "019ec6c3-28a4-79f0-91f9-74a2f34b0928",
        Some("2026-06-14T23:32:19Z"),
    );
    assert_eq!(lines.len(), 3);
    assert!(lines[1].contains("first readable chat"));
    assert!(lines[2].contains("second readable chat"));
    assert!(!repaired_raw.contains("{not-json"));

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, session_id);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_upgrades_minimal_repair_metadata() {
    let root = test_temp_dir("session-repair-upgrade-minimal-metadata");
    let sessions = root.join("sessions/2026/06/14");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let session_path = sessions.join(format!("rollout-2026-06-14T23-32-19-{session_id}.jsonl"));
    fs::write(
        &session_path,
        format!(
            "{{\"payload\":{{\"id\":\"{session_id}\"}},\"type\":\"session_meta\"}}\n{{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"event\",\"payload\":{{\"message\":\"first readable chat\"}}}}\n"
        ),
    )
    .expect("session should be written");

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");
    let unrepairable =
        find_unrepairable_resume_session(&root, session_id).expect("unrepairable check");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    assert_eq!(unrepairable, None);
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    let lines = repaired_raw.lines().collect::<Vec<_>>();
    assert_codex_session_meta_line(
        lines
            .first()
            .copied()
            .expect("session should have first line"),
        session_id,
        Some("2026-06-14T23:32:19Z"),
    );
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("first readable chat"));

    let _ = fs::remove_dir_all(root);
}
