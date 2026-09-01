use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES: usize = 512;
pub const DEFAULT_RUNTIME_LIVE_LOG_MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_RUNTIME_LIVE_LOG_PATHS: usize = 64;
const MAX_RUNTIME_LIVE_LOG_ENTRIES: usize = 2048;
const MAX_RUNTIME_LIVE_LOG_BYTES: usize = 8 * 1024 * 1024;
const MAX_RUNTIME_LIVE_LOG_LINE_BYTES: usize = 128 * 1024;

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
    pub(super) fn append(&self, path: &Path, line: &str) {
        let Ok(mut state) = self.state.try_lock() else {
            return;
        };
        let line = bounded_live_log_line(line);
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
        let mut entries = bucket
            .entries
            .iter()
            .filter(|entry| entry.sequence > after)
            .cloned()
            .collect::<Vec<_>>();
        let keep = limit.min(DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES);
        if entries.len() > keep {
            entries.drain(..entries.len() - keep);
        }
        RuntimeLiveLogSnapshot {
            cursor: state.next_sequence,
            dropped: bucket.dropped,
            entries,
        }
    }
}

fn bounded_live_log_line(line: &str) -> String {
    if line.len() <= MAX_RUNTIME_LIVE_LOG_LINE_BYTES {
        return line.to_string();
    }

    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(line.trim_end()) {
        clip_json_strings(&mut value, 8 * 1024);
        if let Ok(serialized) = serde_json::to_string(&value)
            && serialized.len() <= MAX_RUNTIME_LIVE_LOG_LINE_BYTES
        {
            return format!("{serialized}\n");
        }
        let mut compact = serde_json::Map::new();
        for key in ["timestamp", "pid", "event"] {
            if let Some(value) = value.get(key) {
                compact.insert(key.to_string(), value.clone());
            }
        }
        compact.insert(
            "message".to_string(),
            serde_json::Value::String("[live log record truncated]".to_string()),
        );
        if let Ok(serialized) = serde_json::to_string(&compact) {
            return format!("{serialized}\n");
        }
    }

    let end = line
        .char_indices()
        .take_while(|(index, _)| *index < MAX_RUNTIME_LIVE_LOG_LINE_BYTES.saturating_sub(32))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    format!("{} …[truncated]\n", &line[..end])
}

fn clip_json_strings(value: &mut serde_json::Value, max_bytes: usize) {
    match value {
        serde_json::Value::String(text) if text.len() > max_bytes => {
            let end = text
                .char_indices()
                .take_while(|(index, _)| *index < max_bytes)
                .map(|(index, ch)| index + ch.len_utf8())
                .last()
                .unwrap_or(0);
            text.truncate(end);
            text.push_str(" …[truncated]");
        }
        serde_json::Value::Array(values) => {
            for value in values {
                clip_json_strings(value, max_bytes);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                clip_json_strings(value, max_bytes);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
        serde_json::Value::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_log_store_is_bounded_and_keeps_complete_lines() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");

        for _ in 0..(DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES + 10) {
            store.append(path, "event\n");
        }

        let snapshot = store.snapshot_after(path, 0, usize::MAX);
        assert_eq!(snapshot.entries.len(), DEFAULT_RUNTIME_LIVE_LOG_MAX_ENTRIES);
        assert!(snapshot.entries.iter().all(|entry| entry.line == "event\n"));
        assert_eq!(snapshot.dropped, 10);
    }

    #[test]
    fn million_live_events_do_not_grow_memory_without_a_disk_sink() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");

        for _ in 0..1_000_000 {
            store.append(path, "load profile busy\n");
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
    fn oversized_json_line_is_clipped_without_breaking_json() {
        let store = RuntimeLiveLogStore::default();
        let path = Path::new("runtime.log");
        let line = format!(
            "{{\"event\":\"payload\",\"message\":\"{}\"}}\n",
            "x".repeat(MAX_RUNTIME_LIVE_LOG_LINE_BYTES)
        );
        store.append(path, &line);

        let snapshot = store.snapshot_after(path, 0, 1);
        let stored = &snapshot.entries[0].line;
        assert!(stored.len() <= MAX_RUNTIME_LIVE_LOG_LINE_BYTES);
        assert!(serde_json::from_str::<serde_json::Value>(stored).is_ok());
    }
}
