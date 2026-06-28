use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct ImageAttachmentTestDir {
    path: PathBuf,
}

impl ImageAttachmentTestDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "prodex-image-attachments-{name}-{}-{unique}",
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

impl Drop for ImageAttachmentTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn persist_codex_session_image_attachments_rewrites_escaped_session_path() {
    let temp_dir = ImageAttachmentTestDir::new("escaped-session-path");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/10");
    let image_source = temp_dir.path.join("codex-clipboard-test.png");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::write(&image_source, b"png bytes").expect("source image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"payload":{{"content":[{{"type":"input_text","text":"<image name=[Image #1] path=\"{}\">"}}]}}}}"#,
            image_source.display()
        ),
    )
    .expect("session should write");

    persist_codex_session_image_attachments(&codex_home).expect("image attachments should persist");

    let copied = codex_home.join("image_attachments/codex-clipboard-test.png");
    assert_eq!(
        fs::read(&copied).expect("copied image should be readable"),
        b"png bytes"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    assert!(rewritten.contains(&format!(r#"path=\"{}\""#, copied.display())));
    assert!(!rewritten.contains(&image_source.display().to_string()));
}

#[test]
fn persist_codex_session_image_attachments_rewrites_to_existing_stable_copy_when_source_is_gone() {
    let temp_dir = ImageAttachmentTestDir::new("clipboard-image-source-gone");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/10");
    let old_path = temp_dir
        .path
        .join("deleted-overlay/codex-clipboard-test.png");
    let stable = codex_home.join("image_attachments/codex-clipboard-test.png");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::create_dir_all(stable.parent().unwrap()).expect("stable dir should exist");
    fs::write(&stable, b"stable png bytes").expect("stable image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"payload":{{"content":[{{"type":"input_text","text":"<image path=\"{}\">"}}]}}}}"#,
            old_path.display()
        ),
    )
    .expect("session should write");

    persist_codex_session_image_attachments(&codex_home).expect("image attachments should persist");

    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    assert!(
        rewritten.contains(&stable.display().to_string()),
        "session should use existing stable clipboard image: {rewritten}"
    );
    assert!(
        !rewritten.contains(&old_path.display().to_string()),
        "session should not retain deleted clipboard path: {rewritten}"
    );
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_persists_session_image_paths_in_shared_root() {
    let temp_dir = ImageAttachmentTestDir::new("prepare-shared-root");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/10");
    let image_source = temp_dir.path.join("codex-clipboard-shared.png");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::write(&image_source, b"shared png").expect("source image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"payload":{{"content":[{{"type":"input_text","text":"<image name=[Image #2] path=\"{}\">"}}]}}}}"#,
            image_source.display()
        ),
    )
    .expect("session should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed home should prepare");

    let copied = paths
        .shared_codex_root
        .join("image_attachments/codex-clipboard-shared.png");
    assert_eq!(
        fs::read(&copied).expect("shared image should be readable"),
        b"shared png"
    );
    let shared_session = paths
        .shared_codex_root
        .join("sessions/2026/06/10/rollout.jsonl");
    let rewritten = fs::read_to_string(shared_session).expect("shared session should be readable");
    assert!(rewritten.contains(&format!(r#"path=\"{}\""#, copied.display())));
    assert_eq!(
        fs::read_link(codex_home.join("sessions")).expect("sessions should be linked"),
        paths.shared_codex_root.join("sessions")
    );
}

#[test]
fn persist_codex_session_pasted_text_paths_in_tool_arguments() {
    let temp_dir = ImageAttachmentTestDir::new("pasted-text-tool-args");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/24");
    let paste_source = temp_dir
        .path
        .join("overlay/attachments/11111111-2222-4333-8444-555555555555/pasted-text-1.txt");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::create_dir_all(paste_source.parent().unwrap()).expect("paste source dir should exist");
    fs::write(&paste_source, b"important pasted context").expect("paste source should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"type":"response_item","payload":{{"arguments":"{{\"path\":\"{}\",\"max_bytes\":12000}}"}}}}"#,
            paste_source.display()
        ),
    )
    .expect("session should write");

    persist_codex_session_image_attachments(&codex_home).expect("attachments should persist");

    let copied =
        codex_home.join("attachments/11111111-2222-4333-8444-555555555555/pasted-text-1.txt");
    assert_eq!(
        fs::read(&copied).expect("copied paste should be readable"),
        b"important pasted context"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    assert!(
        rewritten.contains(&copied.display().to_string()),
        "session should point at stable pasted-text path: {rewritten}"
    );
    assert!(
        !rewritten.contains(&paste_source.display().to_string()),
        "session should not retain ephemeral overlay path: {rewritten}"
    );
}

#[test]
fn persist_codex_session_attachment_image_paths_in_goal_resume_context() {
    let temp_dir = ImageAttachmentTestDir::new("attachment-image-goal-resume");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/25");
    let image_source = temp_dir
        .path
        .join("overlay/attachments/31e02015-1740-4a23-85fe-51cf33a476e6/image-1.png");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::create_dir_all(image_source.parent().unwrap()).expect("image source dir should exist");
    fs::write(&image_source, b"png attachment bytes").expect("image source should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"type":"event","payload":{{"content":[{{"type":"input_text","text":"resume goal with image file: {}"}}]}}}}"#,
            image_source.display()
        ),
    )
    .expect("session should write");

    persist_codex_session_image_attachments(&codex_home).expect("attachments should persist");

    let copied = codex_home.join("attachments/31e02015-1740-4a23-85fe-51cf33a476e6/image-1.png");
    assert_eq!(
        fs::read(&copied).expect("copied image attachment should be readable"),
        b"png attachment bytes"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    assert!(
        rewritten.contains(&copied.display().to_string()),
        "session should point at stable attachment image path: {rewritten}"
    );
    assert!(
        !rewritten.contains(&image_source.display().to_string()),
        "session should not retain ephemeral overlay image path: {rewritten}"
    );
}

#[test]
fn persist_codex_session_attachment_image_rewrites_to_existing_stable_copy_when_source_is_gone() {
    let temp_dir = ImageAttachmentTestDir::new("attachment-image-source-gone");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/25");
    let old_path = temp_dir
        .path
        .join("deleted-overlay/attachments/31e02015-1740-4a23-85fe-51cf33a476e6/image-1.png");
    let stable = codex_home.join("attachments/31e02015-1740-4a23-85fe-51cf33a476e6/image-1.png");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::create_dir_all(stable.parent().unwrap()).expect("stable dir should exist");
    fs::write(&stable, b"stable image attachment").expect("stable image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"payload":{{"content":[{{"type":"input_text","text":"Resume goal with image file: {}"}}]}}}}"#,
            old_path.display()
        ),
    )
    .expect("session should write");

    persist_codex_session_image_attachments(&codex_home).expect("attachments should persist");

    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    assert!(
        rewritten.contains(&stable.display().to_string()),
        "session should use existing stable image attachment: {rewritten}"
    );
    assert!(
        !rewritten.contains(&old_path.display().to_string()),
        "session should not retain deleted overlay image path: {rewritten}"
    );
}

