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
