use super::*;
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
