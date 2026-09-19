use super::{bounded_redacted_text, logging::ExposeAuditLog, now_millis};
use crate::{configure_child_process_group, terminate_child_process_tree};
use anyhow::{Context, Result};
use base64::Engine as _;
use prodex_cli::{SuperArgs, SuperExternalProvider};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_ACTIVE_RUNS: usize = 4;
const OUTPUT_MAX_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunState {
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
    runs: Mutex<BTreeMap<String, RunRecord>>,
}

struct RunRecord {
    state: RunState,
    created_at: u64,
    started_at: Option<u64>,
    finished_at: Option<u64>,
    exit_status: Option<i32>,
    output: String,
    output_truncated: bool,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl RunManager {
    pub(super) fn new(workspace: PathBuf, base_args: SuperArgs, audit: ExposeAuditLog) -> Self {
        Self {
            inner: Arc::new(RunManagerInner {
                workspace,
                base_args,
                audit,
                runs: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    pub(super) fn start(
        &self,
        task: String,
        overrides: &Value,
    ) -> std::result::Result<Value, String> {
        if task.trim().is_empty() || task.len() > 65_536 {
            return Err("task is empty or too large".to_string());
        }
        let mut args = self.inner.base_args.clone();
        apply_overrides(&mut args, overrides)?;
        args.validate_urls().map_err(|error| error.to_string())?;

        let run_id = new_run_id().map_err(|error| error.to_string())?;
        let cancel = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        {
            let mut runs = self
                .inner
                .runs
                .lock()
                .map_err(|_| "run manager unavailable".to_string())?;
            if runs.values().filter(|run| !run.state.terminal()).count() >= MAX_ACTIVE_RUNS {
                self.inner.audit.event(
                    "super_expose_run_rejected",
                    [crate::runtime_proxy_log_field("reason", "active_limit")],
                );
                return Err(format!(
                    "active run limit reached ({MAX_ACTIVE_RUNS}); wait for a run to finish"
                ));
            }
            runs.insert(
                run_id.clone(),
                RunRecord {
                    state: RunState::Starting,
                    created_at: now_millis(),
                    started_at: None,
                    finished_at: None,
                    exit_status: None,
                    output: String::new(),
                    output_truncated: false,
                    cancel: cancel.clone(),
                    child: child.clone(),
                },
            );
        }

        self.inner.audit.event(
            "super_expose_run_created",
            [crate::runtime_proxy_log_field("run_id", run_id.clone())],
        );
        let manager = self.clone();
        let thread_run_id = run_id.clone();
        thread::spawn(move || manager.execute(thread_run_id, task, args, cancel, child));
        self.status(&run_id)
            .ok_or_else(|| "run manager lost new run".to_string())
    }

    pub(super) fn status(&self, run_id: &str) -> Option<Value> {
        let runs = self.inner.runs.lock().ok()?;
        let record = runs.get(run_id)?;
        Some(summary_json(run_id, record))
    }

    pub(super) fn result(&self, run_id: &str) -> Option<Value> {
        let runs = self.inner.runs.lock().ok()?;
        let record = runs.get(run_id)?;
        let mut result = summary_json(run_id, record);
        result["output"] = Value::String(record.output.clone());
        result["output_truncated"] = Value::Bool(record.output_truncated);
        Some(result)
    }

    pub(super) fn list(&self) -> Vec<Value> {
        let Ok(runs) = self.inner.runs.lock() else {
            return Vec::new();
        };
        runs.iter()
            .map(|(run_id, record)| summary_json(run_id, record))
            .collect()
    }

    pub(super) fn cancel(&self, run_id: &str) -> Option<Value> {
        let child = {
            let mut runs = self.inner.runs.lock().ok()?;
            let record = runs.get_mut(run_id)?;
            if record.state.terminal() {
                return Some(summary_json(run_id, record));
            }
            record.cancel.store(true, Ordering::SeqCst);
            self.inner.audit.event(
                "super_expose_run_cancel_requested",
                [crate::runtime_proxy_log_field("run_id", run_id.to_string())],
            );
            record.child.clone()
        };
        if let Ok(mut child) = child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = terminate_child_process_tree(child, true);
        }
        self.status(run_id)
    }
    fn execute(
        &self,
        run_id: String,
        task: String,
        args: SuperArgs,
        cancel: Arc<AtomicBool>,
        child_slot: Arc<Mutex<Option<Child>>>,
    ) {
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                self.inner.audit.event(
                    "super_expose_run_start_failed",
                    [
                        crate::runtime_proxy_log_field("run_id", run_id.clone()),
                        crate::runtime_proxy_log_field("stage", "current_exe"),
                    ],
                );
                self.finish_start_failed(&run_id, &format!("current executable: {error}"));
                return;
            }
        };
        let mut command = Command::new(executable);
        command
            .args(build_child_args(&args))
            .current_dir(&self.inner.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some((name, value)) = api_key_env(&args) {
            command.env(name, value);
        }
        configure_child_process_group(&mut command, true);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.inner.audit.event(
                    "super_expose_run_start_failed",
                    [
                        crate::runtime_proxy_log_field("run_id", run_id.clone()),
                        crate::runtime_proxy_log_field("stage", "spawn"),
                    ],
                );
                self.finish_start_failed(&run_id, &format!("spawn: {error}"));
                return;
            }
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdin = child.stdin.take();
        {
            let Ok(mut slot) = child_slot.lock() else {
                let _ = terminate_child_process_tree(&mut child, true);
                self.finish_start_failed(&run_id, "child state unavailable");
                return;
            };
            *slot = Some(child);
        }
        self.mark_running(&run_id);
        self.inner.audit.event(
            "super_expose_run_started",
            [crate::runtime_proxy_log_field("run_id", run_id.clone())],
        );

        let stdout_reader = stdout.map(|reader| spawn_reader(self.clone(), run_id.clone(), reader));
        let stderr_reader = stderr.map(|reader| spawn_reader(self.clone(), run_id.clone(), reader));
        if let Some(mut stdin) = stdin.take()
            && (stdin.write_all(task.as_bytes()).is_err() || stdin.flush().is_err())
        {
            cancel.store(true, Ordering::SeqCst);
        }

        let status = loop {
            if cancel.load(Ordering::SeqCst)
                && let Ok(mut slot) = child_slot.lock()
                && let Some(child) = slot.as_mut()
            {
                let _ = terminate_child_process_tree(child, true);
            }
            let polled = child_slot
                .lock()
                .ok()
                .and_then(|mut slot| slot.as_mut().map(Child::try_wait));
            match polled {
                Some(Ok(Some(status))) => break Some(status),
                Some(Ok(None)) => thread::sleep(Duration::from_millis(25)),
                Some(Err(_)) | None => break None,
            }
        };
        if let Ok(mut slot) = child_slot.lock() {
            slot.take();
        }
        for reader in [stdout_reader, stderr_reader].into_iter().flatten() {
            let _ = reader.join();
        }

        let Ok(mut runs) = self.inner.runs.lock() else {
            return;
        };
        let Some(record) = runs.get_mut(&run_id) else {
            return;
        };
        record.finished_at = Some(now_millis());
        if cancel.load(Ordering::SeqCst) {
            record.state = RunState::Cancelled;
        } else if let Some(status) = status {
            record.exit_status = status.code();
            record.state = if status.success() {
                RunState::Succeeded
            } else {
                RunState::Failed
            };
        } else {
            record.state = RunState::StartFailed;
        }
        self.inner.audit.event(
            "super_expose_run_completed",
            [
                crate::runtime_proxy_log_field("run_id", run_id.clone()),
                crate::runtime_proxy_log_field("state", record.state.as_str()),
                crate::runtime_proxy_log_field(
                    "exit_code",
                    record
                        .exit_status
                        .map_or_else(|| "none".to_string(), |code| code.to_string()),
                ),
                crate::runtime_proxy_log_field("output_bytes", record.output.len().to_string()),
                crate::runtime_proxy_log_field(
                    "output_truncated",
                    record.output_truncated.to_string(),
                ),
            ],
        );
    }

    fn mark_running(&self, run_id: &str) {
        if let Ok(mut runs) = self.inner.runs.lock()
            && let Some(record) = runs.get_mut(run_id)
        {
            record.state = RunState::Running;
            record.started_at = Some(now_millis());
        }
    }

    fn finish_start_failed(&self, run_id: &str, message: &str) {
        if let Ok(mut runs) = self.inner.runs.lock()
            && let Some(record) = runs.get_mut(run_id)
        {
            record.state = RunState::StartFailed;
            record.finished_at = Some(now_millis());
            append_output(record, message.as_bytes());
        }
    }

    fn append(&self, run_id: &str, bytes: &[u8]) {
        if let Ok(mut runs) = self.inner.runs.lock()
            && let Some(record) = runs.get_mut(run_id)
        {
            append_output(record, bytes);
        }
    }
}

fn spawn_reader(
    manager: RunManager,
    run_id: String,
    mut reader: impl Read + Send + 'static,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => manager.append(&run_id, &buffer[..size]),
                Err(_) => break,
            }
        }
    })
}

