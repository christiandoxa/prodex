use crate::runtime_panic::{catch_runtime_unwind_silently, runtime_panic_payload_label};
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::{runtime_proxy_latest_log_path_from_pointer, runtime_proxy_log_to_path};

pub(crate) fn spawn_runtime_background_worker_or_log(
    name: impl Into<String>,
    panic_log_path: Option<PathBuf>,
    worker: impl FnOnce() + Send + 'static,
) {
    let name = name.into();
    if let Err(err) =
        try_spawn_runtime_background_worker(name.clone(), panic_log_path.clone(), worker)
    {
        log_runtime_background_worker_spawn_error(panic_log_path.as_deref(), &name, &err);
    }
}

pub(crate) fn try_spawn_runtime_background_worker(
    name: impl Into<String>,
    panic_log_path: Option<PathBuf>,
    worker: impl FnOnce() + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    let name = name.into();
    let worker_name = name.clone();
    thread::Builder::new().name(name).spawn(move || {
        let panic_result = catch_runtime_unwind_silently(worker);
        if let Err(payload) = panic_result {
            let panic_message = runtime_panic_payload_label(payload.as_ref());
            log_runtime_background_worker_panic(
                panic_log_path.as_deref(),
                &worker_name,
                &panic_message,
            );
            panic::resume_unwind(payload);
        }
    })
}

pub(crate) fn try_spawn_runtime_supervised_worker(
    name: impl Into<String>,
    log_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    worker: impl Fn() + Send + Sync + 'static,
) -> io::Result<JoinHandle<()>> {
    let name = name.into();
    let worker_name = name.clone();
    thread::Builder::new().name(name).spawn(move || {
        while !shutdown.load(Ordering::SeqCst) {
            let started_at = std::time::Instant::now();
            let result = catch_runtime_unwind_silently(&worker);
            if shutdown.load(Ordering::SeqCst) {
                break;
            }
            let (reason, error) = match result {
                Ok(()) => ("returned", None),
                Err(payload) => (
                    "panic",
                    Some(runtime_background_worker_error_log_value(
                        runtime_panic_payload_label(payload.as_ref()).as_str(),
                    )),
                ),
            };
            let mut fields = vec![
                runtime_proxy_log_field("worker", &worker_name),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("restart", "true"),
                runtime_proxy_log_field("uptime_ms", started_at.elapsed().as_millis().to_string()),
            ];
            if let Some(error) = error {
                fields.push(runtime_proxy_log_field("error", error));
            }
            runtime_proxy_log_to_path(
                &log_path,
                &runtime_proxy_structured_log_message("runtime_worker_restart", fields),
            );
            thread::sleep(Duration::from_millis(100));
        }
    })
}

fn log_runtime_background_worker_spawn_error(
    log_path: Option<&Path>,
    worker_name: &str,
    err: &io::Error,
) {
    log_runtime_background_worker_event(
        log_path,
        runtime_proxy_structured_log_message(
            "runtime_background_worker_spawn_error",
            [
                runtime_proxy_log_field("worker", worker_name),
                runtime_proxy_log_field(
                    "error",
                    runtime_background_worker_error_log_value(&err.to_string()),
                ),
            ],
        ),
    );
}

fn log_runtime_background_worker_panic(
    log_path: Option<&Path>,
    worker_name: &str,
    panic_message: &str,
) {
    log_runtime_background_worker_event(
        log_path,
        runtime_proxy_structured_log_message(
            "runtime_background_worker_panic",
            [
                runtime_proxy_log_field("worker", worker_name),
                runtime_proxy_log_field(
                    "error",
                    runtime_background_worker_error_log_value(panic_message),
                ),
            ],
        ),
    );
}

fn runtime_background_worker_error_log_value(error: &str) -> String {
    redaction_redact_secret_like_text(error).replace('\n', " ")
}

fn log_runtime_background_worker_event(log_path: Option<&Path>, message: String) {
    if let Some(log_path) = log_path {
        runtime_proxy_log_to_path(log_path, &message);
        return;
    }
    if let Some(log_path) = runtime_background_latest_log_path() {
        runtime_proxy_log_to_path(&log_path, &message);
    }
}

fn runtime_background_latest_log_path() -> Option<PathBuf> {
    runtime_proxy_latest_log_path_from_pointer()
}

#[cfg(test)]
mod tests {
    use super::{
        runtime_background_worker_error_log_value, try_spawn_runtime_background_worker,
        try_spawn_runtime_supervised_worker,
    };
    use std::fs;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn runtime_background_worker_spawn_names_thread() {
        let (tx, rx) = mpsc::channel();
        let handle = try_spawn_runtime_background_worker("prodex-test-worker", None, move || {
            tx.send(thread::current().name().map(str::to_string))
                .expect("thread name send should succeed");
        })
        .expect("worker should spawn");

        handle.join().expect("worker should not panic");

        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("thread name should be sent")
                .as_deref(),
            Some("prodex-test-worker")
        );
    }

    #[test]
    fn runtime_background_worker_spawn_logs_panic_to_runtime_log() {
        let log_path = std::env::temp_dir().join(format!(
            "prodex-background-worker-panic-{}-{}.log",
            std::process::id(),
            thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_file(&log_path);

        let handle = try_spawn_runtime_background_worker(
            "prodex-panic-worker",
            Some(log_path.clone()),
            || panic!("simulated background panic"),
        )
        .expect("worker should spawn");

        assert!(handle.join().is_err());

        let log = fs::read_to_string(&log_path).expect("panic log should be written");
        assert!(log.contains("runtime_background_worker_panic"));
        assert!(log.contains("worker=prodex-panic-worker"));
        assert!(log.contains("error=<redacted>"));
        let _ = fs::remove_file(log_path);
    }

    #[test]
    fn runtime_background_worker_error_log_value_redacts_secret_like_material() {
        let message = runtime_background_worker_error_log_value(
            "spawn failed\nAuthorization: Bearer worker-token\napi_key=worker-key",
        );

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("worker-token"));
        assert!(!message.contains("worker-key"));
    }

    #[test]
    fn supervised_worker_restarts_after_panic_until_shutdown() {
        let log_path = std::env::temp_dir().join(format!(
            "prodex-supervised-worker-{}-{}.log",
            std::process::id(),
            thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_file(&log_path);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_calls = Arc::clone(&calls);

        let handle = try_spawn_runtime_supervised_worker(
            "prodex-supervised-test-worker",
            log_path.clone(),
            Arc::clone(&shutdown),
            move || {
                if worker_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("simulated supervised worker panic");
                }
                worker_shutdown.store(true, Ordering::SeqCst);
            },
        )
        .expect("supervised worker should spawn");

        handle
            .join()
            .expect("supervisor should contain worker panic");

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let log = fs::read_to_string(&log_path).expect("restart log should be written");
        assert!(log.contains("runtime_worker_restart"));
        assert!(log.contains("worker=prodex-supervised-test-worker"));
        assert!(log.contains("reason=panic"));
        assert!(log.contains("restart=true"));
        let _ = fs::remove_file(log_path);
    }
}
