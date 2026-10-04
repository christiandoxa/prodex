use prodex_mojo_core::{MojoError, live_log_record as mojo_live_log_record};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES: usize = 512;
pub const DEFAULT_RUNTIME_LIVE_LOG_MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_RUNTIME_LIVE_LOG_PATHS: usize = 64;
const MAX_RUNTIME_LIVE_LOG_ENTRIES: usize = 2048;
const MAX_RUNTIME_LIVE_LOG_BYTES: usize = 8 * 1024 * 1024;

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
            while bucket.entries.len() > DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES
                || bucket.bytes > DEFAULT_RUNTIME_LIVE_LOG_MAX_BYTES
            {
                let Some(removed) = bucket.entries.pop_front() else {
                    break;
                };
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

        while state.total_entries > MAX_RUNTIME_LIVE_LOG_ENTRIES
            || state.total_bytes > MAX_RUNTIME_LIVE_LOG_BYTES
            || state.paths.len() > MAX_RUNTIME_LIVE_LOG_PATHS
        {
            let Some(oldest_path) = state
                .paths
                .iter()
                .min_by_key(|(_, bucket)| bucket.last_sequence)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
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
        let Some(bucket) = state.paths.get(path) else {
            return RuntimeLiveLogSnapshot {
                cursor: state.next_sequence,
                dropped: 0,
                entries: Vec::new(),
            };
        };
        let entries = bucket
            .entries
            .iter()
            .filter(|entry| entry.sequence > after)
            .take(limit.min(DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES))
            .cloned()
            .collect::<Vec<_>>();
        let cursor = entries
            .last()
            .map(|entry| entry.sequence)
            .unwrap_or(state.next_sequence);
        RuntimeLiveLogSnapshot {
            cursor,
            dropped: bucket.dropped,
            entries,
        }
    }
}

fn bounded_live_log_line(line: &str) -> Result<String, MojoError> {
    if !mojo_live_log_record::record_exceeds_bound(line.len())? {
        return Ok(line.to_string());
    }

    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(line.trim_end()) {
        materialize_nested_string_clips(&mut value)?;
        if let Ok(serialized) = serde_json::to_string(&value) {
            let plan = mojo_live_log_record::json_plan(serialized.len())?;
            if !plan.compact_metadata {
                return Ok(format!("{serialized}\n"));
            }
            let mut compact = serde_json::Map::new();
            for (key, include) in [
                ("timestamp", plan.timestamp),
                ("pid", plan.pid),
                ("event", plan.event),
            ] {
                if include && let Some(value) = value.get(key) {
                    compact.insert(key.to_string(), value.clone());
                }
            }
            compact.insert(
                "message".to_string(),
                serde_json::Value::String("[live log record truncated]".to_string()),
            );
            if let Ok(serialized) = serde_json::to_string(&compact) {
                return Ok(format!("{serialized}\n"));
            }
        }
    }

    mojo_live_log_record::truncate_plain_text(line)
}

fn materialize_nested_string_clips(value: &mut serde_json::Value) -> Result<(), MojoError> {
    match value {
        serde_json::Value::String(text) => {
            let end = mojo_live_log_record::nested_string_clip_end(text)?;
            if end < text.len() {
                text.truncate(end);
                text.push_str(" …[truncated]");
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                materialize_nested_string_clips(value)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                materialize_nested_string_clips(value)?;
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
    Ok(())
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
