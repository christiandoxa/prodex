use super::{bounded_redacted_text, logging::ExposeAuditLog, now_millis};
use crate::{configure_child_process_group, terminate_child_process_tree};
use anyhow::{Context, Result};
use base64::Engine as _;
use prodex_cli::SuperArgs;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_ACTIVE_RUNS: usize = 4;
const MAX_QUEUED_RUNS: usize = 16;
const MAX_RETAINED_TERMINAL_RUNS: usize = 32;
const OUTPUT_MAX_BYTES: usize = 256 * 1024;
const MAX_RUN_EVENTS: usize = 256;
const MAX_RUN_EVENT_TEXT_BYTES: usize = 8 * 1024;

#[path = "run/child_args.rs"]
mod child_args;
#[path = "run/config.rs"]
mod config;
#[path = "run/helpers.rs"]
mod helpers;

use child_args::{api_key_env, build_child_args};
use config::{apply_overrides, main_provider, validate_run_configuration};
use helpers::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunState {
    Queued,
    Starting,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    StartFailed,
}

impl RunState {
    fn as_str(self) -> &'static str {
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

    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::StartFailed
        )
    }
}

#[derive(Clone)]
pub(super) struct RunManager {
    inner: Arc<RunManagerInner>,
}

struct RunManagerInner {
    workspace: PathBuf,
    base_args: SuperArgs,
    audit: ExposeAuditLog,
    instance_id: String,
    workspace_name: String,
    state: Mutex<RunManagerState>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

struct RunManagerState {
    runs: BTreeMap<String, RunRecord>,
    queue: VecDeque<QueuedRun>,
    active_runs: usize,
    shutting_down: bool,
}

struct QueuedRun {
    run_id: String,
    task: String,
    args: SuperArgs,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

#[derive(Clone, Debug)]
pub(super) struct RunEvent {
    pub(super) seq: u64,
    pub(super) event_type: String,
    pub(super) text: String,
}

pub(super) struct RunEvents {
    pub(super) events: Vec<RunEvent>,
    pub(super) next_seq: u64,
    pub(super) truncated: bool,
}

struct RunRecord {
    state: RunState,
    created_at: u64,
    started_at: Option<u64>,
    finished_at: Option<u64>,
    exit_status: Option<i32>,
    output: String,
    output_truncated: bool,
    events: VecDeque<RunEvent>,
    next_seq: u64,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    provider: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
}

impl RunManager {
    pub(super) fn new(
        workspace: PathBuf,
        base_args: SuperArgs,
        audit: ExposeAuditLog,
        instance_id: String,
    ) -> Self {
        let workspace_name = workspace
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("workspace")
            .to_string();
        Self {
            inner: Arc::new(RunManagerInner {
                workspace,
                base_args,
                audit,
                instance_id,
                workspace_name,
                state: Mutex::new(RunManagerState {
                    runs: BTreeMap::new(),
                    queue: VecDeque::new(),
                    active_runs: 0,
                    shutting_down: false,
                }),
                threads: Mutex::new(Vec::new()),
            }),
        }
    }

    pub(super) fn start(
        &self,
        task: String,
        overrides: &Value,
    ) -> std::result::Result<Value, String> {
        self.reap_finished_threads();
        if task.trim().is_empty() || task.len() > 65_536 {
            return Err("task is empty or too large".to_string());
        }
        let mut args = self.inner.base_args.clone();
        apply_overrides(&mut args, overrides)?;
        validate_run_configuration(&args)?;

        let run_id = new_run_id().map_err(|error| error.to_string())?;
        let cancel = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        let provider = Some(main_provider(&args).label().to_string());
        let model = args
            .local_model
            .clone()
            .or_else(|| crate::codex_cli_config_override_value(&args.codex_args, "model"));
        let reasoning_effort =
            crate::codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort");

        {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| "run manager unavailable".to_string())?;
            if state.shutting_down {
                return Err("run manager is stopping".to_string());
            }
            if state.queue.len() >= MAX_QUEUED_RUNS && state.active_runs >= MAX_ACTIVE_RUNS {
                self.inner.audit.event(
                    "super_expose_run_rejected",
                    [crate::runtime_proxy_log_field("reason", "queue_full")],
                );
                return Err("run queue is full".to_string());
            }
            let mut record = RunRecord {
                state: RunState::Queued,
                created_at: now_millis(),
                started_at: None,
                finished_at: None,
                exit_status: None,
                output: String::new(),
                output_truncated: false,
                events: VecDeque::new(),
                next_seq: 0,
                cancel: cancel.clone(),
                child: child.clone(),
                provider,
                model,
                reasoning_effort,
            };
            push_event(&mut record, "run_queued", "");
            state.runs.insert(run_id.clone(), record);
            state.queue.push_back(QueuedRun {
                run_id: run_id.clone(),
                task,
                args,
                cancel,
                child,
            });
            self.dispatch_locked(&mut state);
        }

