use super::{SESSION_STORE_FILE_MAX_BYTES, repair_transaction::same_named_file};
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub(super) fn read_session_file_to_string(path: &Path) -> Result<String> {
    let file = open_session_regular_file(path)?;
    if !is_compressed_session_file(path) && file.metadata()?.len() > SESSION_STORE_FILE_MAX_BYTES {
        bail!(
            "session {} exceeds safe size limit ({} bytes)",
            path.display(),
            SESSION_STORE_FILE_MAX_BYTES
        );
    }
    let mut input: Box<dyn Read> = if is_compressed_session_file(path) {
        Box::new(zstd::stream::read::Decoder::new(file)?)
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(SESSION_STORE_FILE_MAX_BYTES.saturating_add(1))
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
    let input: Box<dyn Read> = if is_compressed_session_file(path) {
        Box::new(zstd::stream::read::Decoder::new(file)?)
    } else {
        Box::new(file)
    };
    let mut reader = BufReader::new(input);
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

fn is_compressed_session_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".jsonl.zst"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn compressed_rollout_is_read_as_jsonl() {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-file-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("rollout-00000000-0000-0000-0000-000000000001.jsonl.zst");
        fs::write(
            &path,
            zstd::stream::encode_all(&b"{\"id\":1}\n"[..], 3).unwrap(),
        )
        .unwrap();
        assert_eq!(read_session_file_to_string(&path).unwrap(), "{\"id\":1}\n");
        let mut lines = Vec::new();
        visit_session_lines(&path, |line| {
            lines.push(line.to_string());
            true
        })
        .unwrap();
        assert_eq!(lines, ["{\"id\":1}\n"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compressed_rollout_metadata_repair_preserves_compression() {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-repair-zst-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = root.join("sessions/2026/08/19");
        fs::create_dir_all(&dir).unwrap();
        let id = "01900000-0000-7000-8000-000000000001";
        let path = dir.join(format!("rollout-{id}.jsonl.zst"));
        let contents = format!(
            "{{\"type\":\"event\"}}\n{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n"
        );
        fs::write(
            &path,
            zstd::stream::encode_all(contents.as_bytes(), 3).unwrap(),
        )
        .unwrap();
        assert_eq!(
            crate::repair_resume_session_metadata_prefix(&root, id).unwrap(),
            Some(path)
        );
        let repaired =
            read_session_file_to_string(&dir.join(format!("rollout-{id}.jsonl.zst"))).unwrap();
        let first_line = repaired.lines().next().unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(first_line).unwrap()["type"],
            "session_meta"
        );
        let _ = fs::remove_dir_all(root);
    }
}
