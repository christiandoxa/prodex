use super::*;
use std::os::unix::fs::symlink;
use std::time::{SystemTime, UNIX_EPOCH};

struct AuthTestDir(PathBuf);
impl AuthTestDir {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "prodex-auth-preparation-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn paths(&self) -> AppPaths {
        AppPaths {
            root: self.0.join("prodex"),
            state_file: self.0.join("prodex/state.json"),
            managed_profiles_root: self.0.join("prodex/profiles"),
            shared_codex_root: self.0.join("shared"),
            legacy_shared_codex_root: self.0.join("legacy"),
        }
    }
}
impl Drop for AuthTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn auth_preparation_defers_goal_repair_until_runtime_without_losing_shared_state() {
    let root = AuthTestDir::new();
    let paths = root.paths();
    fs::create_dir_all(&paths.shared_codex_root).unwrap();
    let old_attachment = root
        .0
        .join("old-overlay/attachments/thread-1/pasted-text-1.txt");
    fs::create_dir_all(old_attachment.parent().unwrap()).unwrap();
    fs::write(&old_attachment, "synthetic goal input").unwrap();
    let original = format!("read {}", old_attachment.display());
    let db = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, objective TEXT NOT NULL, status TEXT NOT NULL, tokens_used INTEGER NOT NULL);").unwrap();
    conn.execute(
        "INSERT INTO thread_goals VALUES ('thread-1', ?1, 'paused', 42)",
        [&original],
    )
    .unwrap();
    drop(conn);
    let home = paths.managed_profiles_root.join("account");
    prepare_managed_codex_home_for_auth_update(&paths, &home).unwrap();
    assert_eq!(
        fs::canonicalize(home.join("goals_1.sqlite")).unwrap(),
        fs::canonicalize(&db).unwrap()
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    let read = || {
        conn.query_row(
            "SELECT objective, status, tokens_used FROM thread_goals",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap()
    };
    assert_eq!(read(), (original.clone(), "paused".into(), 42));
    let stable = paths
        .shared_codex_root
        .join("attachments/thread-1/pasted-text-1.txt");
    assert!(
        !stable.exists(),
        "login must not perform goal-attachment I/O"
    );
    prepare_managed_codex_home_for_runtime_launch(&paths, &home).unwrap();
    assert_eq!(fs::read_to_string(&stable).unwrap(), "synthetic goal input");
    assert_eq!(
        read(),
        (format!("read {}", stable.display()), "paused".into(), 42)
    );
}

#[test]
fn auth_preparation_keeps_managed_home_symlink_defenses() {
    let root = AuthTestDir::new();
    let paths = root.paths();
    let outside = root.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(&paths.managed_profiles_root).unwrap();
    let home = paths.managed_profiles_root.join("account");
    symlink(&outside, &home).unwrap();
    assert!(prepare_managed_codex_home_for_auth_update(&paths, &home).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}
