use super::child_args::{build_super_child_args, expose_api_key_env};
use super::{
    EXPOSE_MAX_RETAINED_RUNS, EXPOSE_MAX_RUN_OUTPUT_BYTES, ExposeRunHandles, ExposeRunManager,
    ExposeRunManagerState, ExposeRunRecord, ExposeRunState, QueuedExposeRun, bounded_text,
    expose_now_millis, redacted_output_text,
};
use crate::{
    child_exit_code, configure_child_process_group, join_thread_with_timeout,
    terminate_child_process_tree,
};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::thread::{self, JoinHandle};
use std::time::Duration;

impl ExposeRunManager {
    pub(super) fn execute(&self, job: QueuedExposeRun) {
        let Some(record) = self.record_handles(&job.run_id) else {
            return;
        };
        if record.0.load(Ordering::SeqCst) {
            self.finish_cancelled(&job.run_id);
            return;
        }
        let executable = match self
            .inner
            .executable
            .clone()
            .map(Ok)
            .unwrap_or_else(std::env::current_exe)
        {
            Ok(executable) => executable,
            Err(_) => {
                self.finish_start_failed(&job.run_id);
                return;
            }
        };
        let mut command = Command::new(executable);
        command
            .args(build_super_child_args(&job.args))
            .current_dir(&self.inner.workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PRODEX_EXPOSE_INSTANCE_ID", &self.inner.instance_id)
            .env("PRODEX_EXPOSE_WORKSPACE_NAME", &self.inner.workspace_name);
        if let Some((key, value)) = expose_api_key_env(&job.args) {
            command.env(key, value);
        }
        configure_child_process_group(&mut command, true);
        crate::configure_child_parent_death(&mut command);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                self.finish_start_failed(&job.run_id);
                return;
            }
        };
        let mut stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child_slot = record.1.clone();
        if let Ok(mut slot) = child_slot.lock() {
            *slot = Some(child);
        }
        if let Some(mut stdin) = stdin.take()
            && (stdin.write_all(job.task.as_bytes()).is_err() || stdin.flush().is_err())
            && let Ok(mut child) = child_slot.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = terminate_child_process_tree(child, true);
        }
        drop(stdin);
        self.mark_running(&job.run_id);
        let readers = [
            stdout.map(|reader| self.spawn_output_reader(&job.run_id, reader, "stdout")),
            stderr.map(|reader| self.spawn_output_reader(&job.run_id, reader, "stderr")),
        ];
        let mut status = loop {
            if record.0.load(Ordering::SeqCst)
                && let Ok(mut child) = child_slot.lock()
                && let Some(child) = child.as_mut()
            {
                let _ = terminate_child_process_tree(child, true);
            }
            let polled = child_slot
                .lock()
                .ok()
                .and_then(|mut child| child.as_mut().map(Child::try_wait));
            match polled {
                Some(Ok(Some(status))) => break Some(status),
                Some(Ok(None)) => thread::sleep(Duration::from_millis(20)),
                Some(Err(_)) | None => break None,
            }
        };
        if let Ok(mut child) = child_slot.lock() {
            if let Some(child) = child.as_mut()
                && status.is_none()
            {
                status = child.wait().ok();
            }
            child.take();
        }
        for reader in readers.into_iter().flatten() {
            let _ =
                join_thread_with_timeout(reader, Duration::from_secs(2), "expose output reader");
        }
        if record.0.load(Ordering::SeqCst) {
            self.finish_cancelled(&job.run_id);
        } else if let Some(status) = status {
            self.finish_process(&job.run_id, &status);
        } else {
            self.finish_start_failed(&job.run_id);
        }
    }

    pub(super) fn record_handles(&self, run_id: &str) -> Option<ExposeRunHandles> {
        let state = self.inner.state.lock().ok()?;
        let record = state.runs.get(run_id)?;
        Some((record.cancel.clone(), record.child.clone()))
    }

    pub(super) fn spawn_output_reader(
        &self,
        run_id: &str,
        mut reader: impl Read + Send + 'static,
        event_type: &'static str,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        let run_id = run_id.to_string();
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => manager.append_output(&run_id, event_type, &buffer[..size]),
                    Err(_) => break,
                }
            }
        })
    }

    pub(super) fn append_output(&self, run_id: &str, event_type: &str, bytes: &[u8]) {
        let text = redacted_output_text(bytes);
        if text.is_empty() {
            return;
        }
        if let Ok(mut state) = self.inner.state.lock()
            && let Some(record) = state.runs.get_mut(run_id)
        {
            if record.output.len() < EXPOSE_MAX_RUN_OUTPUT_BYTES {
                let remaining = EXPOSE_MAX_RUN_OUTPUT_BYTES - record.output.len();
                let prefix = bounded_text(&text, remaining);
                record.output.push_str(&prefix);
                if prefix.len() < text.len() {
                    record.output_truncated = true;
                }
            } else {
                record.output_truncated = true;
            }
            record.push_event(event_type, &text);
            if record.output_truncated
                && !record
                    .events
                    .iter()
                    .any(|event| event.event_type == "output_truncated")
            {
                record.push_event("output_truncated", "output limit reached");
            }
        }
    }

    pub(super) fn mark_running(&self, run_id: &str) {
        if let Ok(mut state) = self.inner.state.lock()
            && let Some(record) = state.runs.get_mut(run_id)
            && record.summary.state == ExposeRunState::Starting
        {
            record.summary.state = ExposeRunState::Running;
        }
    }

    pub(super) fn finish_process(&self, run_id: &str, status: &std::process::ExitStatus) {
        self.finish(run_id, |record| {
            record.summary.state = if status.success() {
                ExposeRunState::Succeeded
            } else {
                ExposeRunState::Failed
            };
            record.summary.exit_status = Some(child_exit_code(status));
            record.push_event(
                if status.success() {
                    "run_succeeded"
                } else {
                    "run_failed"
                },
                "",
            );
        });
    }

    pub(super) fn finish_cancelled(&self, run_id: &str) {
        self.finish(run_id, |record| {
            record.summary.state = ExposeRunState::Cancelled;
            record.push_event("run_cancelled", "");
        });
    }

    pub(super) fn finish_start_failed(&self, run_id: &str) {
        self.finish(run_id, |record| {
            if record.cancel.load(Ordering::SeqCst) {
                record.summary.state = ExposeRunState::Cancelled;
                record.push_event("run_cancelled", "");
            } else {
                record.summary.state = ExposeRunState::StartFailed;
                record.push_event("run_start_failed", "Super child could not start");
            }
        });
    }

    pub(super) fn reap_finished_threads(&self) {
        let Ok(mut threads) = self.inner.threads.lock() else {
            return;
        };
        let mut active = Vec::with_capacity(threads.len());
        for thread in threads.drain(..) {
            if thread.is_finished() {
                let _ = thread.join();
            } else {
                active.push(thread);
            }
        }
        *threads = active;
    }

    pub(super) fn finish(&self, run_id: &str, update: impl FnOnce(&mut ExposeRunRecord)) {
        if let Ok(mut state) = self.inner.state.lock()
            && let Some(record) = state.runs.get_mut(run_id)
            && !record.summary.state.terminal()
        {
            update(record);
            record.summary.finished_at = Some(expose_now_millis());
            state.active_runs = state.active_runs.saturating_sub(1);
            self.prune_completed_locked(&mut state);
            self.dispatch_locked(&mut state);
        }
    }

    pub(super) fn prune_completed_locked(&self, state: &mut ExposeRunManagerState) {
        while state
            .runs
            .values()
            .filter(|record| record.summary.state.terminal())
            .count()
            > EXPOSE_MAX_RETAINED_RUNS
        {
            let Some(oldest) = state
                .runs
                .iter()
                .filter(|(_, record)| record.summary.state.terminal())
                .min_by_key(|(_, record)| record.summary.finished_at.unwrap_or(u64::MAX))
                .map(|(run_id, _)| run_id.clone())
            else {
                break;
            };
            state.runs.remove(&oldest);
        }
    }
}
