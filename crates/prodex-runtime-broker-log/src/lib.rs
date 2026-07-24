use prodex_runtime_broker::{
    RuntimeBrokerContinuityFailureReasonMetrics,
    runtime_broker_continuity_failure_reason_metrics_from_log_bytes,
    runtime_broker_merge_continuity_failure_reason_metrics,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

pub const DEFAULT_RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE_LIMIT: usize = 16;
const RUNTIME_BROKER_LOG_LINE_MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeLogFingerprint {
    len: u64,
    modified_at: Duration,
}

#[derive(Debug, Clone)]
struct RuntimeBrokerContinuityFailureReasonCacheEntry {
    fingerprint: RuntimeLogFingerprint,
    parsed_len: u64,
    metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    last_used_at: u64,
}

#[derive(Debug)]
struct RuntimeBrokerContinuityFailureReasonCache {
    entries: BTreeMap<PathBuf, RuntimeBrokerContinuityFailureReasonCacheEntry>,
    next_touch: u64,
    limit: usize,
    full_rebuilds: u64,
    incremental_updates: u64,
    hits: u64,
    misses: u64,
}

impl Default for RuntimeBrokerContinuityFailureReasonCache {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_touch: 0,
            limit: DEFAULT_RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE_LIMIT,
            full_rebuilds: 0,
            incremental_updates: 0,
            hits: 0,
            misses: 0,
        }
    }
}

impl RuntimeBrokerContinuityFailureReasonCache {
    fn touch(&mut self) -> u64 {
        self.next_touch = self.next_touch.wrapping_add(1);
        self.next_touch
    }

