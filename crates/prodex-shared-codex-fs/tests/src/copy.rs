use super::*;
use filetime::FileTime;
use std::time::{SystemTime, UNIX_EPOCH};

struct CopyTestDir {
    path: PathBuf,
}

impl CopyTestDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "prodex-shared-copy-{name}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("test dir should be created");
        Self { path }
    }
}

impl Drop for CopyTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn make_readonly(path: &Path) {
    let mut permissions = fs::metadata(path)
        .expect("metadata should read")
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).expect("permissions should update");
}

#[test]
fn copy_directory_contents_replaces_readonly_existing_file() {
    let temp_dir = CopyTestDir::new("readonly-existing-file");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let relative_pack_path = Path::new(".tmp/plugins/.git/objects/pack/pack-test.pack");
    let source_pack_path = source.join(relative_pack_path);
    let destination_pack_path = destination.join(relative_pack_path);

    fs::create_dir_all(source_pack_path.parent().expect("source parent"))
        .expect("source parent should be created");
    fs::create_dir_all(destination_pack_path.parent().expect("destination parent"))
        .expect("destination parent should be created");
    fs::write(&source_pack_path, "fresh plugin pack").expect("source pack should write");
    fs::write(&destination_pack_path, "stale plugin pack").expect("destination pack should write");
    make_readonly(&destination_pack_path);

    copy_directory_contents(&source, &destination)
        .expect("readonly destination pack should be replaced");

    assert_eq!(
        fs::read_to_string(&destination_pack_path).expect("destination pack should be readable"),
        "fresh plugin pack"
    );
}

#[test]
fn copy_directory_contents_preserves_source_file_modified_time() {
    let temp_dir = CopyTestDir::new("preserve-mtime");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let source_file = source.join("sessions/2026/06/19/session.jsonl");
    let destination_file = destination.join("sessions/2026/06/19/session.jsonl");
    let source_mtime = FileTime::from_unix_time(1_765_930_560, 123_000_000);

    fs::create_dir_all(source_file.parent().expect("source parent"))
        .expect("source parent should be created");
    fs::write(&source_file, "session").expect("source file should write");
    filetime::set_file_mtime(&source_file, source_mtime).expect("source mtime should update");

    copy_directory_contents(&source, &destination).expect("copy should succeed");

    let destination_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&destination_file).expect("destination metadata should read"),
    );
    assert_eq!(destination_mtime, source_mtime);
}

#[test]
fn copy_directory_entry_ignores_a_source_file_removed_after_enumeration() {
    let temp_dir = CopyTestDir::new("vanished-source-file");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let source_file = source.join("queue_1.sqlite-journal");
    let destination_file = destination.join("queue_1.sqlite-journal");
    fs::create_dir_all(&source).expect("source should exist");
    fs::write(&source_file, "journal").expect("source file should write");
    let file_type = fs::symlink_metadata(&source_file)
        .expect("source metadata should read")
        .file_type();
    fs::remove_file(&source_file).expect("source should disappear");

    copy_directory_entry(&source, &source_file, &destination_file, file_type)
        .expect("a transient source file may disappear");

    assert!(!destination_file.exists());
}

#[cfg(unix)]
#[test]
fn copy_directory_contents_does_not_preserve_symlink_escape() {
    let temp_dir = CopyTestDir::new("symlink-escape");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let outside = temp_dir.path.join("outside-secret.txt");
    let source_link = source.join("auth.json");

    fs::create_dir_all(&source).expect("source should exist");
    fs::write(&outside, "outside secret").expect("outside target should write");
    std::os::unix::fs::symlink(&outside, &source_link).expect("source symlink should be created");

    copy_directory_contents(&source, &destination).expect("copy should succeed");

    assert!(
        fs::symlink_metadata(destination.join("auth.json")).is_err(),
        "copy must not preserve symlinks that point outside the copied CODEX_HOME"
    );
}

#[cfg(unix)]
#[test]
fn copy_codex_home_skips_codex_managed_packages_directory() {
    let temp_dir = CopyTestDir::new("codex-packages");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let release_dir = source.join("packages/standalone/releases/0.145.0-aarch64-apple-darwin");
    let current_link = source.join("packages/standalone/current");

    fs::create_dir_all(&release_dir).expect("release dir should be created");
    fs::write(release_dir.join("codex"), "binary").expect("release binary should write");
    std::os::unix::fs::symlink(
        Path::new("releases/0.145.0-aarch64-apple-darwin"),
        &current_link,
    )
    .expect("release symlink should be created");
    fs::write(source.join("config.toml"), "model = \"gpt-5\"\n").expect("config should write");

    copy_codex_home(&source, &destination)
        .expect("Codex managed packages directory must not fail the copy");

    assert_eq!(
        fs::read_to_string(destination.join("config.toml")).expect("config should be readable"),
        "model = \"gpt-5\"\n"
    );
    assert!(
        fs::symlink_metadata(destination.join("packages")).is_err(),
        "Codex managed packages directory should not be copied into the profile"
    );
}

#[cfg(unix)]
#[test]
fn copy_codex_home_copies_nested_packages_directory() {
    let temp_dir = CopyTestDir::new("nested-packages");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let nested_file = source.join("skills/packages/manifest.json");

    fs::create_dir_all(nested_file.parent().expect("nested parent"))
        .expect("nested parent should be created");
    fs::write(&nested_file, "{}").expect("nested file should write");

    copy_codex_home(&source, &destination).expect("copy should succeed");

    assert_eq!(
        fs::read_to_string(destination.join("skills/packages/manifest.json"))
            .expect("nested file should be readable"),
        "{}"
    );
}

#[cfg(unix)]
#[test]
fn copy_codex_home_cleans_only_destination_it_created_after_failure() {
    let temp_dir = CopyTestDir::new("failed-copy-cleanup");
    let source = temp_dir.path.join("source");
    let internal_dir = source.join("state/releases/current");
    let directory_link = source.join("state/current");

    fs::create_dir_all(&internal_dir).expect("internal directory should be created");
    std::os::unix::fs::symlink(&internal_dir, &directory_link)
        .expect("directory symlink should be created");

    for destination_existed in [false, true] {
        let destination = temp_dir
            .path
            .join(format!("destination-{destination_existed}"));
        if destination_existed {
            fs::create_dir(&destination).expect("existing destination should be created");
        }

        copy_codex_home(&source, &destination)
            .expect_err("directory symlink should fail the file-only copy");

        assert_eq!(
            destination.exists(),
            destination_existed,
            "copy should remove only the destination it created"
        );
    }
}

#[cfg(unix)]
#[test]
fn copy_directory_contents_localizes_internal_file_symlink() {
    let temp_dir = CopyTestDir::new("internal-file-symlink");
    let source = temp_dir.path.join("source");
    let destination = temp_dir.path.join("destination");
    let source_file = source.join("AGENTS.base.md");
    let source_link = source.join("AGENTS.md");
    let destination_link = destination.join("AGENTS.md");

    fs::create_dir_all(&source).expect("source should exist");
    fs::write(&source_file, "# Local agents\n").expect("source target should write");
    std::os::unix::fs::symlink(&source_file, &source_link)
        .expect("source symlink should be created");

    copy_directory_contents(&source, &destination).expect("copy should succeed");

    assert_eq!(
        fs::read_to_string(&destination_link).expect("localized symlink file should read"),
        "# Local agents\n"
    );
    assert!(
        !fs::symlink_metadata(&destination_link)
            .expect("destination metadata should read")
            .file_type()
            .is_symlink(),
        "internal file symlink should be localized as a regular file"
    );
}
