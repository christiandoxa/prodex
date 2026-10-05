use crate::app_state::{AppStateIoExt, ProfileProviderExt};
use crate::{AppPaths, AppState};
use anyhow::{Context, Result};
use prodex_mojo_core::rich::{
    RuntimeUsageLimitInputFormat, ascii_casefold_equal_exact, runtime_session_usage_limit_marker,
};
use rusqlite::OptionalExtension;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "usage_limit_recovery/monitor_workflow.rs"]
mod monitor_workflow;
#[path = "usage_limit_recovery/plan.rs"]
mod plan;
#[path = "usage_limit_recovery/telemetry.rs"]
mod telemetry;
#[path = "usage_limit_recovery/workflow.rs"]
mod workflow;
pub(crate) use monitor_workflow::wait_for_runtime_recovery_round;
pub(crate) use plan::{
    GoalResumeRelaunchPlan, RuntimeUsageLimitResumeOptions, next_observed_runtime_recovery_plan,
    next_runtime_usage_limit_plan, plan_runtime_usage_limit_relaunch,
    runtime_exit_status_is_cancelled,
};
pub(crate) use telemetry::{
    runtime_session_recovery_message, runtime_session_recovery_wait_message,
};
use workflow::{
    RuntimeWorkflowEvidence, RuntimeWorkflowRecoveryClass, observe_runtime_workflow_evidence,
    runtime_workflow_effective_model, runtime_workflow_recovery_class,
};

const GOAL_USAGE_LIMIT_RETRY_INTERVAL: Duration = Duration::from_secs(5);
pub(crate) const RUNTIME_SESSION_CONTINUATION_PROMPT: &str = "Continue the interrupted task from the persisted session. Preserve completed work and do not repeat completed tool calls.";
static RUNTIME_USAGE_LIMIT_MONITOR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct GoalUsageLimitMonitor {
    pub(crate) paths: AppPaths,
    db_path: Option<PathBuf>,
    pub(crate) marker_path: PathBuf,
    session_id: Option<String>,
    session_path: Option<PathBuf>,
    connection: Option<rusqlite::Connection>,
    armed: bool,
    usage_limit_pending: bool,
    workflow_recovery_class: Option<RuntimeWorkflowRecoveryClass>,
    workflow_model: Option<String>,
    workflow_evidence: RuntimeWorkflowEvidence,
    session_goal_present: bool,
    workflow_scan_disabled: bool,
    session_usage_limit_reported: bool,
    session_scan_offset: u64,
    next_retry_at: Instant,
    started_at_ms: i64,
}

impl GoalUsageLimitMonitor {
    pub(crate) fn new(
        paths: AppPaths,
        db_path: Option<PathBuf>,
        marker_path: PathBuf,
        session_id: Option<String>,
    ) -> Self {
        let mut monitor = Self {
            paths,
            db_path,
            marker_path,
            session_id,
            session_path: None,
            connection: None,
            armed: false,
            usage_limit_pending: false,
            workflow_recovery_class: None,
            workflow_model: None,
            workflow_evidence: RuntimeWorkflowEvidence::default(),
            session_goal_present: false,
            workflow_scan_disabled: false,
            session_usage_limit_reported: false,
            session_scan_offset: 0,
            next_retry_at: Instant::now(),
            started_at_ms: current_unix_time_millis(),
        };
        if monitor.session_id.is_some() {
            monitor.session_scan_offset = monitor.session_file_size();
        }
        monitor
    }

