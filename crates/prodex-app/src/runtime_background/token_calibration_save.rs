use super::{
    runtime_allocator_trim_best_effort, runtime_proxy_log_to_path,
    worker_spawn::spawn_runtime_background_worker_or_log,
};
use anyhow::{Context, Result};
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
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
            spawn_runtime_background_worker_or_log(
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
                        runtime_proxy_log_field(
                            "error",
                            runtime_token_calibration_save_error(&err),
                        ),
                    ],
                ),
            ),
        }
        runtime_allocator_trim_best_effort();
    }
}

fn runtime_token_calibration_save_error(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&format!("{err:#}"))
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
    if !runtime_json_path_is_regular_file(path) {
        return T::default();
    }
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
    runtime_write_json_file_atomic(path, &bytes)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn runtime_json_path_is_regular_file(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_file(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn runtime_write_json_file_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp_path = crate::runtime_store::unique_state_temp_file_path(path);
    write_private_file(&temp_path, bytes)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    if let Err(err) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err).with_context(|| format!("failed to replace {}", path.display()));
    }
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = open_private_file(path)?;
    file.write_all(bytes)
}

#[cfg(unix)]
fn open_private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "prodex-token-calibration-save-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    #[test]
    fn save_merged_json_replaces_symlink_without_reading_or_touching_target() {
        let root = temp_dir("symlink");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("calibration.json");
        let target = root.join("target.json");
        fs::write(&target, r#"{"outside":"secret"}"#).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        let incoming = BTreeMap::from([("safe".to_string(), "value".to_string())]);
        runtime_save_merged_json_file(
            &path,
            incoming,
            |_existing: &BTreeMap<String, String>| true,
            |mut existing, incoming| {
                existing.extend(incoming);
                existing
            },
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            r#"{"outside":"secret"}"#
        );
        assert!(
            !fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let saved: BTreeMap<String, String> =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved,
            BTreeMap::from([("safe".to_string(), "value".to_string())])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_token_calibration_save_error_redacts_secret_like_chain() {
        let err = anyhow::anyhow!("failed: Authorization: Bearer token-calibration-token")
            .context("token calibration save failed");

        let message = runtime_token_calibration_save_error(&err);

        assert!(message.contains("token calibration save failed"));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(!message.contains("token-calibration-token"));
    }
}
