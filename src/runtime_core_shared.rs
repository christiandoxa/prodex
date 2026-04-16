use super::*;

pub(super) fn runtime_proxy_log_dir() -> PathBuf {
    env::var_os("PRODEX_RUNTIME_LOG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| runtime_policy_runtime().and_then(|policy| policy.log_dir))
        .unwrap_or_else(env::temp_dir)
}

pub(super) fn runtime_proxy_log_format() -> RuntimeLogFormat {
    env::var("PRODEX_RUNTIME_LOG_FORMAT")
        .ok()
        .and_then(|value| RuntimeLogFormat::parse(&value))
        .or_else(|| runtime_policy_runtime().and_then(|policy| policy.log_format))
        .unwrap_or(RuntimeLogFormat::Text)
}

pub(super) fn create_runtime_proxy_log_path() -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = RUNTIME_PROXY_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dir = runtime_proxy_log_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join(format!(
        "{RUNTIME_PROXY_LOG_FILE_PREFIX}-{}-{millis}-{sequence}.log",
        std::process::id()
    ))
}

pub(super) fn runtime_proxy_latest_log_pointer_path() -> PathBuf {
    runtime_proxy_log_dir().join(RUNTIME_PROXY_LATEST_LOG_POINTER)
}

pub(super) fn initialize_runtime_proxy_log_path() -> PathBuf {
    cleanup_runtime_proxy_log_housekeeping();
    let log_path = create_runtime_proxy_log_path();
    let _ = fs::write(
        runtime_proxy_latest_log_pointer_path(),
        format!("{}\n", log_path.display()),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy log initialized pid={} cwd={}",
            std::process::id(),
            std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string())
        ),
    );
    log_path
}

pub(super) fn runtime_proxy_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.worker_count),
        (parallelism.saturating_mul(4)).clamp(8, 32),
    )
    .clamp(1, 64)
}

pub(super) fn runtime_proxy_long_lived_worker_count_default(parallelism: usize) -> usize {
    parallelism.saturating_mul(8).clamp(32, 128)
}

pub(super) fn runtime_proxy_long_lived_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.long_lived_worker_count),
        runtime_proxy_long_lived_worker_count_default(parallelism),
    )
    .clamp(1, 256)
}

pub(super) fn runtime_probe_refresh_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.probe_refresh_worker_count),
        parallelism.clamp(2, 4),
    )
    .clamp(1, 8)
}

pub(super) fn runtime_proxy_async_worker_count_default(parallelism: usize) -> usize {
    parallelism.saturating_mul(2).clamp(2, 8)
}

pub(super) fn runtime_proxy_async_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.async_worker_count),
        runtime_proxy_async_worker_count_default(parallelism),
    )
    .clamp(2, 8)
}

pub(super) fn runtime_proxy_long_lived_queue_capacity(worker_count: usize) -> usize {
    let default_capacity = worker_count.saturating_mul(8).clamp(128, 1024);
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.long_lived_queue_capacity),
        default_capacity,
    )
    .max(1)
}

pub(super) fn runtime_proxy_active_request_limit_default(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    worker_count
        .saturating_add(long_lived_worker_count.saturating_mul(3))
        .clamp(64, 512)
}

