use super::*;
use std::io::{BufRead, BufReader, Read as _, Write as _};

const CODEX_HISTORY_MERGE_MAX_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn is_history_jsonl(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "history.jsonl")
}

pub(super) fn merge_history_files(source: &Path, destination: &Path) -> Result<()> {
    #[derive(Debug)]
    struct HistoryLine {
        ts: Option<i64>,
        line: String,
        order: usize,
    }

    fn load_history_lines(
        path: &Path,
        merged: &mut Vec<HistoryLine>,
        seen: &mut BTreeSet<String>,
    ) -> Result<()> {
        let file = open_history_file_for_merge(path)?;
        let mut reader = BufReader::new(file);
        let mut raw_line = String::new();
        let mut read_bytes = 0_u64;

        loop {
            raw_line.clear();
            let bytes = read_history_line_bounded(
                &mut reader,
                &mut raw_line,
                CODEX_HISTORY_MERGE_MAX_BYTES.saturating_sub(read_bytes),
            )
            .with_context(|| format!("failed to read {}", path.display()))?;
            if bytes == 0 {
                break;
            }
            read_bytes = read_bytes.saturating_add(bytes as u64);
            if read_bytes > CODEX_HISTORY_MERGE_MAX_BYTES {
                bail!(
                    "history {} exceeds safe size limit ({} bytes)",
                    path.display(),
                    CODEX_HISTORY_MERGE_MAX_BYTES
                );
            }

            let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
            if line.is_empty() || !seen.insert(line.to_string()) {
                continue;
            }

            let ts = serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|value| value.get("ts").and_then(serde_json::Value::as_i64));
            merged.push(HistoryLine {
                ts,
                line: line.to_string(),
                order: merged.len(),
            });
        }

        Ok(())
    }

    let mut merged = Vec::new();
    let mut seen = BTreeSet::new();

    let destination_metadata = load_shared_codex_entry_metadata(destination)?;
    if let Some(metadata) = destination_metadata.as_ref() {
        ensure_shared_codex_file_public(destination, metadata)?;
        load_history_lines(destination, &mut merged, &mut seen)?;
    }
    load_history_lines(source, &mut merged, &mut seen)?;

    merged.sort_by(|left, right| match (left.ts, right.ts) {
        (Some(left_ts), Some(right_ts)) => {
            left_ts.cmp(&right_ts).then(left.order.cmp(&right.order))
        }
        _ => left.order.cmp(&right.order),
    });

    let mut content = String::new();
    for (index, entry) in merged.iter().enumerate() {
        let next_len = content
            .len()
            .saturating_add(usize::from(index > 0))
            .saturating_add(entry.line.len());
        if next_len as u64 > CODEX_HISTORY_MERGE_MAX_BYTES {
            bail!(
                "merged history {} exceeds safe size limit ({} bytes)",
                destination.display(),
                CODEX_HISTORY_MERGE_MAX_BYTES
            );
        }
        if index > 0 {
            content.push('\n');
        }
        content.push_str(&entry.line);
    }

    let permissions = match destination_metadata {
        Some(metadata) => metadata.permissions(),
        None => fs::symlink_metadata(source)
            .with_context(|| format!("failed to inspect {}", source.display()))?
            .permissions(),
    };
    replace_shared_codex_file_atomic(
        destination,
        permissions,
        None,
        "failed to write merged history",
        |file| file.write_all(content.as_bytes()),
    )
}

fn read_history_line_bounded(
    reader: &mut impl BufRead,
    line: &mut String,
    remaining_bytes: u64,
) -> std::io::Result<usize> {
    reader
        .take(remaining_bytes.saturating_add(1))
        .read_line(line)
}

fn open_history_file_for_merge(path: &Path) -> Result<fs::File> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!(
            "refusing to read history through symlink {}",
            path.display()
        );
    }
    if !metadata.file_type().is_file() {
        bail!("history path {} is not a file", path.display());
    }
    if metadata.len() > CODEX_HISTORY_MERGE_MAX_BYTES {
        bail!(
            "history {} exceeds safe size limit ({} bytes)",
            path.display(),
            CODEX_HISTORY_MERGE_MAX_BYTES
        );
    }
    let file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    if !history_same_file_metadata(&metadata, &file.metadata()?) {
        bail!("history path changed while opening {}", path.display());
    }
    Ok(file)
}

#[cfg(unix)]
fn history_same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn history_same_file_metadata(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Seek as _};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn merge_history_files_rejects_oversized_source_before_reading() {
        let root = std::env::temp_dir().join(format!(
            "prodex-history-oversized-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temp dir should be created");
        let source = root.join("source-history.jsonl");
        let destination = root.join("history.jsonl");
        fs::File::create(&source)
            .expect("source should be created")
            .set_len(CODEX_HISTORY_MERGE_MAX_BYTES + 1)
            .expect("source size should be set");
        let existing = "{\"ts\":1,\"text\":\"existing\"}\n";
        fs::write(&destination, existing).expect("destination should be written");

        let err =
            merge_history_files(&source, &destination).expect_err("oversized history should fail");

        assert!(format!("{err:#}").contains("exceeds safe size limit"));
        assert_eq!(fs::read_to_string(&destination).unwrap(), existing);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn history_line_reader_stops_after_bounded_overflow_byte() {
        let mut reader = BufReader::new(Cursor::new(vec![b'x'; 32]));
        let mut line = String::new();

        let read = read_history_line_bounded(&mut reader, &mut line, 8).unwrap();

        assert_eq!(read, 9);
        assert_eq!(line.len(), 9);
        assert_eq!(reader.stream_position().unwrap(), 9);
    }

    #[cfg(unix)]
    #[test]
    fn merge_history_files_preserves_destination_mode() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = std::env::temp_dir().join(format!(
            "prodex-history-mode-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source-history.jsonl");
        let destination = root.join("history.jsonl");
        fs::write(&source, "{\"ts\":2,\"text\":\"new\"}\n").unwrap();
        fs::write(&destination, "{\"ts\":1,\"text\":\"old\"}\n").unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o640)).unwrap();

        merge_history_files(&source, &destination).unwrap();

        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "{\"ts\":1,\"text\":\"old\"}\n{\"ts\":2,\"text\":\"new\"}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
