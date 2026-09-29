use super::{
    ChildProcessPlan, LatestThreadIndexState, ThreadIndexRepairScope, latest_thread_index_state,
    reconcile_codex_thread_index_protocol, reconcile_codex_thread_index_protocol_with_scope,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn reconciliation_scans_all_active_and_archived_pages() {
    let responses = concat!(
        "{\"id\":1,\"result\":{}}\n",
        "{\"method\":\"remoteControl/status/changed\",\"params\":{}}\n",
        "{\"id\":2,\"result\":{\"data\":[],\"nextCursor\":\"active-next\"}}\n",
        "{\"id\":3,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
        "{\"id\":4,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
    );
    let mut reader = std::io::Cursor::new(responses.as_bytes());
    let mut written = Vec::new();

    reconcile_codex_thread_index_protocol(&mut reader, &mut written).unwrap();

    let requests = String::from_utf8(written)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[0]["method"], "initialize");
    assert_eq!(requests[1]["method"], "initialized");
    assert_eq!(requests[2]["method"], "thread/list");
    assert_eq!(requests[2]["params"]["archived"], false);
    assert_eq!(requests[2]["params"]["cursor"], serde_json::Value::Null);
    assert_eq!(requests[2]["params"]["useStateDbOnly"], false);
    assert_eq!(
        requests[2]["params"]["modelProviders"],
        serde_json::json!([])
    );
    assert_eq!(requests[3]["params"]["cursor"], "active-next");
    assert_eq!(requests[4]["params"]["archived"], true);
}

#[test]
fn reconciliation_rejects_repeated_cursor() {
    let responses = concat!(
        "{\"id\":1,\"result\":{}}\n",
        "{\"id\":2,\"result\":{\"nextCursor\":\"same\"}}\n",
        "{\"id\":3,\"result\":{\"nextCursor\":\"same\"}}\n",
    );
    let mut reader = std::io::Cursor::new(responses.as_bytes());
    let mut written = Vec::new();

    let error = reconcile_codex_thread_index_protocol(&mut reader, &mut written).unwrap_err();

    assert!(error.to_string().contains("repeated"));
}

#[test]
fn targeted_reconciliation_requests_only_the_newest_active_page() {
    let responses = concat!(
        "{\"id\":1,\"result\":{}}\n",
        "{\"id\":2,\"result\":{\"data\":[],\"nextCursor\":\"ignored\"}}\n",
    );
    let mut reader = std::io::Cursor::new(responses.as_bytes());
    let mut written = Vec::new();

    reconcile_codex_thread_index_protocol_with_scope(
        &mut reader,
        &mut written,
        ThreadIndexRepairScope::Latest,
    )
    .unwrap();

    let requests = String::from_utf8(written)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2]["params"]["archived"], false);
    assert_eq!(requests[2]["params"]["limit"], 1);
    assert_eq!(requests[2]["params"]["sortKey"], "updated_at");
}

