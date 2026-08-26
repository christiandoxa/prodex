use crate::{
    join_thread_with_timeout, redaction_redact_secret_like_text, terminate_child_process_tree,
};
use anyhow::Result;
use base64::Engine;
use prodex_cli::SuperArgs;
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod child_args;
mod worker;

pub(super) const EXPOSE_MAX_ACTIVE_RUNS: usize = 4;
pub(super) const EXPOSE_MAX_QUEUED_RUNS: usize = 16;
pub(super) const EXPOSE_MAX_RETAINED_RUNS: usize = 32;
pub(super) const EXPOSE_MAX_RUN_EVENTS: usize = 256;
pub(super) const EXPOSE_MAX_RUN_EVENT_TEXT_BYTES: usize = 8 * 1024;
pub(super) const EXPOSE_MAX_RUN_OUTPUT_BYTES: usize = 256 * 1024;
pub(super) const EXPOSE_MAX_RUN_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExposeRunState {
    Queued,
    Starting,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    StartFailed,
}

impl ExposeRunState {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::StartFailed => "start_failed",
        }
    }

    pub(super) fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::StartFailed
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct ExposeRunEvent {
    pub(super) seq: u64,
    pub(super) event_type: String,
    pub(super) text: String,
}

#[derive(Clone, Debug)]
pub(super) struct ExposeRunSummary {
    pub(super) run_id: String,
    pub(super) state: ExposeRunState,
    pub(super) created_at: u64,
    pub(super) started_at: Option<u64>,
    pub(super) finished_at: Option<u64>,
    pub(super) exit_status: Option<i32>,
    pub(super) provider: Option<String>,
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
    pub(super) cancellation_requested: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ExposeRunResult {
    pub(super) summary: ExposeRunSummary,
    pub(super) output: String,
    pub(super) output_truncated: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ExposeRunEvents {
    pub(super) events: Vec<ExposeRunEvent>,
    pub(super) next_seq: u64,
    pub(super) truncated: bool,
}

#[derive(Clone)]
pub(super) struct ExposeRunManager {
    inner: Arc<ExposeRunManagerInner>,
}

struct ExposeRunManagerInner {
    state: Mutex<ExposeRunManagerState>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    workspace_root: PathBuf,
    instance_id: String,
    workspace_name: String,
    executable: Option<PathBuf>,
}

struct ExposeRunManagerState {
    runs: std::collections::BTreeMap<String, ExposeRunRecord>,
    queue: VecDeque<QueuedExposeRun>,
    active_runs: usize,
    shutting_down: bool,
}

struct QueuedExposeRun {
    run_id: String,
    task: String,
    args: SuperArgs,
}

struct ExposeRunRecord {
    summary: ExposeRunSummary,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    events: VecDeque<ExposeRunEvent>,
    next_seq: u64,
    output: String,
    output_truncated: bool,
}

type ExposeRunHandles = (Arc<AtomicBool>, Arc<Mutex<Option<Child>>>);

impl ExposeRunManager {
    pub(super) fn new(
        workspace_root: PathBuf,
        instance_id: String,
        workspace_name: String,
    ) -> Self {
        Self::new_inner(workspace_root, instance_id, workspace_name, None)
    }

    #[cfg(test)]
    pub(super) fn new_with_executable(
        workspace_root: PathBuf,
        instance_id: String,
        workspace_name: String,
        executable: PathBuf,
    ) -> Self {
        Self::new_inner(
            workspace_root,
            instance_id,
            workspace_name,
            Some(executable),
        )
    }

    fn new_inner(
        workspace_root: PathBuf,
        instance_id: String,
        workspace_name: String,
        executable: Option<PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(ExposeRunManagerInner {
                state: Mutex::new(ExposeRunManagerState {
                    runs: std::collections::BTreeMap::new(),
                    queue: VecDeque::new(),
                    active_runs: 0,
                    shutting_down: false,
                }),
                threads: Mutex::new(Vec::new()),
                workspace_root,
                instance_id,
                workspace_name,
                executable,
            }),
        }
    }

    pub(super) fn start(
        &self,
        task: String,
        args: SuperArgs,
    ) -> Result<ExposeRunSummary, &'static str> {
        self.reap_finished_threads();
        let run_id = expose_run_id().map_err(|_| "run id generation failed")?;
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "run manager unavailable")?;
        if state.shutting_down {
            return Err("run manager is stopping");
        }
        if state.queue.len() >= EXPOSE_MAX_QUEUED_RUNS
            && state.active_runs >= EXPOSE_MAX_ACTIVE_RUNS
        {
            return Err("run queue is full");
        }
        let summary = ExposeRunSummary {
            run_id: run_id.clone(),
            state: ExposeRunState::Queued,
            created_at: expose_now_millis(),
            started_at: None,
            finished_at: None,
            exit_status: None,
            provider: Some(
                crate::expose::mcp::expose_main_provider(&args)
                    .label()
                    .to_string(),
            ),
            model: args
                .local_model
                .clone()
                .or_else(|| crate::codex_cli_config_override_value(&args.codex_args, "model")),
            reasoning_effort: crate::codex_cli_config_override_value(
                &args.codex_args,
                "model_reasoning_effort",
            ),
            cancellation_requested: false,
        };
        let record = ExposeRunRecord {
            summary: summary.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            events: VecDeque::new(),
            next_seq: 0,
            output: String::new(),
            output_truncated: false,
        };
        state.runs.insert(run_id.clone(), record);
        state.queue.push_back(QueuedExposeRun {
            run_id: run_id.clone(),
            task,
            args,
        });
        if let Some(record) = state.runs.get_mut(&run_id) {
            record.push_event("run_queued", "");
        }
        self.dispatch_locked(&mut state);
        state
            .runs
            .get(&run_id)
            .map(|record| record.summary.clone())
            .ok_or("run manager lost new run")
    }

    pub(super) fn status(&self, run_id: &str) -> Option<ExposeRunSummary> {
        self.inner
            .state
            .lock()
            .ok()?
            .runs
            .get(run_id)
            .map(|record| record.summary.clone())
    }

    pub(super) fn list(&self) -> Vec<ExposeRunSummary> {
        let Ok(state) = self.inner.state.lock() else {
            return Vec::new();
        };
        state
            .runs
            .values()
            .map(|record| record.summary.clone())
            .collect()
    }

    pub(super) fn events(
        &self,
        run_id: &str,
        after_seq: u64,
        limit: usize,
    ) -> Option<ExposeRunEvents> {
        let state = self.inner.state.lock().ok()?;
        let record = state.runs.get(run_id)?;
        let first_seq = record
            .events
            .front()
            .map_or(record.next_seq, |event| event.seq);
        Some(ExposeRunEvents {
            events: record
                .events
                .iter()
                .filter(|event| event.seq > after_seq)
                .take(limit.min(EXPOSE_MAX_RUN_EVENTS))
                .cloned()
                .collect(),
            next_seq: record.next_seq,
            truncated: after_seq.saturating_add(1) < first_seq,
        })
    }

    pub(super) fn result(&self, run_id: &str) -> Option<ExposeRunResult> {
        let state = self.inner.state.lock().ok()?;
        let record = state.runs.get(run_id)?;
        Some(ExposeRunResult {
            summary: record.summary.clone(),
            output: record.output.clone(),
            output_truncated: record.output_truncated,
        })
    }

    pub(super) fn cancel(&self, run_id: &str) -> Option<ExposeRunSummary> {
        let (child, queued) = {
            let mut state = self.inner.state.lock().ok()?;
            let (child, queued) = {
                let record = state.runs.get_mut(run_id)?;
                if record.summary.state.terminal() {
                    return Some(record.summary.clone());
                }
                record.summary.cancellation_requested = true;
                record.cancel.store(true, Ordering::SeqCst);
                (
                    record.child.clone(),
                    record.summary.state == ExposeRunState::Queued,
                )
            };
            if queued {
                state.queue.retain(|job| job.run_id != run_id);
                if let Some(record) = state.runs.get_mut(run_id) {
                    record.summary.state = ExposeRunState::Cancelled;
                    record.summary.finished_at = Some(expose_now_millis());
                    record.push_event("run_cancelled", "");
                }
                self.prune_completed_locked(&mut state);
                self.dispatch_locked(&mut state);
            }
            (child, queued)
        };
        if queued {
            return self.status(run_id);
        }
        if let Ok(mut child) = child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = terminate_child_process_tree(child, true);
        }
        self.status(run_id)
    }

    pub(super) fn shutdown(&self) {
        let children = {
            let Ok(mut state) = self.inner.state.lock() else {
                return;
            };
            if state.shutting_down {
                Vec::new()
            } else {
                state.shutting_down = true;
                let queued = state
                    .queue
                    .drain(..)
                    .map(|job| job.run_id)
                    .collect::<Vec<_>>();
                for run_id in queued {
                    if let Some(record) = state.runs.get_mut(&run_id) {
                        record.cancel.store(true, Ordering::SeqCst);
                        record.summary.cancellation_requested = true;
                        record.summary.state = ExposeRunState::Cancelled;
                        record.summary.finished_at = Some(expose_now_millis());
                        record.push_event("run_cancelled", "");
                    }
                }
                state
                    .runs
                    .values()
                    .filter(|record| !record.summary.state.terminal())
                    .map(|record| {
                        record.cancel.store(true, Ordering::SeqCst);
                        record.child.clone()
                    })
                    .collect::<Vec<_>>()
            }
        };
        for child in children {
            if let Ok(mut child) = child.lock()
                && let Some(child) = child.as_mut()
            {
                let _ = terminate_child_process_tree(child, true);
            }
        }
        let threads = self
            .inner
            .threads
            .lock()
            .map(|mut threads| std::mem::take(&mut *threads))
            .unwrap_or_default();
        for thread in threads {
            if thread.thread().id() == std::thread::current().id() {
                continue;
            }
            let _ = join_thread_with_timeout(thread, Duration::from_secs(2), "expose run worker");
        }
    }

    fn dispatch_locked(&self, state: &mut ExposeRunManagerState) {
        while !state.shutting_down
            && state.active_runs < EXPOSE_MAX_ACTIVE_RUNS
            && !state.queue.is_empty()
        {
            let Some(job) = state.queue.pop_front() else {
                break;
            };
            let Some(record) = state.runs.get_mut(&job.run_id) else {
                continue;
            };
            if record.summary.state != ExposeRunState::Queued {
                continue;
            }
            state.active_runs += 1;
            record.summary.state = ExposeRunState::Starting;
            record.summary.started_at = Some(expose_now_millis());
            record.push_event("run_started", "");
            let manager = self.clone();
            let handle = thread::spawn(move || manager.execute(job));
            if let Ok(mut threads) = self.inner.threads.lock() {
                threads.push(handle);
            }
        }
    }
}

impl Drop for ExposeRunManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.shutdown();
        }
    }
}

impl ExposeRunRecord {
    fn push_event(&mut self, event_type: &str, text: &str) {
        let event = ExposeRunEvent {
            seq: self.next_seq,
            event_type: event_type.to_string(),
            text: bounded_text(text, EXPOSE_MAX_RUN_EVENT_TEXT_BYTES),
        };
        self.next_seq = self.next_seq.saturating_add(1);
        self.events.push_back(event);
        while self.events.len() > EXPOSE_MAX_RUN_EVENTS {
            self.events.pop_front();
        }
    }
}

pub(super) fn redacted_output_text(bytes: &[u8]) -> String {
    bounded_text(
        &redaction_redact_secret_like_text(&String::from_utf8_lossy(bytes)),
        EXPOSE_MAX_RUN_EVENT_TEXT_BYTES,
    )
}

pub(super) fn bounded_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .take_while(|(index, _)| *index <= max_bytes)
        .map(|(index, _)| index)
        .last()
        .unwrap_or(0)
        .min(max_bytes);
    text[..end].to_string()
}

pub(super) fn expose_run_id() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(format!(
        "spr_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn expose_now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
