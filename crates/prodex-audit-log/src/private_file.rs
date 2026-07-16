use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::path::Path;

pub(super) fn open_private_append_locked(path: &Path) -> Result<fs::File> {
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to secure {}", path.display()))?;
        file
    };

    #[cfg(not(unix))]
    let file = {
        if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            anyhow::bail!("refusing audit symlink {}", path.display());
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)
            .with_context(|| format!("failed to open {}", path.display()))?
    };

    file.lock()
        .with_context(|| format!("failed to lock {}", path.display()))?;
    Ok(file)
}

pub(super) fn open_no_follow_read(path: &Path) -> Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .with_context(|| format!("failed to open {}", path.display()))
    }

    #[cfg(not(unix))]
    {
        if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            anyhow::bail!("refusing audit symlink {}", path.display());
        }
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))
    }
}
