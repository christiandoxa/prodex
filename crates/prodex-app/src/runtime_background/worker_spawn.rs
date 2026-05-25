use crate::runtime_panic::{catch_runtime_unwind_silently, runtime_panic_payload_label};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::fs;
use std::io;
use std::panic;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use super::{runtime_proxy_latest_log_pointer_path, runtime_proxy_log_to_path};

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
                runtime_proxy_log_field("error", err.to_string()),
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
                runtime_proxy_log_field("error", panic_message),
            ],
        ),
    );
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
    let pointer = fs::read_to_string(runtime_proxy_latest_log_pointer_path()).ok()?;
    let path = pointer.lines().next()?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
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
        assert!(log.contains("error=\"simulated background panic\""));
        let _ = fs::remove_file(log_path);
    }
}
