use super::*;

#[test]
fn repair_resume_session_metadata_prefix_ignores_unrelated_state_database_schema() {
    let root = test_temp_dir("session-repair-unrelated-state-schema");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let sessions = root.join("sessions/2026/06/14");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_path = sessions.join(format!("rollout-2026-06-14-{session_id}.jsonl"));
    fs::write(
        &session_path,
        "{\"type\":\"event\",\"payload\":{\"message\":\"existing session\"}}\n",
    )
    .expect("session should be written");
    let connection = rusqlite::Connection::open(root.join("state_4.sqlite"))
        .expect("unrelated state db should open");
    connection
        .execute("CREATE TABLE unrelated (id TEXT PRIMARY KEY)", [])
        .expect("unrelated table should be created");
    drop(connection);

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");

    assert_eq!(repaired.as_deref(), Some(session_path.as_path()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repair_resume_session_metadata_prefix_repoints_stale_overlay_rollout_path() {
    let root = test_temp_dir("session-repair-state-db-stale-overlay");
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let sessions = root.join("sessions/2026/06/14");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let session_path = sessions.join(format!("rollout-2026-06-14T23-32-19-{session_id}.jsonl"));
    fs::write(
        &session_path,
        format!(
            "{{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"timestamp\":\"2026-06-14T23:32:19Z\",\"cwd\":\"/home/test-user/project\",\"originator\":\"codex-cli\",\"cli_version\":\"0.145.0\"}}}}\n"
        ),
    )
    .expect("session should be written");
    let stale_path = root
        .with_file_name(".prodex-overlay-12345-0")
        .join(session_path.strip_prefix(&root).unwrap());
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
            rusqlite::params![session_id, stale_path.display().to_string()],
        )
        .expect("thread row should be created");
    drop(connection);

    let repaired =
        repair_resume_session_metadata_prefix(&root, session_id).expect("repair should succeed");
    let connection = rusqlite::Connection::open(&db_path).expect("state db should reopen");
    let rollout_path: String = connection
        .query_row(
            "SELECT rollout_path FROM threads WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .expect("rollout path should read");

    assert_eq!(repaired, None);
    assert_eq!(std::path::Path::new(&rollout_path), session_path.as_path());
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
fn repair_resume_session_metadata_prefix_ignores_rollout_path_through_symlink_directory() {
    let root = test_temp_dir("session-repair-state-db-symlink-parent");
    let outside = root.with_extension("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let outside_session = outside.join("rollout.jsonl");
    fs::write(&outside_session, "{\"type\":\"event\"}\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-sessions")).unwrap();
    let rollout_path = root.join("linked-sessions/rollout.jsonl");
    let connection = rusqlite::Connection::open(root.join("state_5.sqlite")).unwrap();
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, rollout_path.display().to_string()],
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        repair_resume_session_metadata_prefix(&root, session_id).unwrap(),
        None
    );
    assert!(
        !outside_session
            .with_extension("jsonl.prodex-repair-bak")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
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
