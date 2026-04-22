use super::*;

const RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS_STORE_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeProxyContinuityFailureReasonMetricsFingerprint {
    len: u64,
    modified_at: Duration,
}

#[derive(Debug, Clone)]
struct RuntimeProxyContinuityFailureReasonMetricsEntry {
    baseline_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    live_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    last_observed_fingerprint: Option<RuntimeProxyContinuityFailureReasonMetricsFingerprint>,
    last_used_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RuntimeProxyContinuityFailureReasonMetricsSnapshot {
    pub(crate) baseline_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    pub(crate) live_metrics: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Default)]
struct RuntimeProxyContinuityFailureReasonMetricsStore {
    entries: BTreeMap<PathBuf, RuntimeProxyContinuityFailureReasonMetricsEntry>,
    next_touch: u64,
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

    fn fingerprint_from_metadata(
        metadata: &fs::Metadata,
    ) -> Option<RuntimeProxyContinuityFailureReasonMetricsFingerprint> {
        let modified_at = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(RuntimeProxyContinuityFailureReasonMetricsFingerprint {
            len: metadata.len(),
            modified_at,
        })
    }

    fn current_fingerprint(
        log_path: &Path,
    ) -> Option<RuntimeProxyContinuityFailureReasonMetricsFingerprint> {
        fs::metadata(log_path)
            .ok()
            .and_then(|metadata| Self::fingerprint_from_metadata(&metadata))
    }

    fn log_rotated(
        current: Option<&RuntimeProxyContinuityFailureReasonMetricsFingerprint>,
        previous: Option<&RuntimeProxyContinuityFailureReasonMetricsFingerprint>,
    ) -> bool {
        match (current, previous) {
            (Some(current), Some(previous)) => {
                current.len < previous.len || current.modified_at < previous.modified_at
            }
            _ => false,
        }
    }

    fn new_entry(
        log_path: &Path,
        fingerprint: Option<RuntimeProxyContinuityFailureReasonMetricsFingerprint>,
        touched_at: u64,
    ) -> RuntimeProxyContinuityFailureReasonMetricsEntry {
        RuntimeProxyContinuityFailureReasonMetricsEntry {
            baseline_metrics: runtime_broker_continuity_failure_reason_metrics_from_log_file(
                log_path,
            )
            .unwrap_or_default(),
            live_metrics: RuntimeBrokerContinuityFailureReasonMetrics::default(),
            last_observed_fingerprint: fingerprint,
            last_used_at: touched_at,
        }
    }

    fn evict_stale_paths(&mut self, keep_log_path: Option<&Path>) {
        self.entries
            .retain(|path, _| Some(path.as_path()) == keep_log_path || fs::metadata(path).is_ok());
    }

    fn enforce_limit(&mut self, keep_log_path: Option<&Path>) {
        while self.entries.len() > RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS_STORE_LIMIT {
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

    fn record(&mut self, log_path: &Path, event: &str, reason: &str) {
        self.evict_stale_paths(Some(log_path));
        let touched_at = self.touch();
        let current_fingerprint = Self::current_fingerprint(log_path);
        let needs_reset = self.entries.get(log_path).is_none_or(|entry| {
            Self::log_rotated(
                current_fingerprint.as_ref(),
                entry.last_observed_fingerprint.as_ref(),
            )
        });
        if needs_reset {
            self.entries.insert(
                log_path.to_path_buf(),
                Self::new_entry(log_path, current_fingerprint.clone(), touched_at),
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

    fn snapshot(
        &mut self,
        log_path: &Path,
    ) -> Option<RuntimeProxyContinuityFailureReasonMetricsSnapshot> {
        self.evict_stale_paths(Some(log_path));
        let current_fingerprint = Self::current_fingerprint(log_path);
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
            return Some(RuntimeProxyContinuityFailureReasonMetricsSnapshot {
                baseline_metrics: entry.baseline_metrics.clone(),
                live_metrics: entry.live_metrics.clone(),
            });
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
        Some(RuntimeProxyContinuityFailureReasonMetricsSnapshot {
            baseline_metrics: entry.baseline_metrics.clone(),
            live_metrics: entry.live_metrics.clone(),
        })
    }

    fn remove(&mut self, log_path: &Path) {
        self.entries.remove(log_path);
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
        self.next_touch = 0;
    }
}

pub(crate) fn runtime_proxy_record_continuity_failure_reason(
    shared: &RuntimeRotationProxyShared,
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
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .record(&shared.log_path, event, reason);
}

pub(crate) fn runtime_proxy_continuity_failure_reason_metrics_snapshot(
    log_path: &Path,
) -> Option<RuntimeProxyContinuityFailureReasonMetricsSnapshot> {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot(log_path)
}

pub(crate) fn clear_runtime_proxy_continuity_failure_reason_metrics(log_path: &Path) {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(log_path);
}

#[cfg(test)]
pub(crate) fn clear_all_runtime_proxy_continuity_failure_reason_metrics() {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[cfg(test)]
pub(crate) fn runtime_proxy_continuity_failure_reason_metrics_store_entry_count() -> usize {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entries
        .len()
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeProxyChainLog<'a> {
    pub(crate) request_id: u64,
    pub(crate) transport: &'a str,
    pub(crate) route: &'a str,
    pub(crate) websocket_session: Option<u64>,
    pub(crate) profile_name: &'a str,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) reason: &'a str,
    pub(crate) via: Option<&'a str>,
}

pub(crate) fn runtime_proxy_log_chain_retried_owner(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyChainLog<'_>,
    delay_ms: u128,
) {
    runtime_proxy_record_continuity_failure_reason(shared, "chain_retried_owner", log.reason);
    runtime_proxy_log(
        shared,
        format!(
            "request={} transport={} route={} websocket_session={} chain_retried_owner profile={} previous_response_id={} delay_ms={delay_ms} reason={} via={}",
            log.request_id,
            log.transport,
            log.route,
            log.websocket_session
                .map(|session_id| session_id.to_string())
                .as_deref()
                .unwrap_or("-"),
            log.profile_name,
            log.previous_response_id.unwrap_or("-"),
            log.reason,
            log.via.unwrap_or("-"),
        ),
    );
}

pub(crate) fn runtime_proxy_log_chain_dead_upstream_confirmed(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyChainLog<'_>,
    event: Option<&str>,
) {
    runtime_proxy_record_continuity_failure_reason(
        shared,
        "chain_dead_upstream_confirmed",
        log.reason,
    );
    runtime_proxy_log(
        shared,
        format!(
            "request={} transport={} route={} websocket_session={} chain_dead_upstream_confirmed profile={} previous_response_id={} reason={} via={} event={}",
            log.request_id,
            log.transport,
            log.route,
            log.websocket_session
                .map(|session_id| session_id.to_string())
                .as_deref()
                .unwrap_or("-"),
            log.profile_name,
            log.previous_response_id.unwrap_or("-"),
            log.reason,
            log.via.unwrap_or("-"),
            event.unwrap_or("-"),
        ),
    );
}
