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
mod tests {
    use super::*;
    use crate::app_state::AppStateIoExt;
    use crate::test_support::TestEnvVarGuard;
    use crate::{ProfileEntry, ProfileProvider, ResponseProfileBinding};
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SESSION_ID: &str = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";

    fn exit_status(code: i32) -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;
            std::process::ExitStatus::from_raw(code << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt as _;
            std::process::ExitStatus::from_raw(code as u32)
        }
    }

    fn fixture_root(label: &str) -> PathBuf {
        let root = crate::test_support::test_temp_root().join(format!(
            "prodex-usage-limit-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        secret_store::ensure_private_directory(&root).unwrap();
        root
    }

    fn configured_env(root: &Path) -> (TestEnvVarGuard, TestEnvVarGuard) {
        let home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let shared = root.join("shared-codex");
        let shared_guard =
            TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared.to_str().unwrap());
        (home, shared_guard)
    }

    fn populate_fixture(root: &Path, paths: &AppPaths, profile_names: &[&str]) -> PathBuf {
        let mut profiles = BTreeMap::new();
        let profiles_root = root.join("profiles");
        fs::create_dir_all(&profiles_root).unwrap();
        secret_store::ensure_private_directory(&profiles_root).unwrap();
        for name in profile_names {
            let home = profiles_root.join(name);
            fs::create_dir_all(&home).unwrap();
            secret_store::ensure_private_directory(&home).unwrap();
            secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
                .write_text(
                    &secret_store::SecretLocation::file(secret_store::auth_json_path(&home)),
                    r#"{"tokens":{"access_token":"synthetic-token"}}"#,
                )
                .unwrap();
            profiles.insert(
                (*name).to_string(),
                ProfileEntry {
                    codex_home: home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            );
        }
        let state = AppState {
            active_profile: profile_names.first().map(|name| (*name).to_string()),
            profiles,
            session_profile_bindings: BTreeMap::from([(
                SESSION_ID.to_string(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: profile_names[0].to_string(),
                    bound_at: chrono::Local::now().timestamp(),
                },
            )]),
            ..AppState::default()
        };
        state.save(paths).unwrap();

        let sessions = paths.shared_codex_root.join("sessions/2026/08/29");
        fs::create_dir_all(&sessions).unwrap();
        let session_path = sessions.join(format!("rollout-2026-08-29T01-00-00-{SESSION_ID}.jsonl"));
        fs::write(
            &session_path,
            format!(
                "{{\"timestamp\":\"2026-08-29T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{SESSION_ID}\",\"cwd\":\"/tmp/prodex-p0\",\"originator\":\"codex_exec\",\"cli_version\":\"0.151.0\",\"model_provider\":\"openai\"}}}}\n"
            ),
        )
        .unwrap();
        session_path
    }

    fn append_usage_limit(path: &Path) {
        let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
        writeln!(
            file,
            "{{\"timestamp\":\"2026-08-29T01:00:01Z\",\"type\":\"event_msg\",\"payload\":{{\"message\":{}}}}}",
            serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
        )
        .unwrap();
    }

    fn append_compressed_usage_limit(path: &Path) {
        let mut contents = zstd::stream::decode_all(fs::File::open(path).unwrap()).unwrap();
        contents.extend_from_slice(
            format!(
                "{{\"timestamp\":\"2026-08-29T01:00:01Z\",\"type\":\"event_msg\",\"payload\":{{\"message\":{}}}}}\n",
                serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
            )
            .as_bytes(),
        );
        fs::write(
            path,
            zstd::stream::encode_all(contents.as_slice(), 3).unwrap(),
        )
        .unwrap();
    }

    fn resume_options<'a>(
        attempted_profiles: &'a BTreeSet<String>,
        no_auto_rotate: bool,
        requested_profile: Option<&'a str>,
    ) -> RuntimeUsageLimitResumeOptions<'a> {
        RuntimeUsageLimitResumeOptions {
            requested_profile,
            no_auto_rotate,
            skip_quota_check: true,
            base_url: None,
            include_code_review: false,
            no_proxy: false,
            attempted_profiles,
        }
    }

    #[test]
    fn usage_limit_detection_accepts_error_envelopes_not_conversation_prose() {
        assert!(goal_resume_line_has_usage_limit(
            OBSERVED_USAGE_LIMIT_MESSAGE
        ));
        assert!(goal_resume_line_has_usage_limit(&format!(
            r#"{{"type":"event_msg","payload":{{"message":{}}}}}"#,
            serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
        )));

        for value in [
            serde_json::json!({
                "type": "error",
                "payload": { "message": "You've hit your usage limit. Try again later." }
            }),
            serde_json::json!({
                "type": "response.failed",
                "response": { "error": { "code": "usage_limit_reached" } }
            }),
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "error",
                    "error": { "type": "usage_not_included" }
                }
            }),
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "error",
                    "message": "Quota unavailable",
                    "codex_error_info": "usage_limit_exceeded"
                }
            }),
            serde_json::json!({ "error": { "code": "insufficient_quota" } }),
        ] {
            assert!(goal_resume_line_has_usage_limit(
                &serde_json::to_string(&value).unwrap()
            ));
        }

        for value in [
            serde_json::json!({
                "type": "message",
                "payload": {
                    "role": "user",
                    "content": OBSERVED_USAGE_LIMIT_MESSAGE
                }
            }),
            serde_json::json!({
                "type": "message",
                "payload": {
                    "role": "assistant",
                    "content": "The docs mention usage_limit_reached as an example."
                }
            }),
            serde_json::json!({
                "type": "event_msg",
                "payload": { "message": "The docs mention usage_limit_reached as an example." }
            }),
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "warning",
                    "message": OBSERVED_USAGE_LIMIT_MESSAGE
                }
            }),
            serde_json::json!({
                "type": "error",
                "payload": {
                    "type": "error",
                    "message": "The docs mention usage_limit_reached as an example."
                }
            }),
        ] {
            assert!(!goal_resume_line_has_usage_limit(
                &serde_json::to_string(&value).unwrap()
            ));
        }
    }

    #[test]
    fn post_exit_recovery_requires_a_matching_resumable_goal_when_goal_store_exists() {
        for (label, thread_id, status, expected_profile) in [
            ("paused", SESSION_ID, "paused", Some("b")),
            ("complete", SESSION_ID, "complete", None),
            ("unrelated", "other-thread", "paused", None),
        ] {
            let root = fixture_root(label);
            let (_home, _shared) = configured_env(&root);
            let paths = AppPaths::discover().unwrap();
            let session_path = populate_fixture(&root, &paths, &["a", "b"]);
            if label == "unrelated" {
                let mut session = fs::OpenOptions::new()
                    .append(true)
                    .open(&session_path)
                    .unwrap();
                writeln!(
                    session,
                    "{{\"type\":\"thread\",\"payload\":{{\"thread_id\":\"actual-thread\"}}}}"
                )
                .unwrap();
            }
            let connection =
                rusqlite::Connection::open(paths.shared_codex_root.join("goals_1.sqlite")).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE thread_goals (
                        thread_id TEXT PRIMARY KEY,
                        goal_id TEXT NOT NULL,
                        objective TEXT NOT NULL,
                        status TEXT NOT NULL,
                        token_budget INTEGER,
                        tokens_used INTEGER NOT NULL DEFAULT 0,
                        time_used_seconds INTEGER NOT NULL DEFAULT 0,
                        created_at_ms INTEGER NOT NULL,
                        updated_at_ms INTEGER NOT NULL
                    );",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', 'finish work', ?2, 1, 1)",
                    rusqlite::params![thread_id, status],
                )
                .unwrap();
            drop(connection);

            let mut monitor = prepare_goal_usage_limit_monitor(
                &[OsString::from("exec"), OsString::from("work")],
                false,
            )
            .unwrap()
            .unwrap();
            super::super::goal_resume::write_runtime_goal_session_marker(
                &monitor.marker_path,
                std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
            )
            .unwrap();
            append_usage_limit(&session_path);
            let attempted = BTreeSet::new();
            let options = resume_options(&attempted, false, Some("a"));
            assert_eq!(
                plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
                    .unwrap()
                    .as_ref()
                    .map(|plan| plan.profile_name.as_str()),
                expected_profile,
                "{label}"
            );
            drop(monitor);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn exact_post_child_usage_limit_rotates_and_old_bytes_are_not_replayed() {
        let root = fixture_root("exact");
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session_path = populate_fixture(&root, &paths, &["a", "b"]);
        let mut monitor = prepare_runtime_usage_limit_monitor(
            &[OsString::from("exec"), OsString::from("work")],
            false,
        )
        .unwrap()
        .unwrap();
        super::super::goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
        )
        .unwrap();
        append_usage_limit(&session_path);
        let attempted = BTreeSet::new();
        let options = resume_options(&attempted, false, Some("a"));

        let plan = plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
            .unwrap()
            .unwrap();
        assert_eq!(plan.profile_name, "b");

        monitor.prepare_for_resume();
        assert!(monitor.detect_usage_limit_after_child().unwrap().is_none());
        append_usage_limit(&session_path);
        assert_eq!(
            monitor.detect_usage_limit_after_child().unwrap().as_deref(),
            Some(SESSION_ID)
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compressed_post_child_usage_limit_uses_decoded_marker_offset() {
        let root = fixture_root("compressed");
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session_path = populate_fixture(&root, &paths, &["a", "b"]);
        let compressed_path = session_path.with_extension("jsonl.zst");
        let initial = fs::read(&session_path).unwrap();
        fs::remove_file(&session_path).unwrap();
        fs::write(
            &compressed_path,
            zstd::stream::encode_all(initial.as_slice(), 3).unwrap(),
        )
        .unwrap();

        let mut monitor = prepare_runtime_usage_limit_monitor(
            &[OsString::from("exec"), OsString::from("work")],
            false,
        )
        .unwrap()
        .unwrap();
        super::super::goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
        )
        .unwrap();
        append_compressed_usage_limit(&compressed_path);

        assert_eq!(
            monitor.detect_usage_limit_after_child().unwrap().as_deref(),
            Some(SESSION_ID)
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_preserves_compacted_rollout_and_completed_tool_side_effect_once() {
        let root = fixture_root("compaction-side-effect");
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session_path = populate_fixture(&root, &paths, &["a", "b"]);
        let mut monitor = prepare_runtime_usage_limit_monitor(
            &[OsString::from("exec"), OsString::from("work")],
            false,
        )
        .unwrap()
        .unwrap();
        super::super::goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
        )
        .unwrap();

        let completed_progress = concat!(
            "{\"type\":\"compacted\",\"payload\":{\"window_id\":\"window-2\"}}\n",
            "{\"type\":\"tool_completed\",\"payload\":{\"call_id\":\"side-effect-1\"}}\n"
        );
        let mut session = fs::OpenOptions::new()
            .append(true)
            .open(&session_path)
            .unwrap();
        session.write_all(completed_progress.as_bytes()).unwrap();
        append_usage_limit(&session_path);
        let before_resume = fs::read(&session_path).unwrap();
        let attempted = BTreeSet::new();
        let options = resume_options(&attempted, false, Some("a"));

        let plan = plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
            .unwrap()
            .unwrap();
        assert_eq!(plan.profile_name, "b");

        let after_plan = fs::read(&session_path).unwrap();
        assert_eq!(after_plan, before_resume);
        assert_eq!(
            String::from_utf8_lossy(&after_plan)
                .matches("side-effect-1")
                .count(),
            1
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn usage_limit_recovery_reaches_a_late_ready_profile() {
        let root = fixture_root("late");
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session_path = populate_fixture(&root, &paths, &["a", "b", "c", "d"]);
        let mut monitor = prepare_runtime_usage_limit_monitor(
            &[OsString::from("exec"), OsString::from("work")],
            false,
        )
        .unwrap()
        .unwrap();
        super::super::goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
        )
        .unwrap();
        append_usage_limit(&session_path);
        let attempted = BTreeSet::from(["b".to_string(), "c".to_string()]);
        let options = resume_options(&attempted, false, Some("a"));
        assert_eq!(
            plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
                .unwrap()
                .unwrap()
                .profile_name,
            "d"
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn no_auto_rotate_keeps_usage_limit_terminal() {
        let root = fixture_root("disabled");
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let _session_path = populate_fixture(&root, &paths, &["a", "b"]);
        let mut monitor = prepare_runtime_usage_limit_monitor(
            &[OsString::from("exec"), OsString::from("work")],
            false,
        )
        .unwrap()
        .unwrap();
        let attempted = BTreeSet::new();
        let options = resume_options(&attempted, true, Some("a"));
        assert!(
            plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
                .unwrap()
                .is_none()
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }
}
