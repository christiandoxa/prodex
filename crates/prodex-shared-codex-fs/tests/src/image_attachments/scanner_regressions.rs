use super::*;

#[test]
fn persist_codex_session_attachment_scanner_never_rewinds_across_escaped_newline() {
    let temp_dir = ImageAttachmentTestDir::new("attachment-monotonic-cursor");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/10/06");
    let first_id = "11111111-2222-4333-8444-555555555555";
    let second_id = "66666666-7777-4888-8999-aaaaaaaaaaaa";
    let old_first = temp_dir
        .path
        .join("deleted-overlay/attachments")
        .join(first_id)
        .join("pasted-text-1.txt");
    let old_second = temp_dir
        .path
        .join("deleted-overlay/attachments")
        .join(second_id)
        .join("image-1.png");
    let stable_first = codex_home
        .join("attachments")
        .join(first_id)
        .join("pasted-text-1.txt");
    let stable_second = codex_home
        .join("attachments")
        .join(second_id)
        .join("image-1.png");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should exist");
    fs::create_dir_all(stable_first.parent().unwrap()).expect("first stable dir");
    fs::create_dir_all(stable_second.parent().unwrap()).expect("second stable dir");
    fs::write(&stable_first, b"stable text").expect("first stable attachment");
    fs::write(&stable_second, b"stable image").expect("second stable attachment");
    let payload = serde_json::json!({
        "type": "response_item",
        "payload": {
            "text": format!("{}\n{}", old_first.display(), old_second.display())
        }
    });
    fs::write(&session_file, payload.to_string()).expect("session should write");

    persist_codex_session_image_attachments(&codex_home)
        .expect("escaped-newline attachment maintenance must not panic or rewind");

    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    let value: serde_json::Value =
        serde_json::from_str(&rewritten).expect("rewritten session should remain valid JSON");
    let text = value["payload"]["text"]
        .as_str()
        .expect("payload text should remain a string");
    assert!(
        text.contains(stable_first.to_string_lossy().as_ref()),
        "{text}"
    );
    assert!(
        text.contains(stable_second.to_string_lossy().as_ref()),
        "{text}"
    );
    assert!(!text.contains("deleted-overlay"), "{text}");
}
