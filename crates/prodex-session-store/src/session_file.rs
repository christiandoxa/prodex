use super::{SESSION_STORE_FILE_MAX_BYTES, repair_transaction::same_named_file};
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
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

/// Returns the decoded byte length of a session file.
///
/// Compressed rollouts use decoded offsets so callers can compare a marker written before and
/// after a child attempt using the same coordinate system for both file formats.
pub fn session_file_logical_len(path: &Path) -> Result<u64> {
    Ok(read_session_file_to_string(path)?.len() as u64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Result of scanning newline-complete session records.
pub struct SessionFileScan {
    /// Whether the visitor accepted a complete record.
    pub matched: bool,
    /// Decoded byte offset immediately after the last complete record read.
    pub complete_offset: u64,
}

/// Scans complete decoded session lines after a decoded byte offset.
pub fn session_file_scan_since(
    path: &Path,
    offset: u64,
    visit: impl FnMut(&str) -> bool,
) -> Result<SessionFileScan> {
    let file = open_session_regular_file(path)?;
    if !is_compressed_session_file(path) {
        let file_len = file.metadata()?.len();
        let start = offset.min(file_len);
        if file_len.saturating_sub(start) > SESSION_STORE_FILE_MAX_BYTES {
            bail!(
                "session tail {} exceeds safe size limit ({} bytes)",
                path.display(),
                SESSION_STORE_FILE_MAX_BYTES
            );
        }
        let mut file = file;
        file.seek(SeekFrom::Start(start))?;
        let mut reader = BufReader::new(file);
        return visit_session_lines_from_reader(path, &mut reader, start, start, visit);
    }

    if offset > SESSION_STORE_FILE_MAX_BYTES {
        bail!(
            "session {} offset exceeds safe size limit ({} bytes)",
            path.display(),
            SESSION_STORE_FILE_MAX_BYTES
        );
    }
    let mut decoder = zstd::stream::read::Decoder::new(file)?;
    {
        let mut remaining = offset;
        let mut discarded = [0_u8; 8 * 1024];
        while remaining > 0 {
            let chunk_len = remaining.min(discarded.len() as u64) as usize;
            let read = decoder.read(&mut discarded[..chunk_len])?;
            if read == 0 {
                return Ok(SessionFileScan {
                    matched: false,
                    complete_offset: offset.saturating_sub(remaining),
                });
            }
            remaining = remaining.saturating_sub(read as u64);
        }
    }

    let mut reader = BufReader::new(decoder);
    visit_session_lines_from_reader(path, &mut reader, offset, offset, visit)
}

/// Returns whether a predicate matched a decoded complete session line after an offset.
pub fn session_file_has_line_since(
    path: &Path,
    offset: u64,
    visit: impl FnMut(&str) -> bool,
) -> Result<bool> {
    Ok(session_file_scan_since(path, offset, visit)?.matched)
}

fn visit_session_lines_from_reader<R: Read>(
    path: &Path,
    reader: &mut BufReader<R>,
    mut decoded_bytes: u64,
    scan_start: u64,
    mut visit: impl FnMut(&str) -> bool,
) -> Result<SessionFileScan> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        if read == 0 {
            return Ok(SessionFileScan {
                matched: false,
                complete_offset: decoded_bytes,
            });
        }
        decoded_bytes = decoded_bytes.saturating_add(read as u64);
        if decoded_bytes.saturating_sub(scan_start) > SESSION_STORE_FILE_MAX_BYTES {
            bail!(
                "session exceeds safe size limit ({} bytes)",
                SESSION_STORE_FILE_MAX_BYTES
            );
        }
        if !line.ends_with('\n') {
            return Ok(SessionFileScan {
                matched: false,
                complete_offset: decoded_bytes.saturating_sub(read as u64),
            });
        }
        if visit(&line) {
            return Ok(SessionFileScan {
                matched: true,
                complete_offset: decoded_bytes,
            });
        }
    }
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

    #[test]
    fn compressed_rollout_line_scan_uses_decoded_offsets() {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-file-scan-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("rollout-00000000-0000-0000-0000-000000000001.jsonl.zst");
        let before = b"{\"type\":\"session_meta\"}\n";
        let after = b"{\"type\":\"usage_limit_reached\"}\n";
        let mut contents = before.to_vec();
        contents.extend_from_slice(after);
        fs::write(
            &path,
            zstd::stream::encode_all(contents.as_slice(), 3).unwrap(),
        )
        .unwrap();

        assert_eq!(
            session_file_logical_len(&path).unwrap(),
            contents.len() as u64
        );
        assert!(
            session_file_has_line_since(&path, before.len() as u64, |line| {
                line.contains("usage_limit_reached")
            })
            .unwrap()
        );
        assert!(!session_file_has_line_since(&path, contents.len() as u64, |_| true).unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn line_scan_keeps_cursor_before_an_incomplete_record() {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-file-partial-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("rollout-00000000-0000-0000-0000-000000000001.jsonl");
        fs::write(&path, b"{\"type\":\"session_meta\"}\n{\"type\":\"error\"").unwrap();
        let first = session_file_scan_since(&path, 0, |line| line.contains("error")).unwrap();
        assert!(!first.matched);
        assert_eq!(
            first.complete_offset,
            b"{\"type\":\"session_meta\"}\n".len() as u64
        );

        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        use std::io::Write as _;
        file.write_all(b"}\n").unwrap();
        let second =
            session_file_scan_since(&path, first.complete_offset, |line| line.contains("error"))
                .unwrap();
        assert!(second.matched);
        assert_eq!(second.complete_offset, fs::metadata(&path).unwrap().len());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn line_scan_tails_a_large_sparse_rollout_without_reading_history() {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-file-large-tail-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("rollout-00000000-0000-0000-0000-000000000001.jsonl");
        let offset = SESSION_STORE_FILE_MAX_BYTES + 1;
        let file = fs::File::create(&path).unwrap();
        file.set_len(offset).unwrap();
        drop(file);
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        use std::io::Write as _;
        file.write_all(b"{\"type\":\"error\"}\n").unwrap();

        let scan = session_file_scan_since(&path, offset, |line| line.contains("error")).unwrap();
        assert!(scan.matched);
        assert_eq!(scan.complete_offset, fs::metadata(&path).unwrap().len());
        let _ = fs::remove_dir_all(root);
    }
}
