use super::*;

#[test]
fn post_exit_maintenance_stabilizes_history_image_attachment_paths() {
    let root = temp_dir("post-exit-history-attachment");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _shared_override = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", "shared-codex");
    let paths = AppPaths::discover().expect("paths should resolve");
    let sessions_dir = paths.shared_codex_root.join("sessions/2026/06/24");
    let session_file = sessions_dir.join("rollout.jsonl");
    let image_source = root.join("codex-clipboard-history.png");
    fs::create_dir_all(&root).expect("root dir should exist");
    fs::create_dir_all(&sessions_dir).expect("sessions dir should exist");
    fs::write(&image_source, b"png bytes").expect("source image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"timestamp":"2026-06-24T01:02:03Z","type":"event","payload":{{"content":[{{"type":"input_text","text":"pasted session text plus <image path=\"{}\">"}}]}}}}"#,
            image_source.display()
        ),
    )
    .expect("session should write");

    maintain_shared_codex_sessions_after_child_exit();

    let copied = paths
        .shared_codex_root
        .join("image_attachments/codex-clipboard-history.png");
    assert_eq!(
        fs::read(&copied).expect("stable image should exist"),
        b"png bytes"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should read");
    assert!(
        rewritten.contains("pasted session text plus"),
        "post-exit maintenance should preserve pasted session text: {rewritten}"
    );
    assert!(
        rewritten.contains(&copied.display().to_string()),
        "session should reference stable attachment path after post-exit maintenance: {rewritten}"
    );
    assert!(
        !rewritten.contains(&image_source.display().to_string()),
        "session should no longer reference transient clipboard image path: {rewritten}"
    );
}