    fn get(
        &mut self,
        log_path: &Path,
        fingerprint: &RuntimeLogFingerprint,
    ) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
        let touched_at = self.touch();
        let cached = self
            .entries
            .get_mut(log_path)
            .filter(|entry| entry.fingerprint == *fingerprint)
            .map(|entry| {
                entry.last_used_at = touched_at;
                entry.metrics.clone()
            });
        if cached.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        cached
    }

    fn store(
        &mut self,
        log_path: &Path,
        fingerprint: RuntimeLogFingerprint,
        parsed_len: u64,
        metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    ) {
        let touched_at = self.touch();
        self.entries.insert(
            log_path.to_path_buf(),
            RuntimeBrokerContinuityFailureReasonCacheEntry {
                fingerprint,
                parsed_len,
                metrics,
                last_used_at: touched_at,
            },
        );
        while self.entries.len() > self.limit {
            let Some(oldest_path) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_at)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            self.entries.remove(&oldest_path);
        }
    }

    fn remove(&mut self, log_path: &Path) {
        self.entries.remove(log_path);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.next_touch = 0;
        self.full_rebuilds = 0;
        self.incremental_updates = 0;
        self.hits = 0;
        self.misses = 0;
    }

    fn stats(&self) -> RuntimeBrokerContinuityFailureReasonCacheStats {
        RuntimeBrokerContinuityFailureReasonCacheStats {
            full_rebuilds: self.full_rebuilds,
            incremental_updates: self.incremental_updates,
            hits: self.hits,
            misses: self.misses,
            entries: self.entries.len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerContinuityFailureReasonCacheStats {
    pub full_rebuilds: u64,
    pub incremental_updates: u64,
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

static RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE: OnceLock<
    Mutex<RuntimeBrokerContinuityFailureReasonCache>,
> = OnceLock::new();

fn runtime_broker_continuity_failure_reason_cache()
-> &'static Mutex<RuntimeBrokerContinuityFailureReasonCache> {
    RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE
        .get_or_init(|| Mutex::new(RuntimeBrokerContinuityFailureReasonCache::default()))
}

fn runtime_log_fingerprint(metadata: &fs::Metadata) -> Option<RuntimeLogFingerprint> {
    let modified_at = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(RuntimeLogFingerprint {
        len: metadata.len(),
        modified_at,
    })
}

fn runtime_broker_continuity_failure_reason_metrics_from_log_range(
    log_path: &Path,
    start: u64,
    end: u64,
) -> Option<(RuntimeBrokerContinuityFailureReasonMetrics, u64)> {
    if start > end {
        return None;
    }
    let mut log = fs::File::open(log_path).ok()?;
    if start > 0 {
        log.seek(SeekFrom::Start(start)).ok()?;
    }
    let mut reader = BufReader::new(log.take(end - start));
    let mut metrics = RuntimeBrokerContinuityFailureReasonMetrics::default();
    let mut line = Vec::new();
    let mut line_oversized = false;
    let mut consumed = 0_u64;
    let mut parsed_len = start;

    loop {
        let available = reader.fill_buf().ok()?;
        if available.is_empty() {
            break;
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        let line_complete = available.get(take.saturating_sub(1)) == Some(&b'\n');
        if !line_oversized {
            if line.len().saturating_add(take) <= RUNTIME_BROKER_LOG_LINE_MAX_BYTES {
                line.extend_from_slice(&available[..take]);
            } else {
                line.clear();
                line_oversized = true;
            }
        }
        reader.consume(take);
        consumed = consumed.checked_add(u64::try_from(take).ok()?)?;
        if line_complete {
            if !line_oversized {
                runtime_broker_merge_continuity_failure_reason_metrics(
                    &mut metrics,
                    runtime_broker_continuity_failure_reason_metrics_from_log_bytes(&line),
                );
            }
            line.clear();
            line_oversized = false;
            parsed_len = start.checked_add(consumed)?;
        }
    }

    Some((metrics, parsed_len))
}

pub fn runtime_broker_continuity_failure_reason_metrics_from_log_file(
    log_path: &Path,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    let end = fs::metadata(log_path).ok()?.len();
    runtime_broker_continuity_failure_reason_metrics_from_log_range(log_path, 0, end)
        .map(|(metrics, _)| metrics)
}

pub fn runtime_broker_cached_continuity_failure_reason_metrics(
    log_path: &Path,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    let Ok(metadata) = fs::metadata(log_path) else {
        runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(log_path);
        return RuntimeBrokerContinuityFailureReasonMetrics::default();
    };
    let log_len = metadata.len();
    let fingerprint = runtime_log_fingerprint(&metadata);

    let append_base = if let Some(fingerprint) = fingerprint.as_ref() {
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(metrics) = cache.get(log_path, fingerprint) {
            return metrics;
        }
        cache.entries.get(log_path).cloned().filter(|entry| {
            entry.fingerprint.len < fingerprint.len
                && entry.fingerprint.modified_at <= fingerprint.modified_at
        })
    } else {
        None
    };

    if let (Some(fingerprint), Some(base)) = (fingerprint.as_ref(), append_base)
        && let Some((delta, parsed_len)) =
            runtime_broker_continuity_failure_reason_metrics_from_log_range(
                log_path,
                base.parsed_len,
                fingerprint.len,
            )
    {
        let mut metrics = base.metrics.clone();
        runtime_broker_merge_continuity_failure_reason_metrics(&mut metrics, delta);
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.store(log_path, fingerprint.clone(), parsed_len, metrics.clone());
        cache.incremental_updates += 1;
        return metrics;
    }

    let Some((metrics, parsed_len)) =
        runtime_broker_continuity_failure_reason_metrics_from_log_range(log_path, 0, log_len)
    else {
        runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(log_path);
        return RuntimeBrokerContinuityFailureReasonMetrics::default();
    };

    if let Some(fingerprint) = fingerprint {
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.store(log_path, fingerprint, parsed_len, metrics.clone());
        cache.full_rebuilds += 1;
    }
    metrics
}

#[doc(hidden)]
pub fn clear_runtime_broker_continuity_failure_reason_cache_for_test() {
    runtime_broker_continuity_failure_reason_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[doc(hidden)]
pub fn runtime_broker_continuity_failure_reason_cache_stats_for_test()
-> RuntimeBrokerContinuityFailureReasonCacheStats {
    runtime_broker_continuity_failure_reason_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .stats()
}

pub const DEFAULT_RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS_STORE_LIMIT: usize = 16;

#[derive(Debug, Clone)]
struct RuntimeProxyContinuityFailureReasonMetricsEntry {
    baseline_metrics: Option<RuntimeBrokerContinuityFailureReasonMetrics>,
    baseline_fingerprint: Option<RuntimeLogFingerprint>,
    live_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    last_observed_fingerprint: Option<RuntimeLogFingerprint>,
    last_used_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProxyContinuityFailureReasonMetricsSnapshot {
    pub baseline_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    pub live_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Default)]
struct RuntimeProxyContinuityFailureReasonMetricsStore {
    entries: BTreeMap<PathBuf, RuntimeProxyContinuityFailureReasonMetricsEntry>,
    next_touch: u64,
}

enum RuntimeProxyContinuityFailureReasonSnapshotState {
    Ready(RuntimeProxyContinuityFailureReasonMetricsSnapshot),
    LoadBaseline(RuntimeLogFingerprint),
}

static RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS: OnceLock<
    Mutex<RuntimeProxyContinuityFailureReasonMetricsStore>,
> = OnceLock::new();

fn runtime_proxy_continuity_failure_reason_metrics_store()
-> &'static Mutex<RuntimeProxyContinuityFailureReasonMetricsStore> {
    RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS
        .get_or_init(|| Mutex::new(RuntimeProxyContinuityFailureReasonMetricsStore::default()))
}

fn increment_runtime_proxy_reason_metric(map: &mut BTreeMap<String, usize>, reason: &str) {
    *map.entry(reason.to_string()).or_insert(0) += 1;
}

fn record_runtime_proxy_reason_metric(
    metrics: &mut RuntimeBrokerContinuityFailureReasonMetrics,
    event: &str,
    reason: &str,
) -> bool {
    match event {
        "chain_retried_owner" => {
            increment_runtime_proxy_reason_metric(&mut metrics.chain_retried_owner, reason);
        }
        "chain_dead_upstream_confirmed" => {
            increment_runtime_proxy_reason_metric(
                &mut metrics.chain_dead_upstream_confirmed,
                reason,
            );
        }
        "stale_continuation" => {
            increment_runtime_proxy_reason_metric(&mut metrics.stale_continuation, reason);
        }
        _ => return false,
    }
    true
}

impl RuntimeProxyContinuityFailureReasonMetricsStore {
    fn touch(&mut self) -> u64 {
        self.next_touch = self.next_touch.wrapping_add(1);
        self.next_touch
    }

    fn current_fingerprint(log_path: &Path) -> Option<RuntimeLogFingerprint> {
        fs::metadata(log_path)
            .ok()
            .and_then(|metadata| runtime_log_fingerprint(&metadata))
    }

    fn log_rotated(
        current: Option<&RuntimeLogFingerprint>,
        previous: Option<&RuntimeLogFingerprint>,
    ) -> bool {
        match (current, previous) {
            (Some(current), Some(previous)) => {
                current.len < previous.len || current.modified_at < previous.modified_at
            }
            _ => false,
        }
    }

    fn new_entry(
        fingerprint: Option<RuntimeLogFingerprint>,
        touched_at: u64,
    ) -> RuntimeProxyContinuityFailureReasonMetricsEntry {
        let baseline_metrics = fingerprint
            .is_none()
            .then(RuntimeBrokerContinuityFailureReasonMetrics::default);
        RuntimeProxyContinuityFailureReasonMetricsEntry {
            baseline_metrics,
            baseline_fingerprint: fingerprint.clone(),
            live_metrics: RuntimeBrokerContinuityFailureReasonMetrics::default(),
            last_observed_fingerprint: fingerprint,
            last_used_at: touched_at,
        }
    }

    fn enforce_limit(&mut self, keep_log_path: Option<&Path>) {
        while self.entries.len()
            > DEFAULT_RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS_STORE_LIMIT
        {
            let oldest_path = self
                .entries
                .iter()
                .filter(|(path, _)| Some(path.as_path()) != keep_log_path)
                .min_by_key(|(_, entry)| entry.last_used_at)
                .or_else(|| {
                    self.entries
                        .iter()
                        .min_by_key(|(_, entry)| entry.last_used_at)
                })
                .map(|(path, _)| path.clone());
            let Some(oldest_path) = oldest_path else {
                break;
            };
            self.entries.remove(&oldest_path);
        }
    }

    fn record(
        &mut self,
        log_path: &Path,
        current_fingerprint: Option<RuntimeLogFingerprint>,
        event: &str,
        reason: &str,
    ) {
        let touched_at = self.touch();
        let needs_reset = self.entries.get(log_path).is_none_or(|entry| {
            Self::log_rotated(
                current_fingerprint.as_ref(),
                entry.last_observed_fingerprint.as_ref(),
            )
        });
        if needs_reset {
            self.entries.insert(
                log_path.to_path_buf(),
                Self::new_entry(current_fingerprint.clone(), touched_at),
            );
        }
        if let Some(entry) = self.entries.get_mut(log_path) {
            entry.last_used_at = touched_at;
            if current_fingerprint.is_some() {
                entry.last_observed_fingerprint = current_fingerprint;
            }
            if !record_runtime_proxy_reason_metric(&mut entry.live_metrics, event, reason) {
                return;
            }
        }
        self.enforce_limit(Some(log_path));
    }

    fn snapshot_state(
        &mut self,
        log_path: &Path,
        current_fingerprint: Option<RuntimeLogFingerprint>,
    ) -> Option<RuntimeProxyContinuityFailureReasonSnapshotState> {
        if current_fingerprint.is_none() {
            if self
                .entries
                .get(log_path)
                .is_some_and(|entry| entry.last_observed_fingerprint.is_some())
            {
                self.entries.remove(log_path);
                return None;
            }
            let touched_at = self.touch();
            let entry = self.entries.get_mut(log_path)?;
            entry.last_used_at = touched_at;
            return Some(RuntimeProxyContinuityFailureReasonSnapshotState::Ready(
                RuntimeProxyContinuityFailureReasonMetricsSnapshot {
                    baseline_metrics: entry.baseline_metrics.clone().unwrap_or_default(),
                    live_metrics: entry.live_metrics.clone(),
                },
            ));
        }
        let touched_at = self.touch();
        if self.entries.get(log_path).is_some_and(|entry| {
            Self::log_rotated(
                current_fingerprint.as_ref(),
                entry.last_observed_fingerprint.as_ref(),
            )
        }) {
            self.entries.remove(log_path);
            return None;
        }
        let entry = self.entries.get_mut(log_path)?;
        entry.last_used_at = touched_at;
        entry.last_observed_fingerprint = current_fingerprint;
        match entry.baseline_metrics.as_ref() {
            Some(baseline_metrics) => {
                Some(RuntimeProxyContinuityFailureReasonSnapshotState::Ready(
                    RuntimeProxyContinuityFailureReasonMetricsSnapshot {
                        baseline_metrics: baseline_metrics.clone(),
                        live_metrics: entry.live_metrics.clone(),
                    },
                ))
            }
            None => entry
                .baseline_fingerprint
                .clone()
                .map(RuntimeProxyContinuityFailureReasonSnapshotState::LoadBaseline),
        }
    }

    fn install_baseline(
        &mut self,
        log_path: &Path,
        fingerprint: &RuntimeLogFingerprint,
        metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    ) -> Option<RuntimeProxyContinuityFailureReasonMetricsSnapshot> {
        let entry = self.entries.get_mut(log_path)?;
        if entry.baseline_fingerprint.as_ref() != Some(fingerprint) {
            return None;
        }
        entry.baseline_metrics = Some(metrics.clone());
        Some(RuntimeProxyContinuityFailureReasonMetricsSnapshot {
            baseline_metrics: metrics,
            live_metrics: entry.live_metrics.clone(),
        })
    }

    fn remove(&mut self, log_path: &Path) {
        self.entries.remove(log_path);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.next_touch = 0;
    }
}

pub fn runtime_proxy_record_continuity_failure_reason_for_log_path(
    log_path: &Path,
    event: &str,
    reason: &str,
) {
    let supported = matches!(
        event,
        "chain_retried_owner" | "chain_dead_upstream_confirmed" | "stale_continuation"
    );
    if !supported {
        return;
    }
    let current_fingerprint =
        RuntimeProxyContinuityFailureReasonMetricsStore::current_fingerprint(log_path);
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .record(log_path, current_fingerprint, event, reason);
}

pub fn runtime_proxy_continuity_failure_reason_metrics_snapshot(
    log_path: &Path,
) -> Option<RuntimeProxyContinuityFailureReasonMetricsSnapshot> {
    for _ in 0..2 {
        let current_fingerprint =
            RuntimeProxyContinuityFailureReasonMetricsStore::current_fingerprint(log_path);
        let state = runtime_proxy_continuity_failure_reason_metrics_store()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state(log_path, current_fingerprint)?;
        match state {
            RuntimeProxyContinuityFailureReasonSnapshotState::Ready(snapshot) => {
                return Some(snapshot);
            }
            RuntimeProxyContinuityFailureReasonSnapshotState::LoadBaseline(fingerprint) => {
                let (metrics, _) = runtime_broker_continuity_failure_reason_metrics_from_log_range(
                    log_path,
                    0,
                    fingerprint.len,
                )?;
                let current_fingerprint =
                    RuntimeProxyContinuityFailureReasonMetricsStore::current_fingerprint(log_path);
                if current_fingerprint.is_none()
                    || RuntimeProxyContinuityFailureReasonMetricsStore::log_rotated(
                        current_fingerprint.as_ref(),
                        Some(&fingerprint),
                    )
                {
                    return None;
                }
                if let Some(snapshot) = runtime_proxy_continuity_failure_reason_metrics_store()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .install_baseline(log_path, &fingerprint, metrics)
                {
                    return Some(snapshot);
                }
            }
        }
    }
    None
}

pub fn clear_runtime_proxy_continuity_failure_reason_metrics(log_path: &Path) {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(log_path);
}

#[doc(hidden)]
pub fn clear_all_runtime_proxy_continuity_failure_reason_metrics_for_test() {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[doc(hidden)]
pub fn runtime_proxy_continuity_failure_reason_metrics_store_entry_count_for_test() -> usize {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entries
        .len()
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