#[test]
fn persist_codex_session_pasted_text_rewrites_to_existing_stable_copy_when_source_is_gone() {
    let temp_dir = ImageAttachmentTestDir::new("pasted-text-source-gone");
    let codex_home = temp_dir.path.join("codex-home");
    let sessions_dir = codex_home.join("sessions/2026/06/24");
    let old_path = temp_dir
        .path
        .join("deleted-overlay/attachments/aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee/pasted-text-1.txt");
    let stable =
        codex_home.join("attachments/aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee/pasted-text-1.txt");
    let session_file = sessions_dir.join("rollout.jsonl");

    fs::create_dir_all(&sessions_dir).expect("sessions dir should be created");
    fs::create_dir_all(stable.parent().unwrap()).expect("stable dir should exist");
    fs::write(&stable, b"stable pasted context").expect("stable paste should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"payload":{{"content":[{{"type":"input_text","text":"Read {} before continuing."}}]}}}}"#,
            old_path.display()
        ),
    )
    .expect("session should write");

    persist_codex_session_image_attachments(&codex_home).expect("attachments should persist");

    let rewritten = fs::read_to_string(&session_file).expect("session should be readable");
    assert!(
        rewritten.contains(&stable.display().to_string()),
        "session should use existing stable copy: {rewritten}"
    );
    assert!(
        !rewritten.contains(&old_path.display().to_string()),
        "session should not retain deleted overlay path: {rewritten}"
    );
}