fn append_output(record: &mut RunRecord, bytes: &[u8]) {
    let text = bounded_redacted_text(bytes, OUTPUT_MAX_BYTES);
    let remaining = OUTPUT_MAX_BYTES.saturating_sub(record.output.len());
    if remaining == 0 {
        record.output_truncated = true;
        return;
    }
    let mut end = text.len().min(remaining);
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    record.output.push_str(&text[..end]);
    record.output_truncated |= end < text.len();
}

fn summary_json(run_id: &str, record: &RunRecord) -> Value {
    json!({
        "run_id": run_id,
        "state": record.state.as_str(),
        "created_at": record.created_at,
        "started_at": record.started_at,
        "finished_at": record.finished_at,
        "exit_status": record.exit_status,
        "cancellation_requested": record.cancel.load(Ordering::SeqCst),
    })
}

fn new_run_id() -> Result<String> {
    let mut bytes = [0_u8; 12];
    getrandom::fill(&mut bytes).context("failed to generate expose run id")?;
    Ok(format!(
        "spr_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn apply_overrides(args: &mut SuperArgs, values: &Value) -> std::result::Result<(), String> {
    if let Some(profile) = optional_string(values, "profile")? {
        args.profile = Some(profile.to_string());
    }
    if let Some(model) = optional_string(values, "model")? {
        args.local_model = Some(model.to_string());
    }
    if let Some(effort) = optional_string(values, "reasoning_effort")? {
        args.codex_args.extend([
            "-c".into(),
            format!("model_reasoning_effort={}", toml_string(effort)).into(),
        ]);
    }
    if let Some(provider) = optional_string(values, "provider")? {
        let provider = prodex_provider_core::ProviderId::parse(provider)
            .ok_or_else(|| "provider is unsupported".to_string())?;
        args.url = None;
        args.api_key = None;
        args.provider = match provider {
            prodex_provider_core::ProviderId::OpenAi => None,
            prodex_provider_core::ProviderId::Local => {
                return Err(
                    "local provider override requires --url on the exposed base command"
                        .to_string(),
                );
            }
            provider => SuperExternalProvider::from_provider_id(provider),
        };
        if provider != prodex_provider_core::ProviderId::OpenAi && args.provider.is_none() {
            return Err("provider is unsupported".to_string());
        }
    }
    if let Some(sub_agents) = values.get("sub_agents").and_then(Value::as_bool) {
        args.sub_agent = sub_agents;
        args.no_sub_agent = !sub_agents;
    }
    Ok(())
}

fn optional_string<'a>(
    values: &'a Value,
    name: &str,
) -> std::result::Result<Option<&'a str>, String> {
    let Some(value) = values.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(Some)
        .ok_or_else(|| format!("{name} must be a string"))
}

