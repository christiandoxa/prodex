use super::migrate_legacy_openai_sessions_for_launch;
use crate::{AppPaths, PreparedRuntimeLaunch};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

#[test]
fn external_home_index_is_repaired_before_uuid_and_in_app_resume() {
    let session_id = "01900000-0000-7000-8000-000000000912";
    for args in [
        vec![],
        vec![OsString::from("resume")],
        vec![OsString::from("resume"), OsString::from(session_id)],
    ] {
        let root = std::env::temp_dir().join(format!(
            "prodex-external-resume-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = root.join("external");
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("unrelated-shared"),
            legacy_shared_codex_root: root.join("legacy"),
        };
        let prepared = PreparedRuntimeLaunch {
            paths,
            codex_home: home.clone(),
            managed: false,
            runtime_proxy: None,
        };
        // Build the expected path using native separators. The repair reconstructs
        // components, so a slash-delimited string would differ only on Windows.
        let relative = PathBuf::from("sessions")
            .join("2026")
            .join("01")
            .join("01")
            .join(format!("rollout-2026-01-01T00-00-00-{session_id}.jsonl"));
        let rollout = home.join(&relative);
        fs::create_dir_all(rollout.parent().unwrap()).unwrap();
        let metadata = serde_json::json!({"id":session_id,"session_id":session_id,
            "timestamp":"2026-01-01T00:00:00Z","model_provider":"prodex-local",
            "cwd":"/tmp/fixture-project","originator":"codex-cli","cli_version":"0.169.0"});
        let contents = format!(
            "{{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{metadata}}}\n{}\n",
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":"keep original history"}})
        );
        fs::write(&rollout, &contents).unwrap();
        let database = home.join("state_5.sqlite");
        let db = rusqlite::Connection::open(&database).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,rollout_path TEXT,model_provider TEXT,title TEXT,updated_at INTEGER);").unwrap();
        let stale = root
            .join("profiles")
            .join(".prodex-overlay-retired-0")
            .join(&relative);
        db.execute(
            "INSERT INTO threads VALUES(?1,?2,'prodex-local','preserve title',123)",
            rusqlite::params![session_id, stale.to_str().unwrap()],
        )
        .unwrap();
        // A foreign/missing rollout must never be manufactured or borrowed.
        db.execute("INSERT INTO threads VALUES('01900000-0000-7000-8000-000000000913',?1,'prodex-local','untouched',456)",
            [stale.to_str().unwrap()]).unwrap();
        migrate_legacy_openai_sessions_for_launch(&prepared, &args, None).unwrap();
        let row: (String, String, i64) = db
            .query_row(
                "SELECT rollout_path,title,updated_at FROM threads WHERE id=?1",
                [session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                rollout.to_str().unwrap().to_owned(),
                "preserve title".into(),
                123
            )
        );
        let other: String = db
            .query_row(
                "SELECT rollout_path FROM threads WHERE id='01900000-0000-7000-8000-000000000913'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(other, stale.to_str().unwrap());
        assert_eq!(fs::read(&rollout).unwrap(), contents.as_bytes());
        // Idempotent launch repair does not rewrite session or unrelated metadata.
        migrate_legacy_openai_sessions_for_launch(&prepared, &args, None).unwrap();
        assert_eq!(fs::read(&rollout).unwrap(), contents.as_bytes());
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
}