    pub(crate) fn take_usage_limit_signal(&mut self) -> Result<Option<String>> {
        self.refresh_session_id()?;
        let Some(session_id) = self.session_id.clone() else {
            return Ok(None);
        };
        let workflow_recovery = self.observe_workflow_recovery(&session_id)?;
        let status = if let Some(db_path) = self.db_path.as_ref() {
            if self.connection.is_none() {
                self.connection = Some(
                    rusqlite::Connection::open_with_flags(
                        db_path,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    )
                    .with_context(|| format!("failed to open {}", db_path.display()))?,
                );
            }
            self.connection
                .as_ref()
                .context("goal usage monitor connection is unavailable")?
                .query_row(
                    "SELECT status, updated_at_ms FROM thread_goals WHERE thread_id = ? ORDER BY updated_at_ms DESC LIMIT 1",
                    [&session_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()
                .with_context(|| format!("failed to read goal status from {}", db_path.display()))?
        } else {
            None
        };
        let normalized = status.as_ref().map(|(status, _)| status.trim());
        self.session_goal_present = status.is_some();
        if let Some(class) = workflow_recovery
            && self.workflow_evidence.safe_to_resume()
            && normalized.is_none_or(goal_status_is_resumable)
        {
            self.workflow_recovery_class = Some(class);
            self.usage_limit_pending = true;
            self.next_retry_at = Instant::now();
        }
        if normalized.is_some_and(|status| {
            ascii_casefold_equal_exact(status, "active")
                .expect("Mojo goal-status comparison failed")
        }) && !self.usage_limit_pending
        {
            self.armed = true;
            return Ok(None);
        }
        let current_attempt_hit_limit = status
            .as_ref()
            .is_some_and(|(_, updated_at_ms)| *updated_at_ms >= self.started_at_ms);
        if !self.usage_limit_pending
            && normalized.is_some_and(|status| {
                ascii_casefold_equal_exact(status, "usage_limited")
                    .expect("Mojo goal-status comparison failed")
            })
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

    pub(crate) fn detect_usage_limit_after_child(&mut self) -> Result<Option<String>> {
        self.refresh_session_id()?;
        if self.session_usage_limit_reported {
            return Ok(None);
        }
        let Some(session_id) = self.session_id.clone() else {
            return Ok(None);
        };
        let state = AppState::load_and_repair(&self.paths)?;
        let report = match prodex_session_store::resolve_session_report_by_id_in_store(
            &self.paths.shared_codex_root,
            &state,
            &session_id,
        ) {
            Ok(report) => report,
            Err(prodex_session_store::SessionResolveError::Missing { .. }) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if let Some(model) = report.last_model() {
            self.workflow_model = Some(model.to_string());
        }
        let path = Path::new(&report.path);
        if !self.session_is_resumable(path, &session_id)? {
            return Ok(None);
        }
        if self.scan_usage_limit_session(path, &session_id)? {
            self.session_usage_limit_reported = true;
            return Ok(Some(session_id));
        }
        Ok(None)
    }

    fn scan_usage_limit_session(&mut self, path: &Path, session_id: &str) -> Result<bool> {
        let file_len = prodex_session_store::session_file_logical_len(path).unwrap_or_else(|_| {
            fs::metadata(path)
                .map(|metadata| metadata.len())
                .unwrap_or(0)
        });
        let scan_offset = if self.session_scan_offset > file_len {
            0
        } else {
            self.session_scan_offset
        };
        let mut workflow_class = None;
        let mut legacy_usage_limit = false;
        let mut model = self.workflow_model.clone();
        let mut evidence = self.workflow_evidence;
        let scan = match prodex_session_store::session_file_scan_since(path, scan_offset, |line| {
            observe_usage_limit_scan_line(
                line,
                session_id,
                &mut model,
                &mut evidence,
                &mut workflow_class,
                &mut legacy_usage_limit,
            )
        }) {
            Ok(scan) => scan,
            Err(_) => {
                self.workflow_scan_disabled = true;
                return Ok(false);
            }
        };
        let saw_usage_limit = scan.matched;
        self.session_scan_offset = scan.complete_offset;
        self.workflow_model = model;
        self.workflow_evidence = evidence;
        if usage_limit_recovery_is_ready(
            saw_usage_limit,
            legacy_usage_limit,
            self.session_goal_present,
            workflow_class,
            self.workflow_evidence,
        ) {
            self.workflow_recovery_class = workflow_class;
            return Ok(true);
        }
        Ok(false)
    }

    fn session_is_resumable(&mut self, path: &Path, session_id: &str) -> Result<bool> {
        let Some(db_path) = self.db_path.clone() else {
            return Ok(true);
        };
        if self.connection.is_none() {
            self.connection = Some(
                rusqlite::Connection::open_with_flags(
                    &db_path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )
                .with_context(|| format!("failed to open {}", db_path.display()))?,
            );
        }
        let connection = self
            .connection
            .as_ref()
            .context("goal usage monitor connection is unavailable")?;
        let thread_id = session_file_thread_id(path)?.unwrap_or_else(|| session_id.to_string());
        let status = goal_status_for_thread(connection, &db_path, &thread_id)?;
        self.session_goal_present = status.is_some();
        Ok(status.is_none_or(|status| goal_status_is_resumable(&status)))
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
        self.session_path = None;
        self.armed = false;
        self.usage_limit_pending = false;
        self.workflow_recovery_class = None;
        self.workflow_model = None;
        self.workflow_evidence = RuntimeWorkflowEvidence::default();
        self.session_goal_present = false;
        self.workflow_scan_disabled = false;
        self.session_usage_limit_reported = false;
        self.session_scan_offset =
            fs::read_to_string(runtime_goal_session_offset_path(&self.marker_path))
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_else(|| self.session_file_size());
        self.next_retry_at = Instant::now();
        Ok(())
    }

    pub(crate) fn prepare_for_resume(&mut self) {
        self.armed = false;
        self.usage_limit_pending = false;
        self.workflow_recovery_class = None;
        self.workflow_evidence = RuntimeWorkflowEvidence::default();
        self.workflow_scan_disabled = false;
        self.session_path = None;
        self.session_usage_limit_reported = false;
        self.session_scan_offset = self.session_file_size();
        self.next_retry_at = Instant::now();
        self.started_at_ms = current_unix_time_millis();
    }

    fn session_file_size(&self) -> u64 {
        let Some(session_id) = self.session_id.as_deref() else {
            return 0;
        };
        let Ok(state) = AppState::load_and_repair(&self.paths) else {
            return 0;
        };
        prodex_session_store::resolve_session_report_by_id_in_store(
            &self.paths.shared_codex_root,
            &state,
            session_id,
        )
        .ok()
        .and_then(|report| {
            prodex_session_store::session_file_logical_len(Path::new(&report.path)).ok()
        })
        .unwrap_or(0)
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
        let _ = fs::remove_file(runtime_goal_session_offset_path(&self.marker_path));
    }
}

pub(crate) fn prepare_goal_usage_limit_monitor(
    codex_args: &[std::ffi::OsString],
    disabled: bool,
) -> Result<Option<GoalUsageLimitMonitor>> {
    let Some(mut monitor) = prepare_runtime_usage_limit_monitor(codex_args, disabled)? else {
        return Ok(None);
    };
    let db_path = monitor.paths.shared_codex_root.join("goals_1.sqlite");
    if goal_database_is_file(&db_path)? {
        let connection = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("failed to open {}", db_path.display()))?;
        if goal_database_has_thread_goals(&connection)? {
            monitor.db_path = Some(db_path);
        }
    }
    Ok(Some(monitor))
}

pub(crate) fn prepare_runtime_usage_limit_monitor(
    codex_args: &[std::ffi::OsString],
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
    let sequence = RUNTIME_USAGE_LIMIT_MONITOR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
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
        paths,
        None,
        marker_path,
        session_id,
    )))
}

