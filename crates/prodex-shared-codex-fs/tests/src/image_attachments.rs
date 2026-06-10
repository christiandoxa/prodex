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
