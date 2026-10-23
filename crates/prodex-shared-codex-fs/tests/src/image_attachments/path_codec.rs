use super::*;

#[test]
#[cfg(windows)]
fn windows_attachment_paths_use_compiled_escape_codec() {
    let plain = r"C:\test-user\image_attachments\clipboard.png";
    let escaped = r"C:\\test-user\\image_attachments\\clipboard.png";
    assert_eq!(decode_codex_session_path(escaped).as_ref(), plain);
    assert_eq!(
        render_codex_session_path(Path::new(plain), escaped),
        escaped
    );
}

#[test]
#[cfg(not(windows))]
fn unix_attachment_paths_keep_literal_backslashes() {
    let raw = r"/tmp/test-user/literal\\image.png";
    assert_eq!(decode_codex_session_path(raw).as_ref(), raw);
    assert_eq!(render_codex_session_path(Path::new(raw), r"\\"), raw);
}