fn toml_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

pub(super) fn build_child_args(args: &SuperArgs) -> Vec<std::ffi::OsString> {
    let mut output = vec!["s".into(), "--full-access".into()];
    push_option(&mut output, "--profile", args.profile.as_deref());
    if args.no_auto_rotate {
        output.push("--no-auto-rotate".into());
    }
    if args.auto_redeem {
        output.push("--auto-redeem".into());
    }
    if args.skip_quota_check {
        output.push("--skip-quota-check".into());
    }
    push_option(&mut output, "--base-url", args.base_url.as_deref());
    if args.no_proxy {
        output.push("--no-proxy".into());
    }
    if args.presidio {
        output.push("--presidio".into());
    } else {
        output.push("--no-presidio".into());
    }
    if args.sub_agent {
        output.push("--sub-agent".into());
        push_option(
            &mut output,
            "--sub-agent-provider",
            args.sub_agent_provider.map(|provider| provider.label()),
        );
        push_option(
            &mut output,
            "--sub-agent-model",
            args.sub_agent_model.as_deref(),
        );
        if let Some(effort) = args.sub_agent_model_reasoning_effort {
            push_option(
                &mut output,
                "--sub-agent-model-reasoning-effort",
                Some(effort.as_str()),
            );
        }
    } else if args.no_sub_agent {
        output.push("--no-sub-agent".into());
    }
    for tool in &args.tools {
        output.extend(["--tool".into(), tool.to_string().into()]);
    }
    for tool in &args.required_tools {
        output.extend(["--require-tool".into(), tool.to_string().into()]);
    }
    push_option(&mut output, "--url", args.url.as_deref());
    if let Some(provider) = args.provider {
        push_option(&mut output, "--provider", Some(provider.as_str()));
    }
    push_option(&mut output, "--model", args.local_model.as_deref());
    if let Some(value) = args.local_context_window {
        output.extend(["--context-window".into(), value.to_string().into()]);
    }
    if let Some(value) = args.local_auto_compact_token_limit {
        output.extend([
            "--auto-compact-token-limit".into(),
            value.to_string().into(),
        ]);
    }
    output.extend(args.codex_features.to_codex_config_args());
    output.extend(args.codex_args.iter().cloned());
    output.extend(["exec".into(), "-".into()]);
    output
}

fn push_option(output: &mut Vec<std::ffi::OsString>, name: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        output.extend([name.into(), value.into()]);
    }
}

fn api_key_env(args: &SuperArgs) -> Option<(&'static str, &str)> {
    let key = args.api_key.as_deref()?;
    Some(match args.provider? {
        SuperExternalProvider::Anthropic => ("ANTHROPIC_API_KEY", key),
        SuperExternalProvider::Copilot => ("GITHUB_COPILOT_API_KEY", key),
        SuperExternalProvider::DeepSeek => ("DEEPSEEK_API_KEY", key),
        SuperExternalProvider::Gemini => ("GEMINI_API_KEY", key),
        SuperExternalProvider::Kiro => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_args_use_normal_super_exec_lifecycle() {
        let prodex_cli::Commands::Super(args) =
            prodex_cli::parse_cli_command_from(["prodex", "s", "--no-presidio"]).unwrap()
        else {
            panic!("expected super args");
        };
        let child = build_child_args(&args);
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
}
