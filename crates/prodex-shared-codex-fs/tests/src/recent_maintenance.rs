use super::*;
use chrono::Datelike;
use std::time::{SystemTime, UNIX_EPOCH};

struct RecentMaintenanceTestDir {
    path: PathBuf,
}

impl RecentMaintenanceTestDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "prodex-shared-recent-maintenance-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("test dir should be created");
        Self { path }
    }

    fn app_paths(&self) -> AppPaths {
        AppPaths {
            root: self.path.join("prodex"),
            state_file: self.path.join("prodex/state.json"),
            managed_profiles_root: self.path.join("prodex/profiles"),
            shared_codex_root: self.path.join("shared-codex"),
            legacy_shared_codex_root: self.path.join("legacy-shared-codex"),
        }
    }
}

impl Drop for RecentMaintenanceTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
#[test]
fn recent_session_maintenance_rewrites_only_the_matching_goal_attachment() {
    let temp_dir = RecentMaintenanceTestDir::new();
    let paths = temp_dir.app_paths();
    let today = chrono::Utc::now().date_naive();
    let session_file = paths.shared_codex_root.join(format!(
        "sessions/{}/{:02}/{:02}/rollout-thread-1.jsonl",
        today.year(),
        today.month(),
        today.day()
    ));
    fs::create_dir_all(session_file.parent().expect("session parent")).expect("session parent");
    let old_root = temp_dir.path.join("deleted-overlay");
    let old_attachment = old_root.join("attachments/thread-1/pasted-text-1.txt");
    fs::create_dir_all(old_attachment.parent().expect("old attachment parent"))
        .expect("old attachment parent");
    fs::write(&old_attachment, b"goal attachment").expect("old attachment should write");
    fs::write(
        &session_file,
        format!(
            "{{\"timestamp\":\"{}T08:00:00Z\",\"thread_id\":\"thread-1\",\"type\":\"session_meta\"}}\n",
            today
        ),
    )
    .expect("session should write");

    fs::create_dir_all(&paths.shared_codex_root).expect("shared root should exist");
    let db_path = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&db_path).expect("goals db should open");
    conn.execute_batch(
        "CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, objective TEXT NOT NULL);",
    )
    .expect("goals table should create");
    conn.execute(
        "INSERT INTO thread_goals (thread_id, objective) VALUES (?1, ?2)",
        rusqlite::params!["thread-1", format!("read {}", old_attachment.display())],
    )
    .expect("goal should insert");
    drop(conn);

    maintain_recent_managed_codex_sessions(&paths).expect("recent maintenance should succeed");

    let stable_attachment = paths
        .shared_codex_root
        .join("attachments/thread-1/pasted-text-1.txt");
    assert_eq!(
        fs::read(&stable_attachment).expect("stable attachment should exist"),
        b"goal attachment"
    );
    let conn = rusqlite::Connection::open(&db_path).expect("goals db should reopen");
    let objective: String = conn
        .query_row(
            "SELECT objective FROM thread_goals WHERE thread_id = 'thread-1'",
            [],
            |row| row.get(0),
        )
        .expect("goal objective should read");
    assert!(objective.contains(&stable_attachment.display().to_string()));
    assert!(!objective.contains(&old_root.display().to_string()));
}
