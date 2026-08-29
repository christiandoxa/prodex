use crate::app_state::{AppStateIoExt, ProfileProviderExt};
use crate::{AppPaths, AppState};
use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use std::collections::BTreeSet;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GOAL_USAGE_LIMIT_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const OBSERVED_USAGE_LIMIT_MESSAGE: &str = "You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 5:08 PM.";
const GOAL_USAGE_LIMIT_JSON_SCAN_LIMIT: usize = 2_048;
static RUNTIME_USAGE_LIMIT_MONITOR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalResumeRelaunchPlan {
    pub(crate) session_id: String,
    pub(crate) profile_name: String,
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeUsageLimitResumeOptions<'a> {
    pub(crate) requested_profile: Option<&'a str>,
    pub(crate) no_auto_rotate: bool,
    pub(crate) skip_quota_check: bool,
    pub(crate) base_url: Option<&'a str>,
    pub(crate) include_code_review: bool,
    pub(crate) no_proxy: bool,
    pub(crate) attempted_profiles: &'a BTreeSet<String>,
}

pub(crate) fn next_runtime_usage_limit_plan(
    state: &AppState,
    session_id: &str,
    options: &RuntimeUsageLimitResumeOptions<'_>,
) -> Option<GoalResumeRelaunchPlan> {
    let failed_profile = state
        .session_profile_bindings
        .get(session_id)
        .map(|binding| binding.profile_name.clone())
        .or_else(|| options.requested_profile.map(ToOwned::to_owned))
        .or_else(|| state.active_profile.clone())
        .unwrap_or_default();
    let candidates = if options.skip_quota_check {
        super::active_profile_selection_order(state, &failed_profile)
    } else {
        super::find_ready_profiles(
            state,
            &failed_profile,
            options.base_url,
            options.include_code_review,
            options.no_proxy,
        )
    };
    candidates
        .into_iter()
        .filter(|candidate| candidate != &failed_profile)
        .filter(|candidate| !options.attempted_profiles.contains(candidate))
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

pub(crate) fn plan_runtime_usage_limit_relaunch(
    monitor: &mut GoalUsageLimitMonitor,
    status: &std::process::ExitStatus,
    options: &RuntimeUsageLimitResumeOptions<'_>,
) -> Result<Option<GoalResumeRelaunchPlan>> {
    if status.success() || options.no_auto_rotate {
        return Ok(None);
    }
    let Some(session_id) = monitor.detect_usage_limit_after_child()? else {
        return Ok(None);
    };
    let state = AppState::load_and_repair(&monitor.paths)?;
    Ok(next_runtime_usage_limit_plan(&state, &session_id, options))
}

pub(crate) struct GoalUsageLimitMonitor {
    pub(crate) paths: AppPaths,
    db_path: Option<PathBuf>,
    pub(crate) marker_path: PathBuf,
    session_id: Option<String>,
    connection: Option<rusqlite::Connection>,
    armed: bool,
    usage_limit_pending: bool,
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
            connection: None,
            armed: false,
            usage_limit_pending: false,
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
        let Some(db_path) = self.db_path.as_ref() else {
            return Ok(None);
        };
        let Some(session_id) = self.session_id.clone() else {
            return Ok(None);
        };
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
            .with_context(|| format!("failed to read goal status from {}", db_path.display()))?;
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
        let path = Path::new(&report.path);
        if !self.session_is_resumable(path, &session_id)? {
            return Ok(None);
        }
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
        let saw_usage_limit = session_file_has_usage_limit_since(path, scan_offset)?;
        self.session_scan_offset =
            prodex_session_store::session_file_logical_len(path).unwrap_or(file_len);
        if saw_usage_limit {
            self.session_usage_limit_reported = true;
            return Ok(Some(session_id));
        }
        Ok(None)
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
        Ok(status.is_some_and(|status| goal_status_is_resumable(&status)))
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
    let trimmed = line.trim();
    if trimmed.eq_ignore_ascii_case(OBSERVED_USAGE_LIMIT_MESSAGE) {
        return true;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    match object.get("type").and_then(serde_json::Value::as_str) {
        Some("event_msg") => object
            .get("payload")
            .is_some_and(goal_resume_event_payload_usage_limit),
        Some("error" | "response.failed") => goal_resume_structured_usage_limit(&value),
        _ if object.contains_key("error") => goal_resume_structured_usage_limit(&value),
        _ => false,
    }
}

fn goal_resume_event_payload_usage_limit(value: &serde_json::Value) -> bool {
    let serde_json::Value::Object(object) = value else {
        return false;
    };
    let explicit_type = object.get("type").and_then(serde_json::Value::as_str);
    if explicit_type.is_some_and(|kind| !kind.eq_ignore_ascii_case("error")) {
        return false;
    }
    if object
        .get("message")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|message| {
            message
                .trim()
                .eq_ignore_ascii_case(OBSERVED_USAGE_LIMIT_MESSAGE)
        })
    {
        return true;
    }
    let is_error = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("error"))
        || object.contains_key("error")
        || ["code", "status", "reason", "codex_error_info"]
            .into_iter()
            .any(|key| {
                object
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(goal_resume_quota_code)
            });
    is_error && goal_resume_structured_usage_limit(value)
}

