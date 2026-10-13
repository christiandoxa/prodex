use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn resume_picker_repairs_goal_only_paste_and_image_before_native_selection() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-picker-media-{}-{nonce}",
        std::process::id()
    ));
    let shared = root.join("shared");
    let id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
    let files = shared.join("attachments").join(id);
    fs::create_dir_all(&files).unwrap();
    let paths = AppPaths {
        root: root.join("prodex"),
        state_file: root.join("prodex/state.json"),
        managed_profiles_root: root.join("prodex/profiles"),
        shared_codex_root: shared.clone(),
        legacy_shared_codex_root: root.join("legacy"),
    };
    let old = paths
        .managed_profiles_root
        .join(".prodex-overlay-deleted-0")
        .join("attachments")
        .join(id);
    fs::write(
        files.join("pasted-text-1.txt"),
        b"The complete original multiline goal\nincluding acceptance criteria",
    )
    .unwrap();
    fs::write(files.join("image-1.png"), b"png fixture").unwrap();
    let objective = format!(
        "Read pasted text file {} and image file {} before continuing.",
        old.join("pasted-text-1.txt").display(),
        old.join("image-1.png").display()
    );
    let db = rusqlite::Connection::open(shared.join("goals_1.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, objective TEXT NOT NULL, status TEXT NOT NULL);").unwrap();
    db.execute(
        "INSERT INTO thread_goals VALUES ('saved-goal',?1,'paused')",
        [&objective],
    )
    .unwrap();
    let held = prodex_shared_codex_fs::lock_codex_sessions_for_child(&shared).unwrap();
    assert!(
        repair_resume_session_for_launch(&paths, &[OsString::from("resume")])
            .unwrap()
            .is_none()
    );
    let (objective, status): (String, String) = db
        .query_row(
            "SELECT objective,status FROM thread_goals WHERE thread_id='saved-goal'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        !objective.contains(".prodex-overlay-deleted-0"),
        "picker launched with dead goal attachment: {objective}"
    );
    assert!(objective.contains(&files.join("pasted-text-1.txt").display().to_string()));
    assert!(objective.contains(&files.join("image-1.png").display().to_string()));
    assert_eq!(status, "paused", "repair cannot activate goals");
    assert_eq!(
        fs::read(files.join("pasted-text-1.txt")).unwrap(),
        b"The complete original multiline goal\nincluding acceptance criteria"
    );
    drop(held);
    drop(db);
    let _ = fs::remove_dir_all(root);
}
