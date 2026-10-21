use super::*;

#[test]
fn compiled_image_tag_batch_rewrites_nested_paths_and_preserves_malformed_text() {
    let temp_dir = ImageAttachmentTestDir::new("compiled-image-tag-batch");
    let codex_home = temp_dir.path.join("codex-home");
    let first = temp_dir.path.join("codex-clipboard-outer.png");
    let second = temp_dir.path.join("codex-clipboard-inner.png");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let input = format!(
        "prefix <image missing> <image path=\"{}\" <image path=\"{}\">> tail <image unfinished",
        first.display(),
        second.display()
    );
    let rewritten = rewrite_codex_persisted_attachment_paths(&codex_home, &input).unwrap();
    let stable_first = codex_home.join("image_attachments/codex-clipboard-outer.png");
    let stable_second = codex_home.join("image_attachments/codex-clipboard-inner.png");
    assert_eq!(
        rewritten,
        format!(
            "prefix <image missing> <image path=\"{}\" <image path=\"{}\">> tail <image unfinished",
            stable_first.display(),
            stable_second.display()
        )
    );
    assert_eq!(fs::read(&stable_first).unwrap(), b"first");
    assert_eq!(fs::read(&stable_second).unwrap(), b"second");
    assert!(codex_session_image_attachments_are_stable(
        &codex_home,
        &rewritten
    ));
    assert_eq!(
        codex_session_persisted_attachment_paths(&rewritten),
        vec![stable_first, stable_second]
    );
}

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
