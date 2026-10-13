use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct MediaFixture {
    root: PathBuf,
    paths: AppPaths,
    old: PathBuf,
    stable: PathBuf,
    db: Connection,
}
impl MediaFixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "prodex-resume-media-{}-{nonce}",
            std::process::id()
        ));
        let shared = root.join("shared");
        let paths = AppPaths {
            root: root.join("prodex"),
            state_file: root.join("prodex/state.json"),
            managed_profiles_root: root.join("prodex/profiles"),
            shared_codex_root: shared.clone(),
            legacy_shared_codex_root: root.join("legacy"),
        };
        let id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
        let stable = shared.join("attachments").join(id);
        let old = paths
            .managed_profiles_root
            .join(".prodex-overlay-deleted-0")
            .join("attachments")
            .join(id);
        fs::create_dir_all(&stable).unwrap();
        fs::create_dir_all(&paths.root).unwrap();
        fs::write(
            stable.join("pasted-text-1.txt"),
            b"Original goal text: keep all details intact.",
        )
        .unwrap();
        fs::write(
            stable.join("image-1.png"),
            b"\x89PNG\r\n\x1a\noriginal image bytes",
        )
        .unwrap();
        let db = Connection::open(shared.join("goals_1.sqlite")).unwrap();
        db.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, objective TEXT NOT NULL, status TEXT NOT NULL, goal_id TEXT NOT NULL, tokens_used INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL);").unwrap();
        Self {
            root,
            paths,
            old,
            stable,
            db,
        }
    }
    fn objective(&self) -> String {
        format!(
            "pasted text file: {}. Read this file before continuing. image file: {}",
            self.old.join("pasted-text-1.txt").display(),
            self.old.join("image-1.png").display()
        )
    }
    fn insert(&self, thread: &str, objective: &str) {
        self.db
            .execute(
                "INSERT INTO thread_goals VALUES (?1,?2,'paused','original-goal',37,1234)",
                params![thread, objective],
            )
            .unwrap();
    }
    fn read(&self, thread: &str) -> String {
        self.db
            .query_row(
                "SELECT objective FROM thread_goals WHERE thread_id=?",
                [thread],
                |r| r.get(0),
            )
            .unwrap()
    }
    fn assert_goal_preserved(&self, thread: &str) {
        let objective = self.read(thread);
        for file in ["pasted-text-1.txt", "image-1.png"] {
            assert!(
                objective.contains(&self.stable.join(file).display().to_string()),
                "missing durable attachment {file}: {objective}"
            );
        }
        assert!(!objective.contains(".prodex-overlay-deleted-0"));
        let state:(String,String,i64,i64)=self.db.query_row("SELECT status,goal_id,tokens_used,updated_at_ms FROM thread_goals WHERE thread_id=?",[thread],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!(state, ("paused".into(), "original-goal".into(), 37, 1234));
        assert_eq!(
            fs::read(self.stable.join("pasted-text-1.txt")).unwrap(),
            b"Original goal text: keep all details intact."
        );
        assert_eq!(
            fs::read(self.stable.join("image-1.png")).unwrap(),
            b"\x89PNG\r\n\x1a\noriginal image bytes"
        );
    }
}
impl Drop for MediaFixture {
    fn drop(&mut self) {
        // SQLite must be closed before cleanup on Windows.
        let conn = std::mem::replace(&mut self.db, Connection::open_in_memory().unwrap());
        drop(conn);
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn goal_resume_repairs_pasted_media_even_when_another_session_is_live() {
    let f = MediaFixture::new();
    f.insert("thread-1", &f.objective());
    let _held = crate::lock_codex_sessions_for_child(&f.paths.shared_codex_root).unwrap();
    crate::maintain_managed_codex_sessions(&f.paths).unwrap();
    f.assert_goal_preserved("thread-1");
}
#[test]
fn exact_session_goal_repair_does_not_wait_for_unrelated_native_child() {
    let f = MediaFixture::new();
    let objective = f.objective();
    f.insert("thread-1", &objective);
    f.insert("foreign-thread", &objective);
    let rollout = f
        .paths
        .shared_codex_root
        .join("sessions/2026/10/13/rollout-thread-1.jsonl");
    fs::create_dir_all(rollout.parent().unwrap()).unwrap();
    let contents = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n";
    fs::write(&rollout, contents).unwrap();
    let _held = crate::lock_codex_sessions_for_child(&f.paths.shared_codex_root).unwrap();
    crate::maintain_managed_codex_session_file(&f.paths, &rollout).unwrap();
    f.assert_goal_preserved("thread-1");
    assert_eq!(f.read("foreign-thread"), objective);
    assert_eq!(
        fs::read_to_string(rollout).unwrap(),
        contents,
        "do not rewrite live rollout while another child holds the lock"
    );
}
#[cfg(unix)]
#[test]
fn runtime_launch_prepares_goal_pastes_without_needing_a_rollout() {
    let f = MediaFixture::new();
    f.insert("thread-1", &f.objective());
    let _held = crate::lock_codex_sessions_for_child(&f.paths.shared_codex_root).unwrap();
    crate::prepare_managed_codex_home_for_runtime_launch(
        &f.paths,
        &f.paths.managed_profiles_root.join("next-home"),
    )
    .unwrap();
    f.assert_goal_preserved("thread-1");
}
#[test]
fn file_backed_goal_repairs_nested_pastes_and_images_not_just_sqlite_reference() {
    let f = MediaFixture::new();
    fs::write(f.stable.join("goal-objective.md"), f.objective()).unwrap();
    f.insert(
        "thread-1",
        &format!(
            "Read the Codex goal objective file at {} before continuing.",
            f.old.join("goal-objective.md").display()
        ),
    );
    persist_codex_goal_attachment_paths(&f.paths.shared_codex_root).unwrap();
    assert!(
        f.read("thread-1")
            .contains(&f.stable.join("goal-objective.md").display().to_string())
    );
    let nested = fs::read_to_string(f.stable.join("goal-objective.md")).unwrap();
    assert!(
        !nested.contains(".prodex-overlay-deleted-0"),
        "nested goal media must be durable: {nested}"
    );
    assert!(nested.contains(&f.stable.join("image-1.png").display().to_string()));
    assert!(nested.contains(&f.stable.join("pasted-text-1.txt").display().to_string()));
}
#[test]
fn already_canonical_goal_reference_still_repairs_nested_objective_media() {
    let f = MediaFixture::new();
    fs::write(f.stable.join("goal-objective.md"), f.objective()).unwrap();
    let objective = format!(
        "Read the Codex goal objective file at {} before continuing.",
        f.stable.join("goal-objective.md").display()
    );
    f.insert("thread-1", &objective);
    persist_codex_goal_attachment_path_for_thread(&f.paths.shared_codex_root, "thread-1").unwrap();
    assert_eq!(f.read("thread-1"), objective);
    assert!(
        !fs::read_to_string(f.stable.join("goal-objective.md"))
            .unwrap()
            .contains(".prodex-overlay-deleted-0")
    );
}

#[test]
fn repairing_goal_paths_preserves_a_concurrent_replacement_and_all_goal_state() {
    let f = MediaFixture::new();
    let before = f.objective();
    f.insert("thread-1", &before);
    let after = "A new user goal submitted while maintenance was reading";
    f.db.execute("UPDATE thread_goals SET objective=?1, goal_id='replacement-goal', status='active', tokens_used=99, updated_at_ms=7777 WHERE thread_id='thread-1'",[after]).unwrap();
    update_goal_attachment_reference(
        &f.db,
        &f.paths.shared_codex_root.join("goals_1.sqlite"),
        "thread-1",
        &before,
        "stale repair would erase new goal",
    )
    .unwrap();
    assert_eq!(
        f.read("thread-1"),
        after,
        "compare-and-swap must preserve concurrently replaced goal"
    );
    let state:(String,String,i64,i64)=f.db.query_row("SELECT status,goal_id,tokens_used,updated_at_ms FROM thread_goals WHERE thread_id='thread-1'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(
        state,
        ("active".into(), "replacement-goal".into(), 99, 7777)
    );
}
#[test]
fn repair_does_not_change_raw_pasted_text_even_if_it_mentions_overlay_paths() {
    let f = MediaFixture::new();
    let payload = f.objective();
    fs::write(f.stable.join("pasted-text-1.txt"), &payload).unwrap();
    f.insert("thread-1", &f.objective());
    persist_codex_goal_attachment_path_for_thread(&f.paths.shared_codex_root, "thread-1").unwrap();
    assert!(!f.read("thread-1").contains(".prodex-overlay-deleted-0"));
    assert_eq!(
        fs::read_to_string(f.stable.join("pasted-text-1.txt")).unwrap(),
        payload,
        "user instructions are opaque bytes, not editable generated wrappers"
    );
}
#[test]
fn missing_both_source_and_shared_attachment_is_not_rewritten_to_an_invented_file() {
    let f = MediaFixture::new();
    let objective = f.objective();
    fs::remove_file(f.stable.join("pasted-text-1.txt")).unwrap();
    fs::remove_file(f.stable.join("image-1.png")).unwrap();
    f.insert("thread-1", &objective);
    persist_codex_goal_attachment_paths(&f.paths.shared_codex_root).unwrap();
    assert_eq!(f.read("thread-1"), objective);
    assert!(!f.stable.join("pasted-text-1.txt").exists());
    assert!(!f.stable.join("image-1.png").exists());
}
#[cfg(unix)]
#[test]
fn generated_goal_wrapper_cannot_redirect_repair_through_a_symlink() {
    let f = MediaFixture::new();
    let outside = f.root.join("outside-user-objective.md");
    let payload = f.objective();
    fs::write(&outside, &payload).unwrap();
    let goal = f.stable.join("goal-objective.md");
    std::os::unix::fs::symlink(&outside, &goal).unwrap();
    f.insert(
        "thread-1",
        &format!(
            "Read the Codex goal objective file at {} before continuing.",
            goal.display()
        ),
    );
    persist_codex_goal_attachment_paths(&f.paths.shared_codex_root).unwrap();
    assert_eq!(fs::read_to_string(&outside).unwrap(), payload);
    assert!(goal.is_symlink());
}
#[test]
fn canonical_goal_resume_is_idempotent_across_three_process_home_cycles() {
    let f = MediaFixture::new();
    f.insert("thread-1", &f.objective());
    for _ in 0..3 {
        crate::repair_managed_codex_goal_attachments(&f.paths, Some("thread-1")).unwrap();
        f.assert_goal_preserved("thread-1");
    }
}

#[test]
fn compressed_rollout_header_repairs_goal_while_other_child_remains_active() {
    let f = MediaFixture::new();
    f.insert("thread-1", &f.objective());
    let rollout = f
        .paths
        .shared_codex_root
        .join("sessions/2026/10/13/rollout-thread-1.jsonl.zst");
    fs::create_dir_all(rollout.parent().unwrap()).unwrap();
    let content = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n";
    let encoded = zstd::stream::encode_all(content.as_slice(), 3).unwrap();
    fs::write(&rollout, &encoded).unwrap();
    let held = crate::lock_codex_sessions_for_child(&f.paths.shared_codex_root).unwrap();
    crate::maintain_managed_codex_session_file(&f.paths, &rollout).unwrap();
    f.assert_goal_preserved("thread-1");
    assert_eq!(
        fs::read(&rollout).unwrap(),
        encoded,
        "do not mutate an active compressed rollout"
    );
    drop(held);
}

#[cfg(unix)]
#[test]
fn attachment_id_directory_cannot_redirect_generated_goal_repair_outside_storage() {
    let f = MediaFixture::new();
    let outside = f.root.join("external-user-dir");
    fs::create_dir_all(&outside).unwrap();
    let payload = f.objective();
    fs::write(outside.join("goal-objective.md"), &payload).unwrap();
    let alias = f
        .paths
        .shared_codex_root
        .join("attachments")
        .join("bbbbbbbb-cccc-4ddd-8eee-ffffffffffff");
    std::os::unix::fs::symlink(&outside, &alias).unwrap();
    f.insert(
        "thread-1",
        &format!(
            "Read the Codex goal objective file at {} before continuing.",
            alias.join("goal-objective.md").display()
        ),
    );
    persist_codex_goal_attachment_paths(&f.paths.shared_codex_root).unwrap();
    assert_eq!(
        fs::read_to_string(outside.join("goal-objective.md")).unwrap(),
        payload
    );
}
