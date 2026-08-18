use super::*;
use chrono::{Datelike, Utc};

#[test]
fn post_exit_maintenance_stabilizes_history_image_attachment_paths() {
    let root = temp_dir("post-exit-history-attachment");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _shared_override = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", "shared-codex");
    let paths = AppPaths::discover().expect("paths should resolve");
    let today = Utc::now().date_naive();
    let sessions_dir = paths.shared_codex_root.join(format!(
        "sessions/{}/{:02}/{:02}",
        today.year(),
        today.month(),
        today.day()
    ));
    let session_file = sessions_dir.join("rollout.jsonl");
    let image_source = root.join("codex-clipboard-history.png");
    fs::create_dir_all(&root).expect("root dir should exist");
    fs::create_dir_all(&sessions_dir).expect("sessions dir should exist");
    fs::write(&image_source, b"png bytes").expect("source image should write");
    let original_text = format!(
        "pasted session text plus <image path=\"{}\">",
        image_source.display()
    );
    fs::write(
        &session_file,
        format!(
            "{}\n",
            serde_json::json!({
                "timestamp": Utc::now().to_rfc3339(),
                "type": "event",
                "payload": {
                    "content": [{
                        "type": "input_text",
                        "text": original_text,
                    }],
                },
            })
        ),
    )
    .expect("session should write");

    let child = ChildProcessPlan::new(
        std::ffi::OsString::from("codex"),
        paths.shared_codex_root.clone(),
    );
    maintain_shared_codex_sessions_after_child_exit(&child);

    let copied = paths
        .shared_codex_root
        .join("image_attachments/codex-clipboard-history.png");
    assert_eq!(
        fs::read(&copied).expect("stable image should exist"),
        b"png bytes"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should read");
    let rewritten: serde_json::Value =
        serde_json::from_str(rewritten.trim()).expect("rewritten session should remain valid JSON");
    let rewritten_text = rewritten["payload"]["content"][0]["text"]
        .as_str()
        .expect("rewritten input text should remain present");
    assert!(
        rewritten_text.contains("pasted session text plus"),
        "post-exit maintenance should preserve pasted session text: {rewritten_text}"
    );
    let rewritten_path = rewritten_text
        .split_once("<image path=\"")
        .and_then(|(_, suffix)| suffix.strip_suffix("\">"))
        .expect("rewritten input should keep its image path tag");
    assert_eq!(std::path::Path::new(rewritten_path), copied);
    assert_ne!(std::path::Path::new(rewritten_path), image_source);
}
