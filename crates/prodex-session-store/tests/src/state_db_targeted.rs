use super::*;

#[test]
fn targeted_stale_overlay_repair_finds_promoted_rollout_at_different_path() {
    let root = test_temp_dir("session-repair-stale-overlay-targeted");
    let session_id = "01a01824-29f7-7332-96c7-5d09044ee0d3";
    let persistent_path = root
        .join("sessions/2026/08/20")
        .join(format!("promoted-{session_id}.jsonl"));
    fs::create_dir_all(
        persistent_path
            .parent()
            .expect("session parent should exist"),
    )
    .expect("persistent session directory should be created");
    fs::write(
        &persistent_path,
        format!(
            "{{\"timestamp\":\"2026-08-20T10:50:18Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"timestamp\":\"2026-08-20T10:50:18Z\",\"cwd\":\"/home/test-user/project\",\"originator\":\"codex-cli\",\"cli_version\":\"0.148.0\"}}}}\n"
        ),
    )
    .expect("persistent rollout should be written");
    let stale_path = root
        .with_file_name(".prodex-overlay-OLD")
        .join("sessions/2026/08/19/old-name.jsonl");
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
        .expect("stale thread row should be created");
    drop(connection);

    assert_eq!(
        repair_stale_overlay_rollout_path_for_session(&root, session_id)
            .expect("targeted repair should succeed"),
        1
    );
    let connection = rusqlite::Connection::open(&db_path).expect("state db should reopen");
    let rollout_path: String = connection
        .query_row(
            "SELECT rollout_path FROM threads WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .expect("repaired rollout path should read");
    assert_eq!(Path::new(&rollout_path), persistent_path.as_path());
    assert_eq!(
        repair_stale_overlay_rollout_path_for_session(&root, session_id)
            .expect("second targeted repair should succeed"),
        0
    );
    let _ = fs::remove_dir_all(root);
}
