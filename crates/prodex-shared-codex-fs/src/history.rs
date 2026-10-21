use super::*;
use std::io::{BufRead, BufReader, Read as _, Write as _};

const CODEX_HISTORY_MERGE_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
struct HistoryLine {
    ts: Option<i64>,
    line: String,
}

pub(super) fn is_history_jsonl(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "history.jsonl")
}

pub(super) fn merge_history_files(source: &Path, destination: &Path) -> Result<()> {
    let mut merged = Vec::new();

    let destination_metadata = load_shared_codex_entry_metadata(destination)?;
    if let Some(metadata) = destination_metadata.as_ref() {
        ensure_shared_codex_file_public(destination, metadata)?;
        load_history_lines(destination, &mut merged)?;
    }
    load_history_lines(source, &mut merged)?;

    let plan = prodex_mojo_core::shared_history_policy::dedup_plan(
        &merged
            .iter()
            .map(|entry| entry.line.as_str())
            .collect::<Vec<_>>(),
        CODEX_HISTORY_MERGE_MAX_BYTES,
    )
    .expect("Mojo history dedup policy returned invalid output");
    if plan.exceeds_limit {
        bail!(
            "merged history {} exceeds safe size limit ({} bytes)",
            destination.display(),
            CODEX_HISTORY_MERGE_MAX_BYTES
        );
    }
    let mut index = 0usize;
    merged.retain(|_| {
        let keep = plan.keep[index];
        index += 1;
        keep
    });
    debug_assert_eq!(merged.len(), plan.unique_count);

    let timestamps = merged.iter().map(|entry| entry.ts).collect::<Vec<_>>();
    let selected = prodex_mojo_core::shared_history_policy::timestamp_merge_order(&timestamps)
        .expect("compiled Mojo shared history chronological order returned invalid output");
    let mut original = merged.into_iter().map(Some).collect::<Vec<_>>();
    let merged = selected
        .into_iter()
        .map(|index| {
            original[index]
                .take()
                .expect("validated Mojo history indices are unique")
        })
        .collect::<Vec<_>>();

    let capacity = usize::try_from(plan.total_bytes)
        .context("merged history size does not fit this platform")?;
    let mut content = String::with_capacity(capacity);
    for (index, entry) in merged.iter().enumerate() {
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

fn load_history_lines(path: &Path, merged: &mut Vec<HistoryLine>) -> Result<()> {
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
        if line.is_empty() {
            continue;
        }

        let ts = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|value| value.get("ts").and_then(serde_json::Value::as_i64));
        merged.push(HistoryLine {
            ts,
            line: line.to_string(),
        });
    }

    Ok(())
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
    let file = prodex_core::open_regular_file_no_follow(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)? {
        bail!("history path changed while opening {}", path.display());
    }
    Ok(file)
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

#[cfg(test)]
mod compiled_history_timestamp_caller_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn history_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "prodex-history-compiled-order-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn compiled_history_merge_preserves_missing_timestamp_barriers_and_deduplicates() {
        let root = history_root("mixed");
        let source = root.join("source-history.jsonl");
        let destination = root.join("history.jsonl");
        let destination_lines = [
            r#"{"ts":40,"text":"late"}"#,
            r#"{"ts":2,"text":"early"}"#,
            r#"{"ts":2,"text":"early"}"#,
            r#"{"text":"missing timestamp"}"#,
            r#"{"ts":12,"text":"post barrier late"}"#,
        ];
        let source_lines = [
            r#"{"ts":-10,"text":"post barrier earliest"}"#,
            r#"not-json"#,
            r#"{"ts":9223372036854775807,"text":"max"}"#,
            r#"{"ts":-9223372036854775808,"text":"min"}"#,
            r#"{"ts":-9223372036854775808,"text":"min"}"#,
        ];
        fs::write(&destination, format!("{}\n", destination_lines.join("\n"))).unwrap();
        fs::write(&source, format!("{}\n", source_lines.join("\n"))).unwrap();

        merge_history_files(&source, &destination).unwrap();
        let merged = fs::read_to_string(&destination).unwrap();
        assert_eq!(
            merged.lines().collect::<Vec<_>>(),
            [
                destination_lines[1],
                destination_lines[0],
                destination_lines[3],
                source_lines[0],
                destination_lines[4],
                source_lines[1],
                source_lines[3],
                source_lines[2],
            ],
        );
        merge_history_files(&source, &destination).unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), merged);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compiled_history_merge_orders_equal_negative_timestamps_stably() {
        let root = history_root("stable");
        let source = root.join("src-history.jsonl");
        let destination = root.join("history.jsonl");
        let old = [
            r#"{"ts":9,"text":"nine"}"#,
            r#"{"ts":-2,"text":"equal first"}"#,
        ];
        let newer = [
            r#"{"ts":-2,"text":"equal second"}"#,
            r#"{"ts":0,"text":"zero"}"#,
        ];
        fs::write(&destination, format!("{}\n", old.join("\n"))).unwrap();
        fs::write(&source, newer.join("\n")).unwrap();
        merge_history_files(&source, &destination).unwrap();
        assert_eq!(
            fs::read_to_string(&destination)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [old[1], newer[0], newer[1], old[0]],
        );
        fs::remove_dir_all(root).unwrap();
    }
}
