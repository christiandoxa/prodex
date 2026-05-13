use super::{
    runtime_allocator_trim_best_effort, runtime_proxy_log_to_path,
    worker_spawn::spawn_runtime_background_worker_or_panic,
};
use anyhow::{Context, Result};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

type RuntimeSmartContextTokenCalibrationSave =
    Box<dyn FnOnce(&Path) -> Result<()> + Send + 'static>;

struct RuntimeSmartContextTokenCalibrationSaveJob {
    path: PathBuf,
    log_path: PathBuf,
    reason: String,
    sample_count: usize,
    queued_at: Instant,
    ready_at: Instant,
    save: RuntimeSmartContextTokenCalibrationSave,
}

struct RuntimeSmartContextTokenCalibrationSaveQueue {
    pending: Mutex<BTreeMap<PathBuf, RuntimeSmartContextTokenCalibrationSaveJob>>,
    wake: Condvar,
}

static RUNTIME_SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_QUEUE: OnceLock<
    Arc<RuntimeSmartContextTokenCalibrationSaveQueue>,
> = OnceLock::new();

impl prodex_runtime_state::RuntimeScheduledSaveJob for RuntimeSmartContextTokenCalibrationSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

pub(crate) fn schedule_runtime_smart_context_token_calibration_save_job(
    path: PathBuf,
    log_path: PathBuf,
    reason: String,
    sample_count: usize,
    delay_ms: u64,
    save: impl FnOnce(&Path) -> Result<()> + Send + 'static,
) -> usize {
    let queue = runtime_smart_context_token_calibration_save_queue();
    let queued_at = Instant::now();
    let ready_at = queued_at + Duration::from_millis(delay_ms);
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        path.clone(),
        RuntimeSmartContextTokenCalibrationSaveJob {
            path,
            log_path,
            reason,
            sample_count,
            queued_at,
            ready_at,
            save: Box::new(save),
        },
    );
    let backlog = pending.len();
    drop(pending);
    queue.wake.notify_one();
    backlog
}

fn runtime_smart_context_token_calibration_save_queue()
-> Arc<RuntimeSmartContextTokenCalibrationSaveQueue> {
    Arc::clone(
        RUNTIME_SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_QUEUE.get_or_init(|| {
            let queue = Arc::new(RuntimeSmartContextTokenCalibrationSaveQueue {
                pending: Mutex::new(BTreeMap::new()),
                wake: Condvar::new(),
            });
            let worker_queue = Arc::clone(&queue);
            spawn_runtime_background_worker_or_panic(
                "prodex-runtime-smart-context-token-calibration-save",
                None,
                move || runtime_smart_context_token_calibration_save_worker_loop(worker_queue),
            );
            queue
        }),
    )
}

fn runtime_smart_context_token_calibration_save_worker_loop(
    queue: Arc<RuntimeSmartContextTokenCalibrationSaveQueue>,
) {
    loop {
        let job = runtime_smart_context_next_token_calibration_save_job(&queue);
        let RuntimeSmartContextTokenCalibrationSaveJob {
            path,
            log_path,
            reason,
            sample_count,
            queued_at,
            ready_at: _,
            save,
        } = job;
        match save(&path) {
            Ok(()) => runtime_proxy_log_to_path(
                &log_path,
                &runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_ok",
                    [
                        runtime_proxy_log_field("reason", reason.as_str()),
                        runtime_proxy_log_field(
                            "lag_ms",
                            queued_at.elapsed().as_millis().to_string(),
                        ),
                        runtime_proxy_log_field("samples", sample_count.to_string()),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log_to_path(
                &log_path,
                &runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_error",
                    [
                        runtime_proxy_log_field("reason", reason.as_str()),
                        runtime_proxy_log_field(
                            "lag_ms",
                            queued_at.elapsed().as_millis().to_string(),
                        ),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            ),
        }
        runtime_allocator_trim_best_effort();
    }
}

fn runtime_smart_context_next_token_calibration_save_job(
    queue: &RuntimeSmartContextTokenCalibrationSaveQueue,
) -> RuntimeSmartContextTokenCalibrationSaveJob {
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        if let Some(path) = pending
            .iter()
            .filter(|(_, job)| job.ready_at <= Instant::now())
            .map(|(path, _)| path.clone())
            .next()
        {
            if let Some(job) = pending.remove(&path) {
                return job;
            }
            continue;
        }
        let wait = pending
            .values()
            .map(|job| job.ready_at.saturating_duration_since(Instant::now()))
            .min()
            .unwrap_or_else(|| Duration::from_secs(60));
        let (next_pending, _) = queue
            .wake
            .wait_timeout(pending, wait)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending = next_pending;
    }
}

pub(crate) fn runtime_load_json_file_or_default<T>(path: &Path, valid: impl Fn(&T) -> bool) -> T
where
    T: DeserializeOwned + Default,
{
    let Ok(bytes) = fs::read(path) else {
        return T::default();
    };
    let Ok(value) = serde_json::from_slice::<T>(&bytes) else {
        return T::default();
    };
    if valid(&value) { value } else { T::default() }
}

pub(crate) fn runtime_save_merged_json_file<T>(
    path: &Path,
    incoming: T,
    valid_existing: impl Fn(&T) -> bool,
    merge: impl FnOnce(T, T) -> T,
) -> Result<()>
where
    T: DeserializeOwned + Serialize + Default,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let _lock = crate::runtime_store::acquire_json_file_lock(path)?;
    let existing = runtime_load_json_file_or_default(path, valid_existing);
    let merged = merge(existing, incoming);
    let bytes = serde_json::to_vec(&merged).context("failed to encode JSON")?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
