use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn assert_codex_session_meta_line(
    line: &str,
    session_id: &str,
    expected_timestamp: Option<&str>,
) -> serde_json::Value {
    let value: serde_json::Value =
        serde_json::from_str(line).expect("synthetic metadata should be valid JSON");
    assert_eq!(value["type"], "session_meta");
    assert_eq!(value["payload"]["id"], session_id);
    let outer_timestamp = value["timestamp"]
        .as_str()
        .expect("synthetic metadata should include rollout timestamp");
    let payload_timestamp = value["payload"]["timestamp"]
        .as_str()
        .expect("synthetic metadata should include session timestamp");
    assert_eq!(outer_timestamp, payload_timestamp);
    if let Some(expected_timestamp) = expected_timestamp {
        assert_eq!(outer_timestamp, expected_timestamp);
    }
    assert_eq!(value["payload"]["originator"], "prodex-repair");
    assert_eq!(value["payload"]["cli_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["payload"]["source"], "cli");
    let cwd = value["payload"]["cwd"]
        .as_str()
        .expect("synthetic metadata should include cwd");
    let _: PathBuf = serde_json::from_value(value["payload"]["cwd"].clone())
        .expect("synthetic cwd should deserialize as a Codex PathBuf");
    assert!(!cwd.is_empty());
    value
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
fn session_reports_reject_oversized_session_file_before_reading() {
    let root = test_temp_dir("session-oversized-jsonl");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let path = sessions.join("oversized.jsonl");
    fs::File::create(&path)
        .expect("oversized session should be created")
        .set_len(SESSION_STORE_FILE_MAX_BYTES + 1)
        .expect("oversized session size should be set");

    let err = collect_session_reports(&root, None, &AppState::default())
        .expect_err("oversized session should be rejected");
    assert!(format!("{err:#}").contains("exceeds safe size limit"));

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

#[test]
fn repair_resume_session_metadata_prefix_uses_state_db_rollout_path() {
    let root = test_temp_dir("session-repair-state-db-rollout");
    fs::create_dir_all(&root).expect("root should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let rollout_dir = root.join("state-db-rollouts");
    fs::create_dir_all(&rollout_dir).expect("rollout dir should be created");
    let session_path = rollout_dir.join("rollout-from-state-db.jsonl");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"event\",\"payload\":{\"message\":\"chat from state db path\"}}\n",
    )
    .expect("session should be written");
    let db_path = root.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&db_path).expect("state db should open");
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .expect("threads table should be created");
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, session_path.display().to_string()],
        )
        .expect("thread row should be created");
    drop(connection);

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    assert_codex_session_meta_line(
        repaired_raw
            .lines()
            .next()
            .expect("session should have first line"),
        "019ec6c3-28a4-79f0-91f9-74a2f34b0928",
        Some("2026-06-14T23:32:19Z"),
    );
    assert!(
        session_path
            .with_extension("jsonl.prodex-repair-bak")
            .is_file()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_uses_state_db_prefix_rollout_path() {
    let root = test_temp_dir("session-repair-state-db-prefix-rollout");
    fs::create_dir_all(&root).expect("root should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let rollout_dir = root.join("state-db-rollouts");
    fs::create_dir_all(&rollout_dir).expect("rollout dir should be created");
    let session_path = rollout_dir.join("rollout-from-state-db.jsonl");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"event\",\"payload\":{\"message\":\"chat from state db prefix path\"}}\n",
    )
    .expect("session should be written");
    let db_path = root.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&db_path).expect("state db should open");
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .expect("threads table should be created");
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![
                format!("thread_{session_id}"),
                session_path.display().to_string()
            ],
        )
        .expect("thread row should be created");
    drop(connection);

    let repaired =
        repair_resume_session_metadata_prefix(&root, "019ec6c3").expect("repair should succeed");
    let unrepairable = find_unrepairable_resume_session(&root, "019ec6c3")
        .expect("unrepairable check should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    assert_eq!(unrepairable, None);
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    assert_codex_session_meta_line(
        repaired_raw
            .lines()
            .next()
            .expect("session should have first line"),
        "019ec6c3-28a4-79f0-91f9-74a2f34b0928",
        Some("2026-06-14T23:32:19Z"),
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_ignores_state_db_rollout_path_outside_root() {
    let root = test_temp_dir("session-repair-state-db-outside-root");
    fs::create_dir_all(&root).expect("root should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let outside_root = std::env::current_dir()
        .expect("current dir should exist")
        .join("target")
        .join(format!(
            "prodex-session-store-outside-root-{}-{unique}",
            std::process::id()
        ));
    let session_path = outside_root.join("rollout-from-state-db.jsonl");
    fs::create_dir_all(&outside_root).expect("outside dir should be created");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"event\",\"payload\":{\"message\":\"outside root\"}}\n",
    )
    .expect("session should be written");
    let db_path = root.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&db_path).expect("state db should open");
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .expect("threads table should be created");
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, session_path.display().to_string()],
        )
        .expect("thread row should be created");
    drop(connection);

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");
    let unrepairable =
        find_unrepairable_resume_session(&root, session_id).expect("check should succeed");

    assert_eq!(repaired, None);
    assert_eq!(unrepairable, None);
    assert!(
        !session_path
            .with_extension("jsonl.prodex-repair-bak")
            .exists(),
        "outside-root rollout path should not be modified"
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside_root);
}

#[cfg(unix)]
#[test]
fn repair_resume_session_metadata_prefix_ignores_symlink_state_db() {
    let root = test_temp_dir("session-repair-symlink-state-db");
    fs::create_dir_all(&root).expect("root should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let sessions = root.join("sessions/2026/07/02");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_path = sessions.join("rollout-hidden-from-state-db.jsonl");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-07-02T01:00:00Z\",\"type\":\"event\",\"payload\":{\"message\":\"must not repair from symlink db\"}}\n",
    )
    .expect("session should be written");
    let outside = root.join("outside-db");
    fs::create_dir_all(&outside).expect("outside db dir should be created");
    let outside_db = outside.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&outside_db).expect("state db should open");
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .expect("threads table should be created");
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, session_path.display().to_string()],
        )
        .expect("thread row should be created");
    drop(connection);
    std::os::unix::fs::symlink(&outside_db, root.join("state_5.sqlite"))
        .expect("state db symlink should be created");

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");

    assert_eq!(repaired, None);
    assert!(
        !session_path
            .with_extension("jsonl.prodex-repair-bak")
            .exists(),
        "symlinked state db must not drive session repair"
    );
    let raw = fs::read_to_string(&session_path).expect("session should remain readable");
    assert!(
        raw.lines()
            .next()
            .expect("session should have first line")
            .contains(r#""type":"event""#)
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn repair_resume_session_metadata_prefix_skips_unreadable_unrelated_files_for_exact_id() {
    use std::os::unix::fs::PermissionsExt;

    let root = test_temp_dir("session-repair-skip-unreadable-unrelated");
    let sessions = root.join("sessions/2026/07/01");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let target = sessions.join(format!("rollout-2026-07-01T20-00-00-{session_id}.jsonl"));
    let unrelated = sessions.join("rollout-2026-07-01T20-00-00-other-session.jsonl");
    fs::write(
        &target,
        "{\"timestamp\":\"2026-07-01T20:00:00Z\",\"type\":\"event\",\"payload\":{\"message\":\"target session\"}}\n",
    )
    .expect("target session should be written");
    fs::write(
        &unrelated,
        "{\"timestamp\":\"2026-07-01T20:00:00Z\",\"type\":\"event\"}\n",
    )
    .expect("unrelated session should be written");
    let mut perms = fs::metadata(&unrelated)
        .expect("unrelated metadata should read")
        .permissions();
    perms.set_mode(0o0);
    fs::set_permissions(&unrelated, perms).expect("unrelated permissions should update");

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");

    assert_eq!(repaired.as_deref(), Some(target.as_path()));
    assert_codex_session_meta_line(
        fs::read_to_string(&target)
            .expect("target session should read")
            .lines()
            .next()
            .expect("target first line should exist"),
        session_id,
        Some("2026-07-01T20:00:00Z"),
    );

    let mut perms = fs::metadata(&unrelated)
        .expect("unrelated metadata should still read")
        .permissions();
    perms.set_mode(0o644);
    let _ = fs::set_permissions(&unrelated, perms);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn resolve_session_report_by_id_in_store_skips_unreadable_unrelated_files_for_exact_id() {
    use std::os::unix::fs::PermissionsExt;

    let root = test_temp_dir("resolve-session-report-skip-unreadable-unrelated");
    let sessions = root.join("sessions/2026/07/01");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let target = sessions.join(format!("rollout-2026-07-01T20-00-00-{session_id}.jsonl"));
    let unrelated = sessions.join("rollout-2026-07-01T20-00-00-other-session.jsonl");
    fs::write(
        &target,
        format!(
            "{{\"timestamp\":\"2026-07-01T20:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"/tmp/target\"}}}}\n"
        ),
    )
    .expect("target session should be written");
    fs::write(
        &unrelated,
        "{\"timestamp\":\"2026-07-01T20:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"other-session\",\"cwd\":\"/tmp/unrelated\"}}\n",
    )
    .expect("unrelated session should be written");
    let mut perms = fs::metadata(&unrelated)
        .expect("unrelated metadata should read")
        .permissions();
    perms.set_mode(0o0);
    fs::set_permissions(&unrelated, perms).expect("unrelated permissions should update");

    let report = resolve_session_report_by_id_in_store(&root, &AppState::default(), session_id)
        .expect("resolve should succeed");

    assert_eq!(report.id, session_id);
    assert_eq!(report.cwd.as_deref(), Some("/tmp/target"));

    let mut perms = fs::metadata(&unrelated)
        .expect("unrelated metadata should still read")
        .permissions();
    perms.set_mode(0o644);
    let _ = fs::set_permissions(&unrelated, perms);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_synthesizes_unique_prefix_match() {
    let root = test_temp_dir("session-synthetic-prefix-rollout");
    let sessions = root.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_path =
        sessions.join("rollout-2026-06-13T02-04-31-019ebd01-c881-74c0-b01d-7fdf5bd4dd32.jsonl");
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"cwd\":\"/tmp/workspace\",\"message\":\"legacy session\"}}\n",
    )
    .expect("session should be written");

    let repaired =
        repair_resume_session_metadata_prefix(&root, "019ebd01").expect("repair should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    let repaired_raw = fs::read_to_string(&session_path).expect("session should be readable");
    let meta = assert_codex_session_meta_line(
        repaired_raw
            .lines()
            .next()
            .expect("session should have first line"),
        "019ebd01-c881-74c0-b01d-7fdf5bd4dd32",
        Some("2026-06-13T02:04:31Z"),
    );
    assert_eq!(meta["payload"]["cwd"], "/tmp/workspace");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_does_not_repair_ambiguous_prefix() {
    let root = test_temp_dir("session-repair-ambiguous-prefix");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let first = sessions.join("first.jsonl");
    let second = sessions.join("second.jsonl");
    fs::write(
        &first,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\"}\n{\"timestamp\":\"2026-06-13T02:04:32Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"019ebd01-c881-74c0-b01d-7fdf5bd4dd32\"}}\n",
    )
    .expect("first session should be written");
    fs::write(
        &second,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\"}\n{\"timestamp\":\"2026-06-13T02:04:32Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"019ebd01-c881-74c0-b01d-7fdf5bd4dd33\"}}\n",
    )
    .expect("second session should be written");

    let repaired =
        repair_resume_session_metadata_prefix(&root, "019ebd01").expect("repair should succeed");

    assert_eq!(repaired, None);
    assert!(
        fs::read_to_string(first)
            .expect("first should be readable")
            .lines()
            .next()
            .expect("first line should exist")
            .contains(r#""type":"event""#)
    );
    assert!(
        fs::read_to_string(second)
            .expect("second should be readable")
            .lines()
            .next()
            .expect("first line should exist")
            .contains(r#""type":"event""#)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_current_filters_by_cwd() {
    let root = test_temp_dir("session-current");
    let sessions = root.join("sessions");
    let current = root.join("current");
    let other = root.join("other");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::create_dir_all(&current).expect("current dir should be created");
    fs::create_dir_all(&other).expect("other dir should be created");
    fs::write(
            sessions.join("current.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"current\",\"cwd\":\"{}\"}}}}\n",
                current.display()
            ),
        )
        .expect("current session should be written");
    fs::write(
            sessions.join("other.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"other\",\"cwd\":\"{}\"}}}}\n",
                other.display()
            ),
        )
        .expect("other session should be written");

    let reports = collect_session_reports(&root, Some(&current), &AppState::default())
        .expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "current");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_exclude_subagents_by_default() {
    let root = test_temp_dir("session-subagents");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("parent.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"parent\",\"cwd\":\"/tmp/workspace\"}}\n",
    )
    .expect("parent session should be written");
    fs::write(
        sessions.join("child.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:01:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"child\",\"cwd\":\"/tmp/workspace\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"parent\"}}}}}\n",
    )
    .expect("child session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "parent");

    let reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            include_subagents: true,
            ..SessionReportFilter::default()
        },
        &AppState::default(),
    )
    .expect("sessions collect");
    assert_eq!(reports.len(), 2);
    assert!(reports.iter().any(|report| report.id == "child"));
    assert_eq!(
        reports
            .iter()
            .find(|report| report.id == "child")
            .and_then(|report| report.parent_thread_id.as_deref()),
        Some("parent")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_attach_profile_bindings() {
    let root = test_temp_dir("session-profile");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("bound.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{\"id\":\"bound\"}}\n",
    )
    .expect("session should be written");
    let state = AppState {
        session_profile_bindings: BTreeMap::from([(
            "bound".to_string(),
            prodex_state::ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: 1,
            },
        )]),
        ..AppState::default()
    };

    let reports = collect_session_reports(&root, None, &state).expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].profile.as_deref(), Some("main"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_filter_by_profile_and_query() {
    let root = test_temp_dir("session-filter");
    let sessions = root.join("sessions");
    let alpha_cwd = root.join("WorkspaceAlpha");
    let beta_cwd = root.join("WorkspaceBeta");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::create_dir_all(&alpha_cwd).expect("alpha cwd should be created");
    fs::create_dir_all(&beta_cwd).expect("beta cwd should be created");
    fs::write(
        sessions.join("alpha-special-path.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"alpha-session\",\"thread_name\":\"Issue Triage\",\"cwd\":\"{}\"}}}}\n",
            alpha_cwd.display()
        ),
    )
    .expect("alpha session should be written");
    fs::write(
        sessions.join("beta-special-path.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"beta-session\",\"thread_name\":\"Docs Review\",\"cwd\":\"{}\"}}}}\n",
            beta_cwd.display()
        ),
    )
    .expect("beta session should be written");
    let state = AppState {
        session_profile_bindings: BTreeMap::from([
            (
                "alpha-session".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1,
                },
            ),
            (
                "beta-session".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "alt".to_string(),
                    bound_at: 1,
                },
            ),
        ]),
        ..AppState::default()
    };

    let profile_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: Some("main"),
            query: Some("triage"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(profile_reports.len(), 1);
    assert_eq!(profile_reports[0].id, "alpha-session");

    let cwd_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: None,
            query: Some("workspacebeta"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(cwd_reports.len(), 1);
    assert_eq!(cwd_reports[0].id, "beta-session");

    let path_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: None,
            query: Some("ALPHA-SPECIAL-PATH"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(path_reports.len(), 1);
    assert_eq!(path_reports[0].id, "alpha-session");

    let profile_query_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: None,
            query: Some("ALT"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(profile_query_reports.len(), 1);
    assert_eq!(profile_query_reports[0].id, "beta-session");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_resolver_handles_unique_ambiguous_and_missing_ids() {
    let root = test_temp_dir("session-resolve");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let first_id = "11111111-1111-1111-1111-111111111111";
    let second_id = "11111111-1111-1111-1111-222222222222";
    fs::write(
        sessions.join("first.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"{first_id}\"}}}}\n"
        ),
    )
    .expect("first session should be written");
    fs::write(
        sessions.join("second.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"{second_id}\"}}}}\n"
        ),
    )
    .expect("second session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(
        resolve_session_report_by_id(&reports, &first_id.to_uppercase())
            .expect("exact match should resolve")
            .id,
        first_id
    );
    assert_eq!(
        resolve_session_report_by_id(&reports, "11111111-1111-1111-1111-222")
            .expect("unique prefix should resolve")
            .id,
        second_id
    );
    assert!(matches!(
        resolve_session_report_by_id(&reports, "11111111").unwrap_err(),
        SessionResolveError::Ambiguous { .. }
    ));
    assert!(matches!(
        resolve_session_report_by_id(&reports, "99999999").unwrap_err(),
        SessionResolveError::Missing { .. }
    ));

    let _ = fs::remove_dir_all(root);
}

fn test_temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-session-store-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp dir should be removed");
    }
    fs::create_dir_all(&root).expect("temp dir should be created");
    root
}
