use super::*;

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
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for raw_line in content.lines() {
            let line = raw_line.trim_end_matches('\r');
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

    if destination.exists() {
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
        if index > 0 {
            content.push('\n');
        }
        content.push_str(&entry.line);
    }

    fs::write(destination, content)
        .with_context(|| format!("failed to write merged history {}", destination.display()))
}
