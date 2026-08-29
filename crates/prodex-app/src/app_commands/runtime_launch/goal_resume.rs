#[cfg(test)]
use super::usage_limit_recovery::GoalUsageLimitMonitor;
use super::usage_limit_recovery::{
    GoalResumeRelaunchPlan, RuntimeUsageLimitResumeOptions, next_runtime_usage_limit_plan,
    plan_runtime_usage_limit_relaunch, runtime_goal_monitor_dir, runtime_goal_session_offset_path,
};
#[cfg(test)]
use super::usage_limit_recovery::{goal_database_has_thread_goals, goal_database_is_file};
use super::{
    RunCommandStrategy, clear_codex_session_binding, codex_cli_config_override_value,
    remove_first_codex_config_override_pair, restore_resume_session_settings,
    runtime_launch_cli_model, runtime_resume_external_provider_from_codex_args,
    runtime_resume_session_settings_from_codex_args, super_external_provider_codex_args,
};
use crate::app_state::AppStateIoExt;
use crate::{
    AppPaths, AppState, codex_cli_config_override_exact_value, codex_profile_v2_config_path,
};
use anyhow::{Context, Result, bail};
#[cfg(test)]
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::collections::VecDeque;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
#[cfg(test)]
use std::fs::File;
use std::io::Read;
#[cfg(test)]
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const RUNTIME_GOAL_SESSION_NOTIFY_COMMAND: &str = "__runtime-goal-session-notify";
const RUNTIME_GOAL_SESSION_HOOK_TIMEOUT_SECS: u64 = 5;
const RUNTIME_GOAL_SESSION_NOTIFY_MAX_PAYLOAD_BYTES: u64 = 64 * 1024;
#[cfg(not(windows))]
const RUNTIME_GOAL_SESSION_HOOK_KEY: &str = "/<session-flags>/config.toml:session_start:0:0";
#[cfg(windows)]
const RUNTIME_GOAL_SESSION_HOOK_KEY: &str = r"C:\<session-flags>\config.toml:session_start:0:0";
#[cfg(test)]
static RUNTIME_GOAL_MONITOR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl RunCommandStrategy {
    #[cfg(test)]
    pub(super) fn resume_session_id(&self) -> Option<&str> {
        prodex_runtime_launch::codex_resume_session_id(&self.codex_args)
    }

    pub(super) fn plan_goal_resume_relaunch(
        &mut self,
        status: &std::process::ExitStatus,
    ) -> Result<Option<GoalResumeRelaunchPlan>> {
        let Some(monitor) = self.goal_usage_limit_monitor.as_mut() else {
            return Ok(None);
        };
        plan_runtime_usage_limit_relaunch(
            monitor,
            status,
            &RuntimeUsageLimitResumeOptions {
                requested_profile: self.args.profile.as_deref(),
                no_auto_rotate: self.args.no_auto_rotate,
                skip_quota_check: self.args.skip_quota_check,
                base_url: self.args.base_url.as_deref(),
                include_code_review: self.include_code_review,
                no_proxy: self.args.no_proxy,
                attempted_profiles: &self.auto_goal_resume_attempted_profiles,
            },
        )
    }

    fn next_goal_resume_plan(
        &self,
        state: &AppState,
        session_id: &str,
    ) -> Option<GoalResumeRelaunchPlan> {
        next_runtime_usage_limit_plan(
            state,
            session_id,
            &RuntimeUsageLimitResumeOptions {
                requested_profile: self.args.profile.as_deref(),
                no_auto_rotate: self.args.no_auto_rotate,
                skip_quota_check: self.args.skip_quota_check,
                base_url: self.args.base_url.as_deref(),
                include_code_review: self.include_code_review,
                no_proxy: self.args.no_proxy,
                attempted_profiles: &self.auto_goal_resume_attempted_profiles,
            },
        )
    }

    pub(super) fn apply_goal_resume_relaunch(
        &mut self,
        plan: GoalResumeRelaunchPlan,
    ) -> Result<()> {
        let session_settings = runtime_resume_session_settings_from_codex_args(&self.codex_args);
        let model_is_explicit = runtime_launch_cli_model(&self.codex_args).is_some()
            || codex_cli_config_override_value(&self.codex_args, "model").is_some();
        let effort_is_explicit =
            codex_cli_config_override_value(&self.codex_args, "model_reasoning_effort").is_some();
        let exec_mode = prodex_runtime_launch::is_codex_exec_invocation(&self.codex_args);
        clear_codex_session_binding(&plan.session_id)?;
        self.goal_resume_session_affinity_release = Some(plan.session_id.clone());
        self.codex_args = if exec_mode {
            prodex_runtime_launch::retarget_codex_exec_resume_args(
                &self.codex_args,
                &plan.session_id,
            )
        } else {
            prodex_runtime_launch::retarget_codex_tui_resume_args(
                &self.codex_args,
                &plan.session_id,
            )
        };
        let session_settings = session_settings
            .or_else(|| runtime_resume_session_settings_from_codex_args(&self.codex_args));
        restore_resume_session_settings(
            &mut self.codex_args,
            session_settings.as_ref(),
            model_is_explicit,
            effort_is_explicit,
        );

        if self.model_provider_override.is_none()
            && let Some(provider) =
                runtime_resume_external_provider_from_codex_args(&self.codex_args)?
        {
            let base_url = self
                .args
                .base_url
                .as_deref()
                .unwrap_or_else(|| provider.default_base_url());
            let mut provider_args =
                super_external_provider_codex_args(provider, base_url, None, None, None);
            remove_first_codex_config_override_pair(&mut provider_args, "model");
            let mut next_args = Vec::with_capacity(provider_args.len() + self.codex_args.len());
            next_args.extend(provider_args);
            next_args.extend(std::mem::take(&mut self.codex_args));
            self.codex_args = next_args;
            self.auto_external_provider = Some(provider);
            self.auto_external_provider_base_url = Some(base_url.to_string());
            self.model_provider_override = Some(provider.model_provider_id().to_string());
            self.model_context_window_tokens =
                super::runtime_launch_cli_model_context_window_tokens(&self.codex_args);
            self.gemini_thinking_budget_tokens =
                super::runtime_launch_cli_gemini_thinking_budget_tokens(&self.codex_args);
        }
        self.auto_goal_resume_attempted_profiles
            .insert(plan.profile_name.clone());
        self.args.profile = Some(plan.profile_name);
        if let Some(monitor) = self.goal_usage_limit_monitor.as_mut() {
            monitor.prepare_for_resume();
        }
        if exec_mode {
            self.codex_args.push(OsString::from(
                "Continue the interrupted task from the persisted session. Preserve completed work and do not repeat completed tool calls.",
            ));
        } else if !codex_args_include_goal_resume(&self.codex_args) {
            self.codex_args.push(OsString::from("/goal resume"));
        }
        Ok(())
    }

    pub(super) fn plan_live_goal_resume_relaunch(
        &self,
        session_id: &str,
    ) -> Result<Option<GoalResumeRelaunchPlan>> {
        let paths = AppPaths::discover()?;
        let state = AppState::load_and_repair(&paths)?;
        Ok(self.next_goal_resume_plan(&state, session_id))
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(super) struct GoalResumeSessionAnalysis {
    pub(super) thread_id: Option<String>,
    pub(super) saw_usage_limit: bool,
}

fn codex_notify_is_configured(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
    codex_args: &[OsString],
) -> Result<bool> {
    if codex_cli_config_override_exact_value(codex_args, "notify").is_some() {
        return Ok(true);
    }
    let config_has_notify = |path: &Path| -> Result<bool> {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        };
        let value = toml::from_str::<toml::Value>(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(value.get("notify").is_some())
    };
    Ok(profile_v2_name
        .and_then(|name| codex_profile_v2_config_path(codex_home, name))
        .map(|path| config_has_notify(&path))
        .transpose()?
        .unwrap_or(false)
        || config_has_notify(&codex_home.join("config.toml"))?)
}

pub(crate) fn add_runtime_goal_session_tracking(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
    codex_args: &mut Vec<OsString>,
    marker_path: &Path,
) -> Result<()> {
    let current_exe = env::current_exe().context("failed to resolve current executable")?;
    let command_argv = [
        current_exe.to_string_lossy().into_owned(),
        RUNTIME_GOAL_SESSION_NOTIFY_COMMAND.to_string(),
        marker_path.to_string_lossy().into_owned(),
    ];
    let mut injected = Vec::new();

    if codex_cli_config_override_value(codex_args, "hooks.SessionStart").is_none() {
        let command = runtime_goal_session_hook_command(&command_argv);
        let command_literal = serde_json::to_string(&command).expect("string serialization");
        let hook_hash = runtime_goal_session_hook_hash(&command);
        let hook_key_literal =
            serde_json::to_string(RUNTIME_GOAL_SESSION_HOOK_KEY).expect("string serialization");
        let hook_hash_literal = serde_json::to_string(&hook_hash).expect("string serialization");
        injected.extend([
            OsString::from("-c"),
            OsString::from(format!(
                "hooks.SessionStart=[{{hooks=[{{type=\"command\",command={command_literal},timeout={RUNTIME_GOAL_SESSION_HOOK_TIMEOUT_SECS}}}]}}]"
            )),
            OsString::from("-c"),
            OsString::from(format!(
                "hooks.state={{{hook_key_literal}={{trusted_hash={hook_hash_literal}}}}}"
            )),
        ]);
    }

    if !codex_notify_is_configured(codex_home, profile_v2_name, codex_args)? {
        let command = serde_json::to_string(&command_argv)
            .context("failed to serialize goal session hook")?;
        injected.extend([
            OsString::from("-c"),
            OsString::from(format!("notify={command}")),
        ]);
    }

    codex_args.splice(0..0, injected);
    Ok(())
}

fn runtime_goal_session_hook_command(argv: &[String]) -> String {
    #[cfg(not(windows))]
    {
        argv.iter()
            .map(|arg| format!("'{}'", arg.replace('\'', "'\"'\"'")))
            .collect::<Vec<_>>()
            .join(" ")
    }
    #[cfg(windows)]
    {
        argv.iter()
            .map(|arg| format!(r#""{}""#, arg.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub(super) fn runtime_goal_session_hook_hash(command: &str) -> String {
    // Match Codex's normalized unmanaged-hook identity so only this generated hook is trusted.
    let command = serde_json::to_string(command).expect("string serialization");
    let identity = format!(
        "{{\"event_name\":\"session_start\",\"hooks\":[{{\"async\":false,\"command\":{command},\"timeout\":{RUNTIME_GOAL_SESSION_HOOK_TIMEOUT_SECS},\"type\":\"command\"}}]}}"
    );
    let digest = Sha256::digest(identity.as_bytes());
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

pub(crate) fn handle_runtime_goal_session_notify_if_requested() -> Result<bool> {
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(RUNTIME_GOAL_SESSION_NOTIFY_COMMAND)) {
        return Ok(false);
    }
    let marker_path = args
        .next()
        .map(PathBuf::from)
        .context("runtime goal session notify requires a marker path")?;
    let payload_arg = args.next();
    if args.next().is_some() {
        bail!("runtime goal session notify accepts at most one payload argument");
    }
    let payload = match payload_arg {
        Some(payload) => payload,
        None => {
            let mut payload = String::new();
            std::io::stdin()
                .lock()
                .take(RUNTIME_GOAL_SESSION_NOTIFY_MAX_PAYLOAD_BYTES + 1)
                .read_to_string(&mut payload)
                .context("failed to read runtime goal session notify payload")?;
            if payload.len() as u64 > RUNTIME_GOAL_SESSION_NOTIFY_MAX_PAYLOAD_BYTES {
                bail!("runtime goal session notify payload exceeds 64 KiB");
            }
            OsString::from(payload)
        }
    };
    write_runtime_goal_session_marker(&marker_path, &payload)?;
    Ok(true)
}

pub(super) fn write_runtime_goal_session_marker(marker_path: &Path, payload: &OsStr) -> Result<()> {
    let paths = AppPaths::discover()?;
    if marker_path.parent() != Some(runtime_goal_monitor_dir(&paths).as_path()) {
        bail!("invalid runtime goal session marker path");
    }
    let payload: serde_json::Value = serde_json::from_str(&payload.to_string_lossy())?;
    let session_id = payload
        .get("thread-id")
        .or_else(|| payload.get("session_id"))
        .and_then(serde_json::Value::as_str)
        .context("runtime goal session payload is missing a session id")?;
    uuid::Uuid::parse_str(session_id).context("invalid runtime goal session id")?;
    let offset = AppState::load_and_repair(&paths)
        .ok()
        .and_then(|state| {
            prodex_session_store::resolve_session_report_by_id_in_store(
                &paths.shared_codex_root,
                &state,
                session_id,
            )
            .ok()
        })
        .and_then(|report| fs::metadata(report.path).ok())
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    fs::write(
        runtime_goal_session_offset_path(marker_path),
        format!("{offset}\n"),
    )?;
    fs::write(marker_path, format!("{session_id}\n"))?;
    Ok(())
}

pub(super) fn codex_args_include_goal_resume(codex_args: &[OsString]) -> bool {
    codex_args.iter().any(|arg| {
        arg.to_string_lossy()
            .trim()
            .eq_ignore_ascii_case("/goal resume")
    })
}

#[cfg(test)]
pub(super) fn analyze_goal_resume_session(path: &Path) -> Result<GoalResumeSessionAnalysis> {
    let file = File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut analysis = GoalResumeSessionAnalysis::default();
    let mut tail = VecDeque::with_capacity(200);
    for (line_number, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        let value = serde_json::from_str::<serde_json::Value>(&line).with_context(|| {
            format!(
                "failed to parse {} line {}",
                path.display(),
                line_number + 1
            )
        })?;
        if analysis.thread_id.is_none() {
            analysis.thread_id = prodex_session_store::first_string_value(
                &value,
                &[&["payload", "thread_id"], &["thread_id"], &["threadId"]],
            );
        }
        if tail.len() == 200 {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    analysis.saw_usage_limit = tail
        .iter()
        .any(|line| runtime_proxy_crate::runtime_usage_limit_text_message(line));
    Ok(analysis)
}

#[cfg(test)]
pub(super) fn shared_goal_needs_resume(shared_codex_root: &Path, thread_id: &str) -> Result<bool> {
    let db_path = shared_codex_root.join("goals_1.sqlite");
    if !goal_database_is_file(&db_path)? {
        return Ok(false);
    }
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open {}", db_path.display()))?;
    if !goal_database_has_thread_goals(&conn)? {
        return Ok(false);
    }
    let status = conn
        .query_row(
            "SELECT status FROM thread_goals WHERE thread_id = ? ORDER BY updated_at_ms DESC LIMIT 1",
            [thread_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(status.is_some_and(|status| {
        let normalized = status.trim().to_ascii_lowercase();
        !matches!(normalized.as_str(), "complete" | "completed")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_resume_read_parse_and_validation_errors_are_propagated() {
        let root = std::env::temp_dir().join(format!(
            "prodex-goal-resume-errors-{}-{}",
            std::process::id(),
            RUNTIME_GOAL_MONITOR_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();

        let missing = analyze_goal_resume_session(&root.join("missing.jsonl")).unwrap_err();
        assert!(missing.to_string().contains("failed to read"));

        let session_path = root.join("session.jsonl");
        fs::write(&session_path, "not-json\n").unwrap();
        let parse_error = analyze_goal_resume_session(&session_path).unwrap_err();
        assert!(parse_error.to_string().contains("failed to parse"));

        let marker_path = root.join("session.id");
        fs::write(&marker_path, "not-a-uuid\n").unwrap();
        let mut monitor = GoalUsageLimitMonitor::new(
            AppPaths::discover().unwrap(),
            Some(root.join("goals.sqlite")),
            marker_path,
            None,
        );
        let validation_error = monitor.take_usage_limit_signal().unwrap_err();
        assert!(
            validation_error
                .to_string()
                .contains("invalid runtime goal session id")
        );

        let _ = fs::remove_dir_all(root);
    }
}
