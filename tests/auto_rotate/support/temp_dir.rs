use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct TestDir {
    pub(crate) path: PathBuf,
}

static TEST_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl TestDir {
    pub(crate) fn new() -> Self {
        let temp_root = std::env::temp_dir()
            .canonicalize()
            .expect("failed to resolve temp dir");
        for _ in 0..32 {
            let unique = format!(
                "prodex-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after unix epoch")
                    .as_nanos(),
                TEST_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            );
            let path = temp_root.join(unique);
            match fs::create_dir(&path) {
                Ok(()) => {
                    #[cfg(unix)]
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                        .expect("failed to secure temp dir");
                    return Self { path };
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("failed to create temp dir: {err}"),
            }
        }
        panic!("failed to allocate unique temp dir after repeated collisions");
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