pub(crate) fn runtime_goal_monitor_dir(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-goal-monitors")
}

pub(crate) fn runtime_goal_session_offset_path(marker_path: &Path) -> PathBuf {
    marker_path.with_extension("offset")
}

fn goal_resume_line_has_usage_limit(line: &str) -> bool {
    if line.len() > prodex_mojo_core::rich::RUNTIME_ERROR_SESSION_USAGE_LIMIT_MAX_BYTES {
        return false;
    }
    let trimmed = line.trim();
    let (input, format) = match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => (
            serde_json::to_string(&value).expect("parsed Serde JSON value must serialize"),
            RuntimeUsageLimitInputFormat::Json,
        ),
        Err(_) => (line.to_string(), RuntimeUsageLimitInputFormat::PlainText),
    };
    runtime_session_usage_limit_marker(&input, format)
        .expect("Mojo session usage-limit classifier returned invalid output")
}

fn observe_usage_limit_scan_line(
    line: &str,
    session_id: &str,
    model: &mut Option<String>,
    evidence: &mut RuntimeWorkflowEvidence,
    workflow_class: &mut Option<RuntimeWorkflowRecoveryClass>,
    legacy_usage_limit: &mut bool,
) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    if let Some(observed) = runtime_workflow_effective_model(&value) {
        *model = Some(observed);
    }
    observe_runtime_workflow_evidence(&value, evidence);
    *workflow_class = runtime_workflow_recovery_class(&value, session_id);
    if workflow_class.is_some() {
        return true;
    }
    *legacy_usage_limit = goal_resume_line_has_usage_limit(line);
    *legacy_usage_limit
}

fn usage_limit_recovery_is_ready(
    saw_usage_limit: bool,
    legacy_usage_limit: bool,
    session_goal_present: bool,
    workflow_class: Option<RuntimeWorkflowRecoveryClass>,
    evidence: RuntimeWorkflowEvidence,
) -> bool {
    saw_usage_limit
        && (legacy_usage_limit && (session_goal_present || evidence.safe_to_resume())
            || workflow_class.is_some() && evidence.safe_to_resume())
}

pub(crate) fn goal_database_is_file(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub(crate) fn goal_database_has_thread_goals(conn: &rusqlite::Connection) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'thread_goals'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn session_file_thread_id(path: &Path) -> Result<Option<String>> {
    let mut thread_id = None;
    let mut session_meta_id = None;
    let _ = prodex_session_store::session_file_has_line_since(path, 0, |line| {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        thread_id = prodex_session_store::first_string_value(
            &value,
            &[&["payload", "thread_id"], &["thread_id"], &["threadId"]],
        );
        if thread_id.is_none()
            && session_meta_id.is_none()
            && value.get("type").and_then(serde_json::Value::as_str) == Some("session_meta")
        {
            session_meta_id = prodex_session_store::first_string_value(
                &value,
                &[
                    &["payload", "id"],
                    &["id"],
                    &["payload", "session_id"],
                    &["session_id"],
                ],
            );
        }
        thread_id.is_some()
    })?;
    Ok(thread_id.or(session_meta_id))
}

fn goal_status_for_thread(
    connection: &rusqlite::Connection,
    db_path: &Path,
    thread_id: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT status FROM thread_goals WHERE thread_id = ? ORDER BY updated_at_ms DESC LIMIT 1",
            [thread_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .with_context(|| format!("failed to read goal status from {}", db_path.display()))
}

fn goal_status_is_resumable(status: &str) -> bool {
    let status = status.trim();
    ["active", "paused", "blocked", "usage_limited"]
        .into_iter()
        .any(|expected| {
            ascii_casefold_equal_exact(status, expected)
                .expect("Mojo resumable goal-status comparison failed")
        })
}

#[cfg(test)]
#[path = "usage_limit_recovery/tests.rs"]
mod tests;
