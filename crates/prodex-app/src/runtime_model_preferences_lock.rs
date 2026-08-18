use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) fn try_acquire_model_preference_lock(
    path: &Path,
    deadline: Instant,
) -> Result<Option<fs::File>> {
    let lock_path = crate::runtime_store::json_lock_file_path(path);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(Some(file)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let now = Instant::now();
                if now >= deadline {
                    return Ok(None);
                }
                std::thread::sleep(
                    Duration::from_millis(10).min(deadline.saturating_duration_since(now)),
                );
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to lock {}", lock_path.display()));
            }
        }
    }
}
