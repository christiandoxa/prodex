use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn temp_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "prodex-runtime-launch-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

pub(super) fn write_runtime_launch_auth(
    path: PathBuf,
    text: impl Into<String>,
) -> Result<(), secret_store::SecretError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create runtime launch auth parent");
        #[cfg(unix)]
        {
            let temp_root = env::temp_dir();
            let mut directory = Some(parent);
            while let Some(current) = directory.filter(|current| *current != temp_root) {
                fs::set_permissions(current, fs::Permissions::from_mode(0o700))
                    .expect("failed to secure runtime launch auth parent");
                directory = current.parent().filter(|next| next.starts_with(&temp_root));
            }
        }
    }
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), text)
}