#[test]
fn latest_thread_index_state_detects_missing_and_present_rows() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-thread-index-state-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("sessions")).unwrap();
    fs::create_dir_all(root.join("overlay")).unwrap();
    let session_id = "01900000-0000-7000-8000-000000000005";
    let session_file = root
        .join("sessions")
        .join(format!("rollout-{session_id}.jsonl.zst"));
    fs::write(
        &session_file,
        zstd::stream::encode_all(&b"session"[..], 3).unwrap(),
    )
    .unwrap();
    let database = root.join("state_test.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .unwrap();
    drop(connection);

    let mut child = ChildProcessPlan::new("codex".into(), root.join("overlay"));
    child
        .extra_env
        .push(("CODEX_SQLITE_HOME".into(), root.clone().into_os_string()));
    assert_eq!(
        latest_thread_index_state(&child, &session_file),
        LatestThreadIndexState::Missing
    );
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, "/stale/rollout.jsonl"],
        )
        .unwrap();
    assert_eq!(
        latest_thread_index_state(&child, &session_file),
        LatestThreadIndexState::Stale
    );
    connection
        .execute(
            "UPDATE threads SET rollout_path = ?1 WHERE id = ?2",
            rusqlite::params![
                "sessions/rollout-01900000-0000-7000-8000-000000000005.jsonl",
                session_id
            ],
        )
        .unwrap();
    assert_eq!(
        latest_thread_index_state(&child, &session_file),
        LatestThreadIndexState::Present
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn latest_thread_index_state_rejects_overlay_path_even_when_canonical_target_matches() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-thread-index-overlay-{}-{stamp}",
        std::process::id()
    ));
    let session_id = "01900000-0000-7000-8000-000000000006";
    let session_file = root
        .join("sessions/2026/08/19")
        .join(format!("rollout-{session_id}.jsonl"));
    let overlay_file = root
        .join(".prodex-overlay-old/sessions/2026/08/19")
        .join(format!("rollout-{session_id}.jsonl"));
    fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    fs::create_dir_all(overlay_file.parent().unwrap()).unwrap();
    fs::write(&session_file, b"session").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&session_file, &overlay_file).unwrap();
    #[cfg(windows)]
    fs::copy(&session_file, &overlay_file).unwrap();

    let database = root.join("state_overlay.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, overlay_file.display().to_string()],
        )
        .unwrap();
    drop(connection);

    let mut child = ChildProcessPlan::new("codex".into(), root.join("overlay-home"));
    child
        .extra_env
        .push(("CODEX_SQLITE_HOME".into(), root.clone().into_os_string()));
    assert_eq!(
        latest_thread_index_state(&child, &session_file),
        LatestThreadIndexState::Stale
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn targeted_reconciliation_kills_a_hanging_app_server() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Instant;

    let root = std::env::temp_dir().join(format!(
        "prodex-thread-index-hang-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let script = root.join("fake-codex.sh");
    fs::write(
        &script,
        "#!/bin/sh\nread line\nprintf '%s\\n' '{\"id\":1,\"result\":{}}'\nread line\nsleep 30\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&script, permissions).unwrap();
    let child = ChildProcessPlan::new(script.into_os_string(), root.clone());
    let started = Instant::now();
    let result = super::reconcile_latest_codex_thread_index(&child.binary, &child);
    assert!(result.is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn thread_state_lookup_recovers_workspace_without_rollout_file() {
    let root = std::env::temp_dir().join(format!(
        "prodex-thread-state-workspace-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let session_id = "01900000-0000-7000-8000-000000000071";
    let database = root.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute("CREATE TABLE threads (id TEXT PRIMARY KEY, cwd TEXT)", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO threads (id, cwd) VALUES (?1, ?2)",
            rusqlite::params![session_id, workspace.display().to_string()],
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        super::runtime_thread_workspace_for_session(&root, session_id).as_deref(),
        Some(workspace.as_path())
    );
    assert_eq!(
        super::runtime_thread_workspace_for_session(&root, "01900000-0000-7000-8000-000000000072"),
        None
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn thread_state_lookup_remembers_latest_model_per_provider() {
    let root = std::env::temp_dir().join(format!(
        "prodex-thread-state-model-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let database = root.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                model_provider TEXT,
                model TEXT,
                reasoning_effort TEXT,
                updated_at_ms INTEGER
            )",
            [],
        )
        .unwrap();
    for row in [
        (
            "openai-old",
            "prodex-openai-governed-http",
            "gpt-5.6-sol",
            Some("high"),
            10_i64,
        ),
        (
            "gemini-new",
            "prodex-gemini",
            "gemini-2.5-pro",
            Some("medium"),
            30_i64,
        ),
        (
            "openai-new",
            "prodex-openai-governed-http",
            "gpt-6-luna",
            Some("max"),
            20_i64,
        ),
    ] {
        connection
            .execute(
                "INSERT INTO threads
                 (id, model_provider, model, reasoning_effort, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![row.0, row.1, row.2, row.3, row.4],
            )
            .unwrap();
    }
    drop(connection);

    assert_eq!(
        super::latest_runtime_thread_model_selection(
            &root,
            prodex_provider_core::ProviderId::OpenAi
        ),
        Some(("gpt-6-luna".to_string(), Some("max".to_string())))
    );
    assert_eq!(
        super::latest_runtime_thread_model_selection(
            &root,
            prodex_provider_core::ProviderId::Gemini
        ),
        Some(("gemini-2.5-pro".to_string(), Some("medium".to_string())))
    );
    let _ = fs::remove_dir_all(root);
}