fn goal_resume_structured_usage_limit(value: &serde_json::Value) -> bool {
    let mut stack = vec![value];
    let mut visited = 0;
    while let Some(value) = stack.pop() {
        visited += 1;
        if visited > GOAL_USAGE_LIMIT_JSON_SCAN_LIMIT {
            return false;
        }
        match value {
            serde_json::Value::Array(values) => stack.extend(values.iter().rev()),
            serde_json::Value::Object(object) => {
                if object.contains_key("role")
                    || matches!(
                        object.get("type").and_then(serde_json::Value::as_str),
                        Some(
                            "message"
                                | "user_message"
                                | "assistant_message"
                                | "agent_message"
                                | "response.output_text.delta"
                                | "response.output_text.done"
                        )
                    )
                {
                    continue;
                }

                let is_quota_code = |key: &str| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(goal_resume_quota_code)
                };
                if [
                    "code",
                    "type",
                    "status",
                    "reason",
                    "error",
                    "codex_error_info",
                ]
                .into_iter()
                .any(is_quota_code)
                    || ["message", "detail", "error"].into_iter().any(|key| {
                        object
                            .get(key)
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(goal_resume_usage_limit_text)
                    })
                {
                    return true;
                }
                stack.extend(
                    object
                        .iter()
                        .filter(|(key, _)| !matches!(key.as_str(), "content" | "text" | "delta"))
                        .map(|(_, value)| value),
                );
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }
    false
}

fn goal_resume_quota_code(code: &str) -> bool {
    runtime_proxy_crate::runtime_quota_payload_code(code)
        || code.trim().eq_ignore_ascii_case("usage_limit_exceeded")
}

fn goal_resume_usage_limit_text(message: &str) -> bool {
    let message = message.trim();
    if message.eq_ignore_ascii_case(OBSERVED_USAGE_LIMIT_MESSAGE) {
        return true;
    }
    let lower = message.to_ascii_lowercase();
    lower.starts_with("you've hit your usage limit")
        || lower.starts_with("you have hit your usage limit")
        || lower == "the usage limit has been reached"
        || lower == "usage limit has been reached"
        || lower.starts_with("your workspace is out of credits")
        || lower.starts_with("you hit your spend cap")
}

fn session_file_has_usage_limit_since(path: &Path, offset: u64) -> Result<bool> {
    prodex_session_store::session_file_has_line_since(path, offset, |line| {
        goal_resume_line_has_usage_limit(line)
    })
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
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "active" | "paused" | "blocked" | "usage_limited"
    )
}

#[cfg(test)]
#[path = "usage_limit_recovery/tests.rs"]
mod tests;