pub(super) fn runtime_proxy_active_request_limit(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT",
        runtime_policy_proxy().and_then(|policy| policy.active_request_limit),
        runtime_proxy_active_request_limit_default(worker_count, long_lived_worker_count),
    )
    .max(1)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeProxyLaneLimits {
    pub(super) responses: usize,
    pub(super) compact: usize,
    pub(super) websocket: usize,
    pub(super) standard: usize,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeProxyLaneAdmission {
    pub(super) responses_active: Arc<AtomicUsize>,
    pub(super) compact_active: Arc<AtomicUsize>,
    pub(super) websocket_active: Arc<AtomicUsize>,
    pub(super) standard_active: Arc<AtomicUsize>,
    pub(super) wait: Arc<(Mutex<()>, Condvar)>,
    pub(super) inflight_release_revision: Arc<AtomicU64>,
    pub(super) limits: RuntimeProxyLaneLimits,
}

impl RuntimeProxyLaneAdmission {
    pub(super) fn new(limits: RuntimeProxyLaneLimits) -> Self {
        Self {
            responses_active: Arc::new(AtomicUsize::new(0)),
            compact_active: Arc::new(AtomicUsize::new(0)),
            websocket_active: Arc::new(AtomicUsize::new(0)),
            standard_active: Arc::new(AtomicUsize::new(0)),
            wait: Arc::new((Mutex::new(()), Condvar::new())),
            inflight_release_revision: Arc::new(AtomicU64::new(0)),
            limits,
        }
    }

    pub(super) fn active_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicUsize> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_active),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_active),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_active),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_active),
        }
    }

    pub(super) fn limit(&self, lane: RuntimeRouteKind) -> usize {
        match lane {
            RuntimeRouteKind::Responses => self.limits.responses,
            RuntimeRouteKind::Compact => self.limits.compact,
            RuntimeRouteKind::Websocket => self.limits.websocket,
            RuntimeRouteKind::Standard => self.limits.standard,
        }
    }
}

pub(super) fn runtime_proxy_lane_limits(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
) -> RuntimeProxyLaneLimits {
    let global_limit = global_limit.max(1);
    RuntimeProxyLaneLimits {
        responses: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.responses_active_limit),
            (global_limit.saturating_mul(3) / 4).clamp(4, global_limit),
        )
        .min(global_limit)
        .max(1),
        compact: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.compact_active_limit),
            (global_limit / 4).clamp(2, 6).min(global_limit),
        )
        .min(global_limit)
        .max(1),
        websocket: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.websocket_active_limit),
            long_lived_worker_count.clamp(2, global_limit),
        )
        .min(global_limit)
        .max(1),
        standard: usize_override_with_policy(
            "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
            runtime_policy_proxy().and_then(|policy| policy.standard_active_limit),
            (worker_count / 2).clamp(2, 8).min(global_limit),
        )
        .min(global_limit)
        .max(1),
    }
}

pub(super) fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

pub(super) fn runtime_proxy_log_event(message: &str) -> Option<&str> {
    message
        .split_whitespace()
        .find(|token| !token.contains('='))
        .filter(|token| !token.is_empty())
}

pub(super) fn runtime_proxy_log_to_path(log_path: &Path, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let sanitized = message.replace(['\r', '\n'], " ");
    let line = match runtime_proxy_log_format() {
        RuntimeLogFormat::Text => format!("[{timestamp}] {sanitized}\n"),
        RuntimeLogFormat::Json => {
            let mut value = serde_json::Map::new();
            value.insert(
                "timestamp".to_string(),
                serde_json::Value::String(timestamp.to_string()),
            );
            value.insert(
                "pid".to_string(),
                serde_json::Value::Number(std::process::id().into()),
            );
            value.insert(
                "message".to_string(),
                serde_json::Value::String(sanitized.clone()),
            );
            if let Some(event) = runtime_proxy_log_event(&sanitized) {
                value.insert(
                    "event".to_string(),
                    serde_json::Value::String(event.to_string()),
                );
            }
            let fields = runtime_proxy_log_fields(&sanitized);
            if !fields.is_empty() {
                value.insert(
                    "fields".to_string(),
                    serde_json::Value::Object(
                        fields
                            .into_iter()
                            .map(|(key, value)| (key, serde_json::Value::String(value)))
                            .collect(),
                    ),
                );
            }
            match serde_json::to_string(&serde_json::Value::Object(value)) {
                Ok(serialized) => format!("{serialized}\n"),
                Err(_) => format!("[{timestamp}] {sanitized}\n"),
            }
        }
    };
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

#[derive(Debug)]
pub(super) struct JsonFileLock {
    pub(super) file: fs::File,
}

impl Drop for JsonFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
