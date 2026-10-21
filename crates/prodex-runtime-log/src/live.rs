use prodex_mojo_core::json::{JsonKind, JsonNode};
use prodex_mojo_core::{MojoError, live_log_record as mojo_live_log_record};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES: usize = 512;
pub const DEFAULT_RUNTIME_LIVE_LOG_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveLogEntry {
    pub sequence: u64,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveLogSnapshot {
    pub cursor: u64,
    pub dropped: u64,
    pub entries: Vec<RuntimeLiveLogEntry>,
}

#[derive(Debug, Default)]
struct RuntimeLiveLogPath {
    entries: VecDeque<RuntimeLiveLogEntry>,
    bytes: usize,
    dropped: u64,
    last_sequence: u64,
}

#[derive(Debug, Default)]
struct RuntimeLiveLogState {
    next_sequence: u64,
    total_entries: usize,
    total_bytes: usize,
    paths: BTreeMap<PathBuf, RuntimeLiveLogPath>,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeLiveLogStore {
    state: Mutex<RuntimeLiveLogState>,
}

impl RuntimeLiveLogStore {
    pub(super) fn append(&self, path: &Path, line: &str) -> Result<(), MojoError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let line = bounded_live_log_line(line)?;
        let sequence = state.next_sequence.saturating_add(1);
        state.next_sequence = sequence;
        let path = path.to_path_buf();
        let entry = RuntimeLiveLogEntry { sequence, line };
        let entry_bytes = entry.line.len();
        let (removed_count, removed_bytes) = {
            let bucket = state.paths.entry(path).or_default();
            bucket.last_sequence = sequence;
            bucket.bytes = bucket.bytes.saturating_add(entry_bytes);
            bucket.entries.push_back(entry);
            let mut removed_count = 0_usize;
            let mut removed_bytes = 0_usize;
            while mojo_live_log_record::retention_trim_required(
                mojo_live_log_record::LiveLogRetentionScope::Path,
                bucket.entries.len(),
                bucket.bytes,
                0,
            )? {
                let removed = bucket
                    .entries
                    .pop_front()
                    .expect("compiled Mojo selected retention from nonempty path");
                let removed_len = removed.line.len();
                bucket.bytes = bucket.bytes.saturating_sub(removed_len);
                bucket.dropped = bucket.dropped.saturating_add(1);
                removed_count += 1;
                removed_bytes = removed_bytes.saturating_add(removed_len);
            }
            (removed_count, removed_bytes)
        };
        state.total_entries = state
            .total_entries
            .saturating_add(1)
            .saturating_sub(removed_count);
        state.total_bytes = state
            .total_bytes
            .saturating_add(entry_bytes)
            .saturating_sub(removed_bytes);

        while mojo_live_log_record::retention_trim_required(
            mojo_live_log_record::LiveLogRetentionScope::Store,
            state.total_entries,
            state.total_bytes,
            state.paths.len(),
        )? {
            let sequences = state
                .paths
                .values()
                .map(|bucket| bucket.last_sequence)
                .collect::<Vec<_>>();
            let oldest_index = mojo_live_log_record::oldest_retention_path(&sequences)?;
            let oldest_path = state
                .paths
                .keys()
                .nth(oldest_index)
                .expect("compiled Mojo selected a valid oldest live-log path")
                .clone();
            if let Some(removed) = state.paths.remove(&oldest_path) {
                state.total_entries = state.total_entries.saturating_sub(removed.entries.len());
                state.total_bytes = state.total_bytes.saturating_sub(removed.bytes);
            }
        }
        Ok(())
    }

    pub(super) fn snapshot_after(
        &self,
        path: &Path,
        after: u64,
        limit: usize,
    ) -> RuntimeLiveLogSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let bucket = state.paths.get(path);
        let sequences = bucket
            .map(|bucket| {
                bucket
                    .entries
                    .iter()
                    .map(|entry| entry.sequence)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let plan = mojo_live_log_record::snapshot_plan(
            &sequences,
            after,
            limit,
            DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES,
            state.next_sequence,
            bucket.map_or(0, |bucket| bucket.dropped),
        )
        .expect("Mojo live-log snapshot planner returned invalid output");
        let entries = bucket
            .map(|bucket| {
                bucket
                    .entries
                    .iter()
                    .skip(plan.start)
                    .take(plan.count)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        RuntimeLiveLogSnapshot {
            cursor: plan.cursor,
            dropped: plan.dropped,
            entries,
        }
    }
}

fn bounded_live_log_line(line: &str) -> Result<String, MojoError> {
    if !mojo_live_log_record::record_exceeds_bound(line.len())? {
        return Ok(line.to_string());
    }
    if let Some(json_line) = bounded_live_log_json(line)? {
        return Ok(json_line);
    }
    mojo_live_log_record::truncate_plain_text(line)
}

// Serde remains the wire parser. Acquire only structural links, borrowed
// strings/keys and canonical Number lexemes; Mojo owns all JSON iteration,
// UTF-8 clipping, escape rules and serialization.
fn live_log_borrowed_tree<'a>(
    value: &'a serde_json::Value,
) -> Result<(Vec<JsonNode<'a>>, String), MojoError> {
    let mut pending = vec![(value, None::<usize>, "")];
    let mut nodes: Vec<JsonNode<'a>> = Vec::new();
    let mut last_child: Vec<Option<usize>> = Vec::new();
    let mut raw = String::new();
    while let Some((value, parent, key)) = pending.pop() {
        let index = nodes.len();
        let kind = match value {
            serde_json::Value::Null => JsonKind::Null,
            serde_json::Value::Bool(false) => JsonKind::False,
            serde_json::Value::Bool(true) => JsonKind::True,
            serde_json::Value::Number(_) => JsonKind::Number,
            serde_json::Value::String(_) => JsonKind::String,
            serde_json::Value::Array(_) => JsonKind::Array,
            serde_json::Value::Object(_) => JsonKind::Object,
        };
        let raw_start = raw.len();
        if let serde_json::Value::Number(number) = value {
            raw.push_str(&number.to_string());
        }
        nodes.push(JsonNode {
            kind,
            first_child: None,
            next_sibling: None,
            parent,
            key,
            text: value.as_str().unwrap_or_default(),
            raw_start,
            raw_length: raw.len() - raw_start,
        });
        last_child.push(None);
        link_live_log_child(&mut nodes, &mut last_child, parent, index);
        match value {
            serde_json::Value::Array(children) => {
                for child in children.iter().rev() {
                    pending.push((child, Some(index), ""));
                }
            }
            serde_json::Value::Object(children) => {
                for (key, child) in children.iter().rev() {
                    pending.push((child, Some(index), key.as_str()));
                }
            }
            _ => {}
        }
    }
    Ok((nodes, raw))
}

fn link_live_log_child(
    nodes: &mut [JsonNode<'_>],
    last_child: &mut [Option<usize>],
    parent: Option<usize>,
    index: usize,
) {
    if let Some(parent) = parent {
        if let Some(previous) = last_child[parent] {
            nodes[previous].next_sibling = Some(index);
        } else {
            nodes[parent].first_child = Some(index);
        }
        last_child[parent] = Some(index);
    }
}

fn live_log_clipped_value(value: &serde_json::Value) -> Result<String, MojoError> {
    let (nodes, raw) = live_log_borrowed_tree(value)?;
    mojo_live_log_record::clipped_json_projection(&nodes, &raw)
}

fn bounded_live_log_json(line: &str) -> Result<Option<String>, MojoError> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim_end()) else {
        return Ok(None);
    };
    let serialized = live_log_clipped_value(&value)?;
    let plan = mojo_live_log_record::json_plan(serialized.len())?;
    if !plan.compact_metadata {
        return Ok(Some(format!("{serialized}\n")));
    }
    // Compact metadata comes from the original Serde values, clipped by the
    // same compiled Mojo serializer. The host does not reimplement clipping.
    let event = value.get("event").map(live_log_clipped_value).transpose()?;
    let pid = value.get("pid").map(live_log_clipped_value).transpose()?;
    let timestamp = value
        .get("timestamp")
        .map(live_log_clipped_value)
        .transpose()?;
    Ok(Some(mojo_live_log_record::compact_json_metadata(
        event.as_deref(),
        pid.as_deref(),
        timestamp.as_deref(),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc::sync_channel};
    use std::thread;
    use std::time::Duration;

    fn dropped_marker(marker: super::super::RuntimeDroppedLogMarker) -> String {
        format!("dropped={}\n", marker.dropped_count)
    }

    #[test]
    fn append_contention_retains_event_after_state_is_released() {
        let store = Arc::new(RuntimeLiveLogStore::default());
        let path = PathBuf::from("runtime.log");
        let state = store.state.lock().unwrap();
        let (done_tx, done_rx) = sync_channel(0);
        let append_store = Arc::clone(&store);
        let append_path = path.clone();
        let append_thread = thread::spawn(move || {
            append_store.append(&append_path, "event\n").unwrap();
            done_tx.send(()).unwrap();
        });

        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(state);
        done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        append_thread.join().unwrap();

        let snapshot = store.snapshot_after(&path, 0, 1);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.dropped, 0);
        assert_eq!(snapshot.entries[0].line, "event\n");
    }

    #[test]
    fn snapshot_contention_waits_for_a_coherent_live_state() {
        let store = Arc::new(RuntimeLiveLogStore::default());
        let path = PathBuf::from("runtime.log");
        store.append(&path, "before\n").unwrap();
        let state = store.state.lock().unwrap();
        let (snapshot_tx, snapshot_rx) = sync_channel(0);
        let snapshot_store = Arc::clone(&store);
        let snapshot_path = path.clone();
        let snapshot_thread = thread::spawn(move || {
            snapshot_tx
                .send(snapshot_store.snapshot_after(&snapshot_path, 0, 1))
                .unwrap();
        });

        assert!(snapshot_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(state);
        let snapshot = snapshot_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        snapshot_thread.join().unwrap();

        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.dropped, 0);
        assert_eq!(snapshot.entries[0].line, "before\n");
    }

    #[test]
    fn token_usage_event_survives_live_store_contention() {
        let path = PathBuf::from("token-usage.log");
        let logger =
            super::super::RuntimeAsyncLogger::new_with_recording(4, dropped_marker, false).unwrap();
        let live_state = logger.inner.live.state.lock().unwrap();
        logger.try_enqueue(&path, "token_usage request=7 output_tokens=4\n".to_string());

        let (flushed_tx, flushed_rx) = sync_channel(0);
        let flush_logger = logger.clone();
        let flush_path = path.clone();
        let flush_thread = thread::spawn(move || {
            flushed_tx
                .send(flush_logger.flush_path(&flush_path))
                .unwrap();
        });

        assert!(flushed_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(live_state);
        flushed_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        flush_thread.join().unwrap();

        let snapshot = logger.live_log_snapshot_after(&path, 0, 1);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.dropped, 0);
        assert_eq!(
            snapshot.entries[0].line,
            "token_usage request=7 output_tokens=4\n"
        );
    }

    #[test]
    fn compiled_live_log_retention_trims_byte_caps_without_changing_line_order() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("bounded-bytes.log");
        let line = "x".repeat(100 * 1024);
        for _ in 0..22 {
            store.append(path, &line).unwrap();
        }
        let snapshot = store.snapshot_after(path, 0, usize::MAX);
        assert_eq!(snapshot.entries.len(), 20);
        assert_eq!(snapshot.dropped, 2);
        assert_eq!(snapshot.entries[0].sequence, 3);
        assert_eq!(snapshot.entries[19].sequence, 22);
    }

    #[test]
    fn compiled_live_log_retention_evicts_oldest_global_paths_and_preserves_ties() {
        let store = RuntimeLiveLogStore::default();
        for index in 0..66 {
            store
                .append(Path::new(&format!("bucket-{index:03}.log")), "x")
                .unwrap();
        }
        let state = store.state.lock().unwrap();
        assert_eq!(state.paths.len(), 64);
        assert!(!state.paths.contains_key(Path::new("bucket-000.log")));
        assert!(!state.paths.contains_key(Path::new("bucket-001.log")));
        assert!(state.paths.contains_key(Path::new("bucket-065.log")));
        drop(state);

        // Force equal saturated sequence values; Mojo must retain the
        // BTreeMap iteration order as its tie-breaker.
        let tied = RuntimeLiveLogStore::default();
        {
            let mut state = tied.state.lock().unwrap();
            state.next_sequence = u64::MAX;
        }
        for index in (0..66).rev() {
            tied.append(Path::new(&format!("tie-{index:03}.log")), "x")
                .unwrap();
        }
        let state = tied.state.lock().unwrap();
        assert_eq!(state.paths.len(), 64);
        assert!(!state.paths.contains_key(Path::new("tie-000.log")));
        assert!(!state.paths.contains_key(Path::new("tie-001.log")));
        assert!(state.paths.contains_key(Path::new("tie-065.log")));
    }

    #[test]
    fn compiled_live_log_retention_reduces_global_entry_pressure() {
        let store = RuntimeLiveLogStore::default();
        for index in 0..2049 {
            store
                .append(Path::new(&format!("shard-{}.log", index % 5)), "entry")
                .unwrap();
        }
        let state = store.state.lock().unwrap();
        assert!(state.total_entries <= 2048);
        assert_eq!(state.paths.len(), 4);
        assert!(!state.paths.contains_key(Path::new("shard-4.log")));
        assert_eq!(
            state.total_entries,
            state
                .paths
                .values()
                .map(|path| path.entries.len())
                .sum::<usize>()
        );
        assert_eq!(
            state.total_bytes,
            state.paths.values().map(|path| path.bytes).sum::<usize>()
        );
        drop(state);
        store.append(Path::new("shard-4.log"), "entry").unwrap();
        assert_eq!(store.state.lock().unwrap().paths.len(), 5);
    }

    #[test]
    fn live_log_store_is_bounded_and_keeps_complete_lines() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");

        for _ in 0..(DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES + 10) {
            store.append(path, "event\n").unwrap();
        }

        let snapshot = store.snapshot_after(path, 0, usize::MAX);
        assert_eq!(snapshot.entries.len(), DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES);
        assert!(snapshot.entries.iter().all(|entry| entry.line == "event\n"));
        assert_eq!(snapshot.dropped, 10);
    }

    #[test]
    fn live_log_snapshot_pages_oldest_unread_entries_without_skipping() {
        const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        for index in 0..3 {
            store.append(path, &format!("event-{index}\n")).unwrap();
        }

        let first = store.snapshot_after(path, 0, 2);
        assert_eq!(first.entries[0].line, "event-0\n");
        assert_eq!(first.entries[1].line, "event-1\n");
        assert_eq!(first.cursor, first.entries[1].sequence);

        let second = store.snapshot_after(path, first.cursor, 2);
        assert_eq!(second.entries.len(), 1);
        assert_eq!(second.entries[0].line, "event-2\n");
    }

    #[test]
    fn million_live_events_do_not_grow_memory_without_a_disk_sink() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");

        for _ in 0..1_000_000 {
            store.append(path, "load profile busy\n").unwrap();
        }

        let snapshot = store.snapshot_after(path, 0, usize::MAX);
        assert!(snapshot.entries.len() <= DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES);
        assert!(
            snapshot
                .entries
                .iter()
                .all(|entry| entry.line.contains("load"))
        );
    }

    #[test]
    fn compiled_live_log_json_tree_preserves_serde_canonical_scalars_keys_and_arrays() {
        let value = serde_json::json!({
            "a\n\"雪": ["text\t\r\n\0\"", null, true, false, -9223372036854775808_i64],
            "z": [18446744073709551615_u64, 1.25, 0, {"quote\\key":"東京"}],
            "empty": {},
            "other": [],
        });
        let projected = live_log_clipped_value(&value).unwrap();
        let expected = serde_json::to_string(&value).unwrap();
        assert_eq!(projected, expected);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&projected).unwrap(),
            value
        );
        for scalar in [
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!(0),
            serde_json::json!(-5),
            serde_json::json!(1.25),
            serde_json::json!("雪\n\"\\"),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            assert_eq!(
                live_log_clipped_value(&scalar),
                Ok(serde_json::to_string(&scalar).unwrap()),
                "{scalar:?}"
            );
        }
    }

    #[test]
    fn compiled_live_log_json_tree_clips_utf8_without_truncating_json_keys() {
        let oversized = format!("{}é{}", "x".repeat(8191), "y".repeat(130_000));
        let long_key = format!("very-large-key-{}", "a".repeat(10_000));
        let value = serde_json::json!({
            long_key.clone(): {
                "nested": ["another", {"payload": oversized, "flag": true}],
            },
        });
        let clipped = live_log_clipped_value(&value).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&clipped).unwrap();
        assert!(parsed.get(&long_key).is_some(), "keys must not be clipped");
        assert_eq!(
            parsed[&long_key]["nested"][1]["payload"],
            serde_json::json!(format!("{}é …[truncated]", "x".repeat(8191)))
        );
        assert_eq!(parsed[&long_key]["nested"][1]["flag"], true);
        assert!(
            !clipped.contains(&"y".repeat(100)),
            "the original oversized payload suffix must not survive"
        );
    }

    #[test]
    fn oversized_nested_json_clips_array_and_object_strings_at_utf8_boundaries() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let ascii = "a".repeat(70_000);
        let unicode = format!("{}é{}", "u".repeat(8 * 1024 - 1), "x".repeat(70_000));
        let value = serde_json::json!({
            "event": "nested",
            "entries": [{"ascii": ascii}, [{"unicode": unicode}]],
        });
        let line = format!("{}\n", serde_json::to_string(&value).unwrap());
        assert!(line.len() > 128 * 1024);
        store.append(path, &line).unwrap();

        let snapshot = store.snapshot_after(path, 0, 1);
        let clipped = serde_json::from_str::<serde_json::Value>(&snapshot.entries[0].line).unwrap();
        assert_eq!(
            clipped["entries"][0]["ascii"].as_str().unwrap(),
            format!("{} …[truncated]", "a".repeat(8 * 1024))
        );
        assert_eq!(
            clipped["entries"][1][0]["unicode"].as_str().unwrap(),
            format!("{}é …[truncated]", "u".repeat(8 * 1024 - 1))
        );
    }

    #[test]
    fn oversized_serialized_json_uses_compact_selected_metadata() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let mut value = serde_json::Map::new();
        value.insert(
            "timestamp".to_string(),
            serde_json::json!("2026-01-01T00:00:00Z"),
        );
        value.insert("pid".to_string(), serde_json::json!(42));
        value.insert("event".to_string(), serde_json::json!("e".repeat(70_000)));
        value.insert("extra".to_string(), serde_json::json!("discarded"));
        value.insert(
            "k".repeat(128 * 1024 + 1),
            serde_json::json!("unclipped key"),
        );
        let line = format!("{}\n", serde_json::to_string(&value).unwrap());
        assert!(line.len() > 128 * 1024);
        store.append(path, &line).unwrap();

        let snapshot = store.snapshot_after(path, 0, 1);
        let stored = &snapshot.entries[0].line;
        let compact = serde_json::from_str::<serde_json::Value>(stored).unwrap();
        assert_eq!(
            compact,
            serde_json::json!({
                "timestamp": "2026-01-01T00:00:00Z",
                "pid": 42,
                "event": format!("{} …[truncated]", "e".repeat(8 * 1024)),
                "message": "[live log record truncated]",
            })
        );
    }

    #[test]
    fn compiled_live_log_compact_json_preserves_canonical_order_escapes_and_missing_metadata() {
        let mut value = serde_json::Map::new();
        value.insert("event".to_string(), serde_json::json!("RUN\n雪"));
        value.insert("pid".to_string(), serde_json::json!(0));
        value.insert("timestamp".to_string(), serde_json::Value::Null);
        value.insert("padding_key_".repeat(12_000), serde_json::json!("ignore"));
        let line = serde_json::Value::Object(value);
        let input = format!("{}\n", serde_json::to_string(&line).unwrap());
        assert!(input.len() > 128 * 1024);
        assert_eq!(
            bounded_live_log_json(&input).unwrap(),
            Some(
                concat!(
                    "{\"event\":\"RUN\\n雪\",",
                    "\"message\":\"[live log record truncated]\",",
                    "\"pid\":0,\"timestamp\":null}\n",
                )
                .to_string()
            ),
        );
        let no_metadata = serde_json::json!({
            "padding_key_".repeat(12_000): "unused"
        });
        let input = format!("{}\n", serde_json::to_string(&no_metadata).unwrap());
        assert!(input.len() > 128 * 1024);
        assert_eq!(
            bounded_live_log_json(&input).unwrap(),
            Some("{\"message\":\"[live log record truncated]\"}\n".to_string()),
        );
    }

    #[test]
    fn oversized_ascii_plain_text_keeps_exact_prefix_marker_and_newline() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let line = "a".repeat(128 * 1024 + 1);
        store.append(path, &line).unwrap();

        let snapshot = store.snapshot_after(path, 0, 1);
        assert_eq!(
            snapshot.entries[0].line,
            format!("{} …[truncated]\n", "a".repeat(128 * 1024 - 32))
        );
    }

    #[test]
    fn oversized_unicode_plain_text_keeps_codepoint_crossing_prefix_limit() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let prefix = "a".repeat(128 * 1024 - 33);
        let line = format!("{prefix}é{}", "tail".repeat(100));
        store.append(path, &line).unwrap();

        let snapshot = store.snapshot_after(path, 0, 1);
        assert_eq!(
            snapshot.entries[0].line,
            format!("{prefix}é …[truncated]\n")
        );
    }

    #[test]
    fn within_boundary_live_log_line_is_passed_through_exactly() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let line = "x".repeat(128 * 1024);
        store.append(path, &line).unwrap();

        let snapshot = store.snapshot_after(path, 0, 1);
        assert_eq!(snapshot.entries[0].line, line);
    }

    #[test]
    fn within_boundary_non_json_text_preserves_original_newlines() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let line = "plain log\n\n";
        store.append(path, line).unwrap();

        let snapshot = store.snapshot_after(path, 0, 1);
        assert_eq!(snapshot.entries[0].line, line);
    }
}
