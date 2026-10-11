use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "prodex-reconnect-attachments-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reconnect_native_session_meta_repairs_only_its_goal_attachment_alias() {
    const SESSION: &str = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let fixture = Fixture::new();
    let shared = fixture.0.join("shared");
    let paths = AppPaths {
        root: fixture.0.join("prodex"),
        state_file: fixture.0.join("prodex/state.json"),
        managed_profiles_root: fixture.0.join("prodex/profiles"),
        shared_codex_root: shared.clone(),
        legacy_shared_codex_root: fixture.0.join("legacy"),
    };
    fs::create_dir_all(&paths.root).unwrap();
    let stable = shared.join("attachments/453873cd-8ea3-4c85-832a-30728dd0954f/pasted-text-1.txt");
    fs::create_dir_all(stable.parent().unwrap()).unwrap();
    fs::write(&stable, "Keep the original task; do not replay tools.").unwrap();
    let stale = paths.managed_profiles_root.join(
        ".prodex-overlay-old-0/attachments/453873cd-8ea3-4c85-832a-30728dd0954f/pasted-text-1.txt",
    );
    let objective = format!(
        "Read the pasted text file at {} before continuing.",
        stale.display()
    );
    let db = Connection::open(shared.join("goals_1.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, objective TEXT NOT NULL, status TEXT NOT NULL);").unwrap();
    for id in [SESSION, "foreign-session"] {
        db.execute(
            "INSERT INTO thread_goals VALUES (?1,?2,'blocked')",
            params![id, objective],
        )
        .unwrap();
    }
    let session = shared.join(format!(
        "sessions/2026/10/11/rollout-2026-10-11T18-49-19-{SESSION}.jsonl"
    ));
    fs::create_dir_all(session.parent().unwrap()).unwrap();
    let content = [
        serde_json::json!({"timestamp":"2026-10-11T11:49:19Z","type":"session_meta","payload":{"id":SESSION,"cwd":"/synthetic"}}),
        serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":objective}]}}),
    ].iter().map(ToString::to_string).collect::<Vec<_>>().join("\n") + "\n";
    fs::write(&session, content).unwrap();
    for pass in 0..3 {
        if pass == 1 {
            // Recreate the old on-disk cache: unchanged rollout, no thread ID.
            // An upgrade must not reuse that cache and silently skip the goal.
            db.execute(
                "UPDATE thread_goals SET objective=?1 WHERE thread_id=?2",
                params![objective, SESSION],
            )
            .unwrap();
            let cache_path = paths.root.join("shared-codex-session-recent-v1.json");
            let mut cache: serde_json::Value =
                serde_json::from_slice(&fs::read(&cache_path).unwrap()).unwrap();
            cache["version"] = serde_json::json!(2);
            cache["thread_id"] = serde_json::Value::Null;
            fs::write(cache_path, serde_json::to_vec(&cache).unwrap()).unwrap();
        }
        // Initial repair, old-cache invalidation, then corrected-cache reuse.
        super::super::maintain_managed_codex_session_file(&paths, &session).unwrap();
        let (rewritten, status): (String, String) = db
            .query_row(
                "SELECT objective,status FROM thread_goals WHERE thread_id=?1",
                [SESSION],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(
            rewritten.contains(&stable.display().to_string()),
            "native session_meta.payload.id must select the goal and preserve its attachment: {rewritten}"
        );
        assert!(!rewritten.contains(".prodex-overlay-old-0"));
        assert_eq!(
            status, "blocked",
            "maintenance is not permission to resume a goal"
        );
        let foreign: String = db
            .query_row(
                "SELECT objective FROM thread_goals WHERE thread_id='foreign-session'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(foreign, objective, "do not rewrite another thread's goal");
    }
    assert_eq!(
        fs::read_to_string(stable).unwrap(),
        "Keep the original task; do not replay tools."
    );
}

#[test]
fn reconnect_session_identity_ignores_nonmetadata_payload_ids() {
    let lines = concat!(
        "{\"type\":\"response_item\",\"payload\":{\"id\":\"message-not-session\"}}\n",
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"native-session\"}}\n"
    );
    assert_eq!(session_thread_id(lines).as_deref(), Some("native-session"));
    assert_eq!(
        session_thread_id("{\"type\":\"response_item\",\"payload\":{\"id\":\"not-a-thread\"}}"),
        None
    );
}

#[test]
fn reconnect_session_identity_preserves_legacy_keys_and_rejects_empty_native_id() {
    for text in [
        r#"{"payload":{"thread_id":"legacy"}}"#,
        r#"{"payload":{"threadId":"legacy"}}"#,
        r#"{"thread_id":"legacy"}"#,
        r#"{"threadId":"legacy"}"#,
    ] {
        assert_eq!(session_thread_id(text).as_deref(), Some("legacy"));
    }
    for text in [
        r#"{"type":"session_meta","payload":{"id":"  "}}"#,
        r#"{"type":"session_meta","payload":{"id":42}}"#,
        r#"{"type":"response_item","payload":{"id":"tool-call"}}"#,
    ] {
        assert_eq!(session_thread_id(text), None);
    }
}
