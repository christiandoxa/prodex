use super::{
    RunCommandStrategy, active_profile_selection_order, clear_codex_session_binding,
    find_ready_profiles, goal_resume_line_has_usage_limit,
};
use crate::app_state::{AppStateIoExt, ProfileProviderExt};
use crate::{
    AppPaths, AppState, codex_cli_config_override_exact_value, codex_cli_config_override_value,
    codex_profile_v2_config_path,
};
use anyhow::{Context, Result, bail};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) const RUNTIME_GOAL_SESSION_NOTIFY_COMMAND: &str = "__runtime-goal-session-notify";
const RUNTIME_GOAL_SESSION_HOOK_TIMEOUT_SECS: u64 = 5;
const RUNTIME_GOAL_SESSION_NOTIFY_MAX_PAYLOAD_BYTES: u64 = 64 * 1024;
#[cfg(not(windows))]
const RUNTIME_GOAL_SESSION_HOOK_KEY: &str = "/<session-flags>/config.toml:session_start:0:0";
#[cfg(windows)]
const RUNTIME_GOAL_SESSION_HOOK_KEY: &str = r"C:\<session-flags>\config.toml:session_start:0:0";
const GOAL_USAGE_LIMIT_RETRY_INTERVAL: Duration = Duration::from_secs(5);
static RUNTIME_GOAL_MONITOR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl RunCommandStrategy {
    pub(super) fn resume_session_id(&self) -> Option<&str> {
        prodex_runtime_launch::codex_resume_session_id(&self.codex_args)
    }

    pub(super) fn plan_goal_resume_relaunch(
        &self,
        status: &std::process::ExitStatus,
    ) -> Result<Option<GoalResumeRelaunchPlan>> {
        if status.success() || self.args.no_auto_rotate {
            return Ok(None);
        }
        let Some(session_selector) = self.resume_session_id() else {
            return Ok(None);
        };
        let paths = AppPaths::discover()?;
        let state = AppState::load_and_repair(&paths)?;
        let report = prodex_session_store::resolve_session_report_by_id_in_store(
            &paths.shared_codex_root,
            &state,
            session_selector,
        )
        .with_context(|| "failed to resolve goal resume session")?;
        let analysis = analyze_goal_resume_session(Path::new(&report.path))?;
        if !analysis.saw_usage_limit {
            return Ok(None);
        }
        let thread_id = analysis
            .thread_id
            .context("goal resume session is missing a thread id")?;
        if !shared_goal_needs_resume(&paths.shared_codex_root, &thread_id)? {
            return Ok(None);
        }
        Ok(self.next_goal_resume_plan(&state, &report.id))
    }

    fn next_goal_resume_plan(
        &self,
        state: &AppState,
        session_id: &str,
    ) -> Option<GoalResumeRelaunchPlan> {
        let failed_profile = state
            .session_profile_bindings
            .get(session_id)
            .map(|binding| binding.profile_name.clone())
            .or_else(|| self.args.profile.clone())
            .or_else(|| state.active_profile.clone())
            .unwrap_or_default();
        let candidates = if self.args.skip_quota_check {
            active_profile_selection_order(state, &failed_profile)
        } else {
            find_ready_profiles(
                state,
                &failed_profile,
                self.args.base_url.as_deref(),
                self.include_code_review,
                self.args.no_proxy,
            )
        };
        candidates
            .into_iter()
            .filter(|candidate| candidate != &failed_profile)
            .filter(|candidate| !self.auto_goal_resume_attempted_profiles.contains(candidate))
            .find(|candidate| {
                state.profiles.get(candidate).is_some_and(|profile| {
                    profile.provider.supports_codex_runtime()
                        && profile
                            .provider
                            .auth_summary(&profile.codex_home)
                            .quota_compatible
                })
            })
            .map(|profile_name| GoalResumeRelaunchPlan {
                session_id: session_id.to_string(),
                profile_name,
            })
    }

    pub(super) fn apply_goal_resume_relaunch(
        &mut self,
        plan: GoalResumeRelaunchPlan,
    ) -> Result<()> {
        clear_codex_session_binding(&plan.session_id)?;
        self.goal_resume_session_affinity_release = Some(plan.session_id.clone());
        if self.resume_session_id().is_none() {
            self.codex_args = prodex_runtime_launch::retarget_codex_tui_resume_args(
                &self.codex_args,
                &plan.session_id,
            );
        }
        self.auto_goal_resume_attempted_profiles
            .insert(plan.profile_name.clone());
        self.args.profile = Some(plan.profile_name);
        if let Some(monitor) = self.goal_usage_limit_monitor.as_mut() {
            monitor.prepare_for_resume();
        }
        if !codex_args_include_goal_resume(&self.codex_args) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GoalResumeRelaunchPlan {
    pub(super) session_id: String,
    pub(super) profile_name: String,
}

#[derive(Debug, Default)]
pub(super) struct GoalResumeSessionAnalysis {
    pub(super) thread_id: Option<String>,
    pub(super) saw_usage_limit: bool,
}

pub(super) struct GoalUsageLimitMonitor {
    db_path: PathBuf,
    pub(super) marker_path: PathBuf,
    session_id: Option<String>,
    connection: Option<rusqlite::Connection>,
    armed: bool,
    usage_limit_pending: bool,
    next_retry_at: Instant,
    started_at_ms: i64,
}

impl GoalUsageLimitMonitor {
    fn new(db_path: PathBuf, marker_path: PathBuf, session_id: Option<String>) -> Self {
        Self {
            db_path,
            marker_path,
            session_id,
            connection: None,
            armed: false,
            usage_limit_pending: false,
            next_retry_at: Instant::now(),
            started_at_ms: current_unix_time_millis(),
        }
    }

    pub(super) fn take_usage_limit_signal(&mut self) -> Result<Option<String>> {
        self.refresh_session_id()?;
        let Some(session_id) = self.session_id.clone() else {
            return Ok(None);
        };
        if self.connection.is_none() {
            self.connection = Some(
                rusqlite::Connection::open_with_flags(
                    &self.db_path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )
                .with_context(|| format!("failed to open {}", self.db_path.display()))?,
            );
        }
        let status = self
            .connection
            .as_ref()
            .context("goal usage monitor connection is unavailable")?
            .query_row(
                "SELECT status, updated_at_ms FROM thread_goals WHERE thread_id = ? ORDER BY updated_at_ms DESC LIMIT 1",
                [&session_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .with_context(|| format!("failed to read goal status from {}", self.db_path.display()))?;
        let normalized = status
            .as_ref()
            .map(|(status, _)| status.trim().to_ascii_lowercase());
        if normalized.as_deref() == Some("active") {
            self.armed = true;
            self.usage_limit_pending = false;
            return Ok(None);
        }
        let current_attempt_hit_limit = status
            .as_ref()
            .is_some_and(|(_, updated_at_ms)| *updated_at_ms >= self.started_at_ms);
        if !self.usage_limit_pending
            && normalized.as_deref() == Some("usage_limited")
            && (self.armed || current_attempt_hit_limit)
        {
            self.armed = false;
            self.usage_limit_pending = true;
            self.next_retry_at = Instant::now();
        }
        if self.usage_limit_pending && Instant::now() >= self.next_retry_at {
            self.next_retry_at = Instant::now() + GOAL_USAGE_LIMIT_RETRY_INTERVAL;
            return Ok(Some(session_id));
        }
        Ok(None)
    }

    fn refresh_session_id(&mut self) -> Result<()> {
        let raw = match fs::read_to_string(&self.marker_path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", self.marker_path.display()));
            }
        };
        let session_id = raw.trim();
        uuid::Uuid::parse_str(session_id).context("invalid runtime goal session id")?;
        if self.session_id.as_deref() == Some(session_id) {
            return Ok(());
        }
        self.session_id = Some(session_id.to_string());
        self.armed = false;
        self.usage_limit_pending = false;
        self.next_retry_at = Instant::now();
        Ok(())
    }

    fn prepare_for_resume(&mut self) {
        self.armed = false;
        self.usage_limit_pending = false;
        self.next_retry_at = Instant::now();
        self.started_at_ms = current_unix_time_millis();
    }
}

fn current_unix_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

impl Drop for GoalUsageLimitMonitor {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.marker_path);
    }
}

pub(super) fn prepare_goal_usage_limit_monitor(
    codex_args: &[OsString],
    disabled: bool,
) -> Result<Option<GoalUsageLimitMonitor>> {
    if disabled {
        return Ok(None);
    }
    let paths = AppPaths::discover()?;
    let state = AppState::load_and_repair(&paths)?;
    let rotatable_profile_count = state
        .profiles
        .values()
        .filter(|profile| {
            profile.provider.supports_codex_runtime()
                && profile
                    .provider
                    .auth_summary(&profile.codex_home)
                    .quota_compatible
        })
        .count();
    if rotatable_profile_count < 2 {
        return Ok(None);
    }
    let db_path = paths.shared_codex_root.join("goals_1.sqlite");
    if !goal_database_is_file(&db_path)? {
        return Ok(None);
    }
    let connection = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open {}", db_path.display()))?;
    if !goal_database_has_thread_goals(&connection)? {
        return Ok(None);
    }
    let session_id = prodex_runtime_launch::codex_resume_session_id(codex_args)
        .map(|selector| {
            prodex_session_store::resolve_session_report_by_id_in_store(
                &paths.shared_codex_root,
                &state,
                selector,
            )
            .with_context(|| "failed to resolve goal resume session")
            .map(|report| report.id)
        })
        .transpose()?;
    let marker_dir = runtime_goal_monitor_dir(&paths);
    fs::create_dir_all(&marker_dir)
        .with_context(|| format!("failed to create {}", marker_dir.display()))?;
    let sequence = RUNTIME_GOAL_MONITOR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let marker_path = marker_dir.join(format!("session-{}-{sequence}.id", std::process::id()));
    match fs::remove_file(&marker_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to clear {}", marker_path.display()));
        }
    }
    Ok(Some(GoalUsageLimitMonitor::new(
        db_path,
        marker_path,
        session_id,
    )))
}

pub(super) fn runtime_goal_monitor_dir(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-goal-monitors")
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

pub(super) fn add_runtime_goal_session_tracking(
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
        .any(|line| goal_resume_line_has_usage_limit(line));
    Ok(analysis)
}

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

fn goal_database_is_file(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn goal_database_has_thread_goals(conn: &rusqlite::Connection) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'thread_goals'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
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
        let mut monitor = GoalUsageLimitMonitor::new(root.join("goals.sqlite"), marker_path, None);
        let validation_error = monitor.take_usage_limit_signal().unwrap_err();
        assert!(
            validation_error
                .to_string()
                .contains("invalid runtime goal session id")
        );

        let _ = fs::remove_dir_all(root);
    }
}