        self.inner.audit.event(
            "super_expose_run_created",
            [crate::runtime_proxy_log_field("run_id", run_id.clone())],
        );
        self.status(&run_id)
            .ok_or_else(|| "run manager lost new run".to_string())
    }

    pub(super) fn status(&self, run_id: &str) -> Option<Value> {
        let state = self.inner.state.lock().ok()?;
        let record = state.runs.get(run_id)?;
        Some(summary_json(run_id, record))
    }

    pub(super) fn result(&self, run_id: &str) -> Option<Value> {
        let state = self.inner.state.lock().ok()?;
        let record = state.runs.get(run_id)?;
        let mut result = summary_json(run_id, record);
        result["output"] = Value::String(record.output.clone());
        result["output_truncated"] = Value::Bool(record.output_truncated);
        Some(result)
    }

    pub(super) fn list(&self) -> Vec<Value> {
        let Ok(state) = self.inner.state.lock() else {
            return Vec::new();
        };
        state
            .runs
            .iter()
            .map(|(run_id, record)| summary_json(run_id, record))
            .collect()
    }

    pub(super) fn events(&self, run_id: &str, after_seq: u64, limit: usize) -> Option<RunEvents> {
        let state = self.inner.state.lock().ok()?;
        let record = state.runs.get(run_id)?;
        Some(events_page(record, after_seq, limit))
    }

    pub(super) fn cancel(&self, run_id: &str) -> Option<Value> {
        let (child, queued) = {
            let mut state = self.inner.state.lock().ok()?;
            let (child, queued) = {
                let record = state.runs.get_mut(run_id)?;
                if record.state.terminal() {
                    return Some(summary_json(run_id, record));
                }
                record.cancel.store(true, Ordering::SeqCst);
                (record.child.clone(), record.state == RunState::Queued)
            };
            self.inner.audit.event(
                "super_expose_run_cancel_requested",
                [crate::runtime_proxy_log_field("run_id", run_id.to_string())],
            );
            if queued {
                state.queue.retain(|job| job.run_id != run_id);
                if let Some(record) = state.runs.get_mut(run_id) {
                    record.state = RunState::Cancelled;
                    record.finished_at = Some(now_millis());
                    push_event(record, "run_cancelled", "");
                }
                prune_terminal_runs(&mut state.runs);
                self.dispatch_locked(&mut state);
            }
            (child, queued)
        };
        if !queued
            && let Ok(mut child) = child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = terminate_child_process_tree(child, true);
        }
        self.status(run_id)
    }

    fn dispatch_locked(&self, state: &mut RunManagerState) {
        if state.shutting_down {
            return;
        }
        while state.active_runs < MAX_ACTIVE_RUNS && !state.queue.is_empty() {
            let Some(job) = state.queue.pop_front() else {
                break;
            };
            let Some(record) = state.runs.get_mut(&job.run_id) else {
                continue;
            };
            if record.state != RunState::Queued {
                continue;
            }
            state.active_runs += 1;
            record.state = RunState::Starting;
            record.started_at = Some(now_millis());
            push_event(record, "run_started", "");
            let manager = self.clone();
            let handle = thread::spawn(move || manager.execute(job));
            if let Ok(mut threads) = self.inner.threads.lock() {
                threads.push(handle);
            }
        }
    }

