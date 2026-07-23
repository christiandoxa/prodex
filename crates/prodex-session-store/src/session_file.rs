use super::{SESSION_STORE_FILE_MAX_BYTES, repair_transaction::same_named_file};
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub(super) fn read_session_file_to_string(path: &Path) -> Result<String> {
    let file = open_session_regular_file(path)?;
    if file.metadata()?.len() > SESSION_STORE_FILE_MAX_BYTES {
        bail!(
            "session {} exceeds safe size limit ({} bytes)",
            path.display(),
            SESSION_STORE_FILE_MAX_BYTES
        );
    }
    let mut bytes = Vec::new();
    file.take(SESSION_STORE_FILE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read session {}", path.display()))?;
    if bytes.len() as u64 > SESSION_STORE_FILE_MAX_BYTES {
        bail!(
            "session {} exceeds safe size limit ({} bytes)",
            path.display(),
            SESSION_STORE_FILE_MAX_BYTES
        );
    }
    String::from_utf8(bytes).with_context(|| format!("failed to decode session {}", path.display()))
}

pub(super) fn visit_session_lines(path: &Path, mut visit: impl FnMut(&str) -> bool) -> Result<()> {
    let file = open_session_regular_file(path)?;
    let file_len = file.metadata()?.len();
    let mut reader = BufReader::new(file.take(file_len));
    let mut line = String::new();
    loop {
        line.clear();
        let read = (&mut reader)
            .take(SESSION_STORE_FILE_MAX_BYTES.saturating_add(1))
            .read_line(&mut line)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        if read == 0 {
            break;
        }
        if read as u64 > SESSION_STORE_FILE_MAX_BYTES {
            bail!(
                "session line {} exceeds safe size limit ({} bytes)",
                path.display(),
                SESSION_STORE_FILE_MAX_BYTES
            );
        }
        if !visit(&line) {
            break;
        }
    }
    Ok(())
}

fn open_session_regular_file(path: &Path) -> Result<fs::File> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect session {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!(
            "refusing to read session through symlink {}",
            path.display()
        );
    }
    if !metadata.file_type().is_file() {
        bail!("session path {} is not a file", path.display());
    }
    let file = fs::File::open(path)
        .with_context(|| format!("failed to read session {}", path.display()))?;
    if !same_named_file(path, &file)? {
        bail!("session path changed while opening {}", path.display());
    }
    Ok(file)
}
