use prodex_runtime_broker::{
    RuntimeBrokerContinuityFailureReasonMetrics,
    runtime_broker_continuity_failure_reason_metrics_from_log_bytes,
    runtime_broker_merge_continuity_failure_reason_metrics,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

pub const DEFAULT_RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeBrokerContinuityFailureReasonCacheFingerprint {
    len: u64,
    modified_at: Duration,
}

#[derive(Debug, Clone)]
struct RuntimeBrokerContinuityFailureReasonCacheEntry {
    fingerprint: RuntimeBrokerContinuityFailureReasonCacheFingerprint,
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
        fingerprint: &RuntimeBrokerContinuityFailureReasonCacheFingerprint,
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
        fingerprint: RuntimeBrokerContinuityFailureReasonCacheFingerprint,
        metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    ) {
        let touched_at = self.touch();
        self.entries.insert(
            log_path.to_path_buf(),
            RuntimeBrokerContinuityFailureReasonCacheEntry {
                fingerprint,
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

fn runtime_broker_continuity_failure_reason_cache_fingerprint(
    metadata: &fs::Metadata,
) -> Option<RuntimeBrokerContinuityFailureReasonCacheFingerprint> {
    let modified_at = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(RuntimeBrokerContinuityFailureReasonCacheFingerprint {
        len: metadata.len(),
        modified_at,
    })
}

fn runtime_broker_continuity_failure_reason_metrics_from_log_range(
    log_path: &Path,
    start: u64,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    let mut log = fs::File::open(log_path).ok()?;
    if start > 0 {
        log.seek(SeekFrom::Start(start)).ok()?;
    }
    let mut buffer = Vec::new();
    log.read_to_end(&mut buffer).ok()?;
    Some(runtime_broker_continuity_failure_reason_metrics_from_log_bytes(&buffer))
}

pub fn runtime_broker_continuity_failure_reason_metrics_from_log_file(
    log_path: &Path,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    runtime_broker_continuity_failure_reason_metrics_from_log_range(log_path, 0)
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
    let fingerprint = runtime_broker_continuity_failure_reason_cache_fingerprint(&metadata);

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
        && let Some(delta) = runtime_broker_continuity_failure_reason_metrics_from_log_range(
            log_path,
            base.fingerprint.len,
        )
    {
        let mut metrics = base.metrics.clone();
        runtime_broker_merge_continuity_failure_reason_metrics(&mut metrics, delta);
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.store(log_path, fingerprint.clone(), metrics.clone());
        cache.incremental_updates += 1;
        return metrics;
    }

    let Some(metrics) =
        runtime_broker_continuity_failure_reason_metrics_from_log_range(log_path, 0)
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
        cache.store(log_path, fingerprint, metrics.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_log_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-runtime-broker-log-{name}-{}-{nanos}.log",
            std::process::id()
        ))
    }

    #[test]
    fn continuity_failure_reason_metrics_follow_runtime_log_summary() {
        clear_runtime_broker_continuity_failure_reason_cache_for_test();
        let log_path = temp_log_path("reasons");
        fs::write(
            &log_path,
            concat!(
                "[2026-04-22 10:00:00.000 +00:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_retried_owner profile=second previous_response_id=resp-second delay_ms=20 reason=previous_response_not_found_locked_affinity via=-\n",
                "[2026-04-22 10:00:00.001 +00:00] request=3 websocket_session=sess-1 stale_continuation reason=websocket_reuse_watchdog_locked_affinity profile=second event=timeout\n",
                "[2026-04-22 10:00:00.002 +00:00] request=4 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
                "[2026-04-22 10:00:00.003 +00:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_dead_upstream_confirmed profile=second previous_response_id=resp-second reason=previous_response_not_found_locked_affinity via=- event=-\n"
            ),
        )
        .expect("test log should write");

        let metrics = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);

        assert_eq!(
            metrics.chain_retried_owner,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
        );
        assert_eq!(
            metrics.chain_dead_upstream_confirmed,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
        );
        assert_eq!(
            metrics.stale_continuation,
            BTreeMap::from([
                ("previous_response_not_found".to_string(), 1),
                ("websocket_reuse_watchdog_locked_affinity".to_string(), 1),
            ])
        );

        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn continuity_failure_reason_metrics_reuse_cached_summary_for_unchanged_log() {
        clear_runtime_broker_continuity_failure_reason_cache_for_test();
        let log_path = temp_log_path("cache-hit");
        fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

        let first = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats_for_test(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 0,
                hits: 0,
                misses: 1,
                entries: 1,
            }
        );

        let second = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
        assert_eq!(second, first);
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats_for_test(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 0,
                hits: 1,
                misses: 1,
                entries: 1,
            }
        );

        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn continuity_failure_reason_metrics_incrementally_update_when_log_appends() {
        clear_runtime_broker_continuity_failure_reason_cache_for_test();
        let log_path = temp_log_path("cache-invalidate");
        fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=5 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

        let first = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
        assert_eq!(
            first.stale_continuation,
            BTreeMap::from([("previous_response_not_found".to_string(), 1)])
        );
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats_for_test(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 0,
                hits: 0,
                misses: 1,
                entries: 1,
            }
        );

        let mut log = fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("test log should open");
        writeln!(
            log,
            "[2026-04-22 10:00:00.001 +00:00] request=5 transport=websocket route=websocket websocket_session=sess-5 chain_retried_owner profile=main previous_response_id=resp-5 delay_ms=20 reason=previous_response_not_found_locked_affinity via=-"
        )
        .expect("test log should append");
        drop(log);

        let second = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
        assert_eq!(
            second.chain_retried_owner,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
        );
        assert_eq!(
            second.stale_continuation,
            BTreeMap::from([("previous_response_not_found".to_string(), 1)])
        );
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats_for_test(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 1,
                hits: 0,
                misses: 2,
                entries: 1,
            }
        );

        fs::remove_file(&log_path).expect("test log should clean up");
    }
}