    fn reap_finished_threads(&self) {
        let Ok(mut threads) = self.inner.threads.lock() else {
            return;
        };
        let mut active = Vec::with_capacity(threads.len());
        for handle in threads.drain(..) {
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                active.push(handle);
            }
        }
        *threads = active;
    }

    fn shutdown(&self) {
        let children = {
            let Ok(mut state) = self.inner.state.lock() else {
                return;
            };
            if state.shutting_down {
                Vec::new()
            } else {
                state.shutting_down = true;
                state.queue.clear();
                let now = now_millis();
                let mut children = Vec::new();
                for record in state.runs.values_mut() {
                    if record.state.terminal() {
                        continue;
                    }
                    record.cancel.store(true, Ordering::SeqCst);
                    if record.state == RunState::Queued {
                        record.state = RunState::Cancelled;
                        record.finished_at = Some(now);
                        push_event(record, "run_cancelled", "");
                    } else {
                        children.push(record.child.clone());
                    }
                }
                children
            }
        };
        for child in children {
            terminate_registered_child(&child);
        }
        let handles = self
            .inner
            .threads
            .lock()
            .map(|mut threads| threads.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        for handle in handles {
            let _ = crate::join_thread_with_timeout(
                handle,
                Duration::from_secs(2),
                "expose run worker",
            );
        }
    }

    fn execute(&self, job: QueuedRun) {
        let run_id = job.run_id.clone();
        if job.cancel.load(Ordering::SeqCst) {
            self.finish_cancelled(&run_id);
            return;
        }
        let command = match self.child_command(&job.args) {
            Ok(command) => command,
            Err(message) => {
                self.record_start_failure(&run_id, "current_exe", &message);
                return;
            }
        };
        let pipes = match spawn_registered_child(command, &job.child) {
            Ok(handles) => handles,
            Err(message) => {
                self.record_start_failure(&run_id, "spawn", &message);
                return;
            }
        };

        self.mark_running(&run_id);
        self.inner.audit.event(
            "super_expose_run_started",
            [crate::runtime_proxy_log_field("run_id", run_id.clone())],
        );
        let readers = spawn_child_readers(self.clone(), &run_id, pipes.stdout, pipes.stderr);
        write_child_task(pipes.stdin, &job.task, &job.cancel);
        let status = poll_child_status(&job.child, &job.cancel);
        clear_child_slot(&job.child);
        join_child_readers(readers);
        self.finish_run(&run_id, &job.cancel, status);
    }

    fn child_command(&self, args: &SuperArgs) -> std::result::Result<Command, String> {
        let executable =
            std::env::current_exe().map_err(|error| format!("current executable: {error}"))?;
        let mut command = Command::new(executable);
        command
            .args(build_child_args(args).map_err(|error| error.to_string())?)
            .current_dir(&self.inner.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PRODEX_EXPOSE_INSTANCE_ID", &self.inner.instance_id)
            .env("PRODEX_EXPOSE_WORKSPACE_NAME", &self.inner.workspace_name)
            .env_remove("CONTROL_PLANE_API_KEY");
        if let Some((name, value)) = api_key_env(args) {
            command.env(name, value);
        }
        configure_child_process_group(&mut command, true);
        crate::configure_child_parent_death(&mut command);
        Ok(command)
    }

    fn record_start_failure(&self, run_id: &str, stage: &'static str, message: &str) {
        self.inner.audit.event(
            "super_expose_run_start_failed",
            [
                crate::runtime_proxy_log_field("run_id", run_id.to_string()),
                crate::runtime_proxy_log_field("stage", stage),
            ],
        );
        self.finish_start_failed(run_id, message);
    }

    fn finish_run(&self, run_id: &str, cancel: &AtomicBool, status: Option<ExitStatus>) {
        let Ok(mut state) = self.inner.state.lock() else {
            return;
        };
        let (run_state, exit_code, output_bytes, output_truncated) = {
            let Some(record) = state.runs.get_mut(run_id) else {
                return;
            };
            if record.state.terminal() {
                return;
            }
            record.finished_at = Some(now_millis());
            apply_run_terminal_state(record, cancel.load(Ordering::SeqCst), status);
            push_event(
                record,
                match record.state {
                    RunState::Succeeded => "run_succeeded",
                    RunState::Failed => "run_failed",
                    RunState::Cancelled => "run_cancelled",
                    RunState::StartFailed => "run_start_failed",
                    RunState::Queued | RunState::Starting | RunState::Running => "run_failed",
                },
                "",
            );
            (
                record.state,
                record.exit_status,
                record.output.len(),
                record.output_truncated,
            )
        };
        state.active_runs = state.active_runs.saturating_sub(1);
        prune_terminal_runs(&mut state.runs);
        self.dispatch_locked(&mut state);
        drop(state);
        self.inner.audit.event(
            "super_expose_run_completed",
            [
                crate::runtime_proxy_log_field("run_id", run_id.to_string()),
                crate::runtime_proxy_log_field("state", run_state.as_str()),
                crate::runtime_proxy_log_field(
                    "exit_code",
                    exit_code.map_or_else(|| "none".to_string(), |code| code.to_string()),
                ),
                crate::runtime_proxy_log_field("output_bytes", output_bytes.to_string()),
                crate::runtime_proxy_log_field("output_truncated", output_truncated.to_string()),
            ],
        );
    }

    fn finish_cancelled(&self, run_id: &str) {
        let Ok(mut state) = self.inner.state.lock() else {
            return;
        };
        if let Some(record) = state.runs.get_mut(run_id)
            && !record.state.terminal()
        {
            record.state = RunState::Cancelled;
            record.finished_at = Some(now_millis());
            push_event(record, "run_cancelled", "");
            state.active_runs = state.active_runs.saturating_sub(1);
            prune_terminal_runs(&mut state.runs);
            self.dispatch_locked(&mut state);
        }
    }

    fn mark_running(&self, run_id: &str) {
        if let Ok(mut state) = self.inner.state.lock()
            && let Some(record) = state.runs.get_mut(run_id)
            && record.state == RunState::Starting
        {
            record.state = RunState::Running;
        }
    }

    fn finish_start_failed(&self, run_id: &str, message: &str) {
        if let Ok(mut state) = self.inner.state.lock()
            && let Some(record) = state.runs.get_mut(run_id)
            && !record.state.terminal()
        {
            record.state = if record.cancel.load(Ordering::SeqCst) {
                RunState::Cancelled
            } else {
                RunState::StartFailed
            };
            record.finished_at = Some(now_millis());
            append_output(record, "stderr", message.as_bytes());
            push_event(
                record,
                if record.state == RunState::Cancelled {
                    "run_cancelled"
                } else {
                    "run_start_failed"
                },
                if record.state == RunState::Cancelled {
                    ""
                } else {
                    "Super child could not start"
                },
            );
            state.active_runs = state.active_runs.saturating_sub(1);
            prune_terminal_runs(&mut state.runs);
            self.dispatch_locked(&mut state);
        }
    }

    fn append(&self, run_id: &str, event_type: &'static str, bytes: &[u8]) {
        if let Ok(mut state) = self.inner.state.lock()
            && let Some(record) = state.runs.get_mut(run_id)
        {
            append_output(record, event_type, bytes);
        }
    }
}

impl Drop for RunManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn record() -> RunRecord {
        RunRecord {
            state: RunState::Queued,
            created_at: 1,
            started_at: None,
            finished_at: None,
            exit_status: None,
            output: String::new(),
            output_truncated: false,
            events: VecDeque::new(),
            next_seq: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            provider: Some("openai".to_string()),
            model: Some("gpt-test".to_string()),
            reasoning_effort: Some("high".to_string()),
        }
    }

    #[test]
    fn child_args_use_normal_super_exec_lifecycle() {
        let prodex_cli::Commands::Super(args) =
            prodex_cli::parse_cli_command_from(["prodex", "s", "--no-presidio"]).unwrap()
        else {
            panic!("expected super args");
        };
        let child = build_child_args(&args).expect("child argument plan should succeed");
        assert_eq!(child.first().and_then(|value| value.to_str()), Some("s"));
        assert!(child.iter().any(|value| value == "--full-access"));
        assert_eq!(
            child
                .iter()
                .rev()
                .take(2)
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-".to_string(), "exec".to_string()]
        );
    }

    #[test]
    fn native_agy_child_args_do_not_receive_codex_exec_or_generated_config() {
        let prodex_cli::Commands::Super(mut args) = prodex_cli::parse_cli_command_from([
            "prodex",
            "s",
            "gemini",
            "--cli",
            "agy",
            "--no-presidio",
        ])
        .unwrap() else {
            panic!("expected super args");
        };
        args.codex_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"gemini\""),
            OsString::from("--config"),
            OsString::from("model_reasoning_effort=\"max\""),
            OsString::from("--prompt"),
            OsString::from("review"),
        ];
        let child = build_child_args(&args).expect("child argument plan should succeed");
        assert!(child.iter().any(|value| value == "--cli"));
        assert!(child.iter().any(|value| value == "agy"));
        assert!(child.iter().any(|value| value == "--prompt"));
        assert!(child.iter().any(|value| value == "review"));
        assert!(!child.iter().any(|value| value == "exec" || value == "-"));
        assert!(!child.iter().any(|value| {
            let value = value.to_string_lossy();
            value.contains("model_provider") || value.contains("model_reasoning_effort")
        }));
    }

    #[test]
    fn sub_agent_url_and_concurrency_are_forwarded() {
        let prodex_cli::Commands::Super(args) = prodex_cli::parse_cli_command_from([
            "prodex",
            "s",
            "--sub-agent",
            "--sub-agent-provider",
            "openai",
            "--sub-agent-url",
            "http://127.0.0.1:8787/v1",
            "--sub-agent-max-concurrency",
            "3",
            "--no-presidio",
        ])
        .unwrap() else {
            panic!("expected super args");
        };
        let child = build_child_args(&args).expect("child argument plan should succeed");
        let rendered = child
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            rendered
                .windows(2)
                .any(|pair| pair == ["--sub-agent-url", "http://127.0.0.1:8787/v1"])
        );
        assert!(
            rendered
                .windows(2)
                .any(|pair| pair == ["--sub-agent-max-concurrency", "3"])
        );
    }

    #[test]
    fn run_ids_match_the_04294_opaque_shape() {
        let one = new_run_id().expect("first run id");
        let two = new_run_id().expect("second run id");
        assert_ne!(one, two);
        assert!(one.starts_with("spr_"));
        assert_eq!(one.len(), 26);
        assert!(
            one.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        );
    }

    #[test]
    fn run_limits_match_04294() {
        assert_eq!(MAX_ACTIVE_RUNS, 4);
        assert_eq!(MAX_QUEUED_RUNS, 16);
        assert_eq!(MAX_RETAINED_TERMINAL_RUNS, 32);
        assert_eq!(MAX_RUN_EVENTS, 256);
        assert_eq!(MAX_RUN_EVENT_TEXT_BYTES, 8 * 1024);
        assert_eq!(OUTPUT_MAX_BYTES, 256 * 1024);
    }

    #[test]
    fn event_retention_and_pagination_match_04294() {
        let mut record = record();
        for index in 0..300 {
            push_event(&mut record, "stdout", &format!("event-{index}"));
        }
        assert_eq!(record.events.len(), 256);
        assert_eq!(record.events.front().map(|event| event.seq), Some(44));
        assert_eq!(record.next_seq, 300);

        let page = events_page(&record, 0, 64);
        assert_eq!(page.events.len(), 64);
        assert_eq!(page.events.first().map(|event| event.seq), Some(44));
        assert_eq!(page.events.last().map(|event| event.seq), Some(107));
        assert_eq!(page.next_seq, 300);
        assert!(page.truncated);

        let next = events_page(&record, 107, 64);
        assert_eq!(next.events.first().map(|event| event.seq), Some(108));
        assert!(!next.truncated);
    }

    #[test]
    fn stdout_events_continue_after_aggregate_output_is_truncated() {
        let mut record = record();
        record.output = "x".repeat(OUTPUT_MAX_BYTES);
        append_output(&mut record, "stdout", b"later-output");
        assert_eq!(record.output.len(), OUTPUT_MAX_BYTES);
        assert!(record.output_truncated);
        assert!(
            record
                .events
                .iter()
                .any(|event| event.event_type == "stdout" && event.text == "later-output")
        );
        assert_eq!(
            record
                .events
                .iter()
                .filter(|event| event.event_type == "output_truncated")
                .count(),
            1
        );
    }

    #[test]
    fn summary_keeps_provider_model_and_effort_metadata() {
        let record = record();
        let summary = summary_json("spr_fixture", &record);
        assert_eq!(summary["provider"], "openai");
        assert_eq!(summary["model"], "gpt-test");
        assert_eq!(summary["reasoning_effort"], "high");
        assert_eq!(summary["state"], "queued");
    }
}
