use anyhow::bail;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const PRODEX_CODEX_FULL_ACCESS_ARG: &str = "--full-access";
const PRODEX_DRY_RUN_ARG: &str = "--dry-run";
const CODEX_BYPASS_APPROVALS_AND_SANDBOX_ARG: &str = "--dangerously-bypass-approvals-and-sandbox";
const LOCAL_PROXY_BYPASS_ENV_KEYS: [&str; 2] = ["NO_PROXY", "no_proxy"];
const LOCAL_PROXY_BYPASS_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "::1"];
const UPSTREAM_PROXY_ENV_KEYS: [&str; 8] = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "PROXY",
    "proxy",
];

pub struct RuntimeLaunchRequest<'a> {
    pub profile: Option<&'a str>,
    pub allow_auto_rotate: bool,
    pub skip_quota_check: bool,
    pub base_url: Option<&'a str>,
    pub upstream_no_proxy: bool,
    pub include_code_review: bool,
    pub force_runtime_proxy: bool,
    pub model_provider_override: Option<&'a str>,
}

pub struct PreparedRuntimeLaunch<P, E> {
    pub paths: P,
    pub codex_home: PathBuf,
    pub managed: bool,
    pub runtime_proxy: Option<E>,
}

pub fn allow_profileless_local_home(
    requested_profile: Option<&str>,
    model_provider_override: Option<&str>,
    local_provider_id: &str,
) -> bool {
    requested_profile.is_none()
        && model_provider_override
            .is_some_and(|provider| provider.eq_ignore_ascii_case(local_provider_id))
}

pub fn fixed_runtime_proxy_state(
    state: &prodex_state::AppState,
    profile_name: &str,
) -> anyhow::Result<prodex_state::AppState> {
    let mut fixed = state.clone();
    if !fixed.profiles.contains_key(profile_name) {
        bail!("profile '{}' is missing", profile_name);
    }
    fixed
        .profiles
        .retain(|candidate_name, _| candidate_name == profile_name);
    fixed.active_profile = Some(profile_name.to_string());
    fixed
        .last_run_selected_at
        .retain(|candidate_name, _| candidate_name == profile_name);
    fixed
        .response_profile_bindings
        .retain(|_, binding| binding.profile_name == profile_name);
    fixed
        .session_profile_bindings
        .retain(|_, binding| binding.profile_name == profile_name);
    Ok(fixed)
}

pub fn record_run_selection_at(
    state: &mut prodex_state::AppState,
    profile_name: &str,
    selected_at: i64,
) {
    state
        .last_run_selected_at
        .retain(|name, _| state.profiles.contains_key(name));
    state
        .last_run_selected_at
        .insert(profile_name.to_string(), selected_at);
}

pub fn resolve_profile_name(
    state: &prodex_state::AppState,
    requested: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(name) = requested {
        if state.profiles.contains_key(name) {
            return Ok(name.to_string());
        }
        bail!("profile '{}' does not exist", name);
    }

    if let Some(active) = state.active_profile.as_deref() {
        if state.profiles.contains_key(active) {
            return Ok(active.to_string());
        }
        bail!("active profile '{}' no longer exists", active);
    }

    if state.profiles.len() == 1 {
        let (name, _) = state
            .profiles
            .iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("single profile lookup failed unexpectedly"))?;
        return Ok(name.clone());
    }

    bail!("no active profile selected; use `prodex use --profile <name>` or pass --profile")
}

pub fn ensure_profile_path_is_unique(
    state: &prodex_state::AppState,
    candidate: &Path,
) -> anyhow::Result<()> {
    for (name, profile) in &state.profiles {
        if prodex_core::same_path(&profile.codex_home, candidate) {
            bail!(
                "path {} is already used by profile '{}'",
                candidate.display(),
                name
            );
        }
    }
    Ok(())
}

pub fn dry_run_caveman_home_placeholder(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> PathBuf {
    managed_profiles_root.join(format!(
        ".caveman-dry-run-from-{}",
        base_codex_home
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ))
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProxyCodexEndpoint<'a> {
    pub listen_addr: SocketAddr,
    pub openai_mount_path: &'a str,
}

#[derive(Debug, Clone)]
pub struct ChildProcessPlan {
    pub binary: OsString,
    pub args: Vec<OsString>,
    pub codex_home: PathBuf,
    pub extra_env: Vec<(OsString, OsString)>,
    pub removed_env: Vec<OsString>,
}

impl ChildProcessPlan {
    pub fn new(binary: OsString, codex_home: PathBuf) -> Self {
        Self {
            binary,
            args: Vec::new(),
            codex_home,
            extra_env: Vec::new(),
            removed_env: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<OsString>) -> Self {
        self.args = args;
        self
    }

    pub fn with_extra_env<I, K>(mut self, extra_env: I) -> Self
    where
        I: IntoIterator<Item = (K, OsString)>,
        K: Into<OsString>,
    {
        self.extra_env = extra_env
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect();
        self
    }

    pub fn with_removed_env<I, K>(mut self, removed_env: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<OsString>,
    {
        self.removed_env = removed_env.into_iter().map(Into::into).collect();
        self
    }
}

pub fn codex_child_plan(
    binary: OsString,
    codex_home: PathBuf,
    args: Vec<OsString>,
    local_provider_id: &str,
) -> ChildProcessPlan {
    let local_provider_hosts = local_proxy_bypass_hosts_from_args(&args, local_provider_id);
    ChildProcessPlan::new(binary, codex_home)
        .with_args(args)
        .with_extra_env(local_proxy_bypass_env_for_hosts(&local_provider_hosts))
        .with_removed_env(codex_sandbox_removed_env())
}

pub fn local_proxy_bypass_env() -> Vec<(&'static str, OsString)> {
    local_proxy_bypass_env_for_hosts(std::iter::empty::<&str>())
}

pub fn local_proxy_bypass_env_for_hosts<I, S>(extra_hosts: I) -> Vec<(&'static str, OsString)>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut parts = Vec::<String>::new();
    for key in LOCAL_PROXY_BYPASS_ENV_KEYS {
        if let Some(value) = env::var_os(key) {
            push_proxy_bypass_parts(&mut parts, &value.to_string_lossy());
        }
    }
    for host in LOCAL_PROXY_BYPASS_HOSTS {
        push_proxy_bypass_part(&mut parts, host);
    }
    for host in extra_hosts {
        push_proxy_bypass_part(&mut parts, host.as_ref());
    }
    let merged = OsString::from(parts.join(","));
    LOCAL_PROXY_BYPASS_ENV_KEYS
        .into_iter()
        .map(|key| (key, merged.clone()))
        .collect()
}

fn local_proxy_bypass_hosts_from_args(args: &[OsString], local_provider_id: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    let local_provider_key = format!("model_providers.{local_provider_id}.base_url");
    for key in [
        "chatgpt_base_url",
        "openai_base_url",
        local_provider_key.as_str(),
    ] {
        if let Some(base_url) = codex_config::codex_cli_config_override_value(args, key) {
            push_proxy_bypass_url_hosts(&mut hosts, &base_url);
        }
    }
    hosts
}

fn push_proxy_bypass_url_hosts(parts: &mut Vec<String>, base_url: &str) {
    let Ok(parsed) = reqwest::Url::parse(base_url) else {
        return;
    };
    let Some(host) = parsed.host_str() else {
        return;
    };
    push_proxy_bypass_part(parts, host);
    if let Some(port) = parsed.port() {
        let host_port = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        push_proxy_bypass_part(parts, &host_port);
    }
}

fn push_proxy_bypass_parts(parts: &mut Vec<String>, value: &str) {
    for part in value.split(',') {
        push_proxy_bypass_part(parts, part);
    }
}

fn push_proxy_bypass_part(parts: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if !parts
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(trimmed))
    {
        parts.push(trimmed.to_string());
    }
}

pub fn codex_sandbox_removed_env() -> Vec<OsString> {
    let mut removed = BTreeSet::from([
        OsString::from("CODEX_SANDBOX"),
        OsString::from("CODEX_SANDBOX_NETWORK_DISABLED"),
    ]);
    removed.extend(env::vars_os().filter_map(|(key, _)| {
        key.to_str()
            .is_some_and(|value| value.starts_with("CODEX_SANDBOX"))
            .then_some(key)
    }));
    removed.into_iter().collect()
}

pub fn upstream_proxy_removed_env() -> Vec<OsString> {
    UPSTREAM_PROXY_ENV_KEYS
        .into_iter()
        .map(OsString::from)
        .collect()
}

pub fn remove_upstream_proxy_env(plan: &mut ChildProcessPlan) {
    let mut removed = BTreeSet::<OsString>::from_iter(plan.removed_env.iter().cloned());
    removed.extend(upstream_proxy_removed_env());
    plan.removed_env = removed.into_iter().collect();
}

pub fn runtime_proxy_codex_passthrough_args(
    runtime_proxy: Option<RuntimeProxyCodexEndpoint<'_>>,
    user_args: &[OsString],
) -> Vec<OsString> {
    runtime_proxy
        .map(|proxy| {
            if proxy.openai_mount_path == runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH {
                runtime_proxy_codex_args(proxy.listen_addr, user_args)
            } else {
                runtime_proxy_codex_args_with_mount_path(
                    proxy.listen_addr,
                    proxy.openai_mount_path,
                    user_args,
                )
            }
        })
        .unwrap_or_else(|| user_args.to_vec())
}

pub fn normalize_run_codex_args(codex_args: &[OsString]) -> Vec<OsString> {
    let Some(first) = codex_args.first().and_then(|arg| arg.to_str()) else {
        return codex_args.to_vec();
    };
    if !looks_like_codex_session_id(first) {
        return codex_args.to_vec();
    }

    let mut normalized = Vec::with_capacity(codex_args.len() + 1);
    normalized.push(OsString::from("resume"));
    normalized.extend(codex_args.iter().cloned());
    normalized
}

fn looks_like_codex_session_id(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 5 {
        return false;
    }
    let expected_lengths = [8usize, 4, 4, 4, 12];
    parts.iter().zip(expected_lengths).all(|(part, expected)| {
        part.len() == expected && part.chars().all(|ch| ch.is_ascii_hexdigit())
    })
}

pub fn runtime_proxy_codex_args(
    listen_addr: std::net::SocketAddr,
    user_args: &[OsString],
) -> Vec<OsString> {
    runtime_proxy_codex_args_with_mount_path(
        listen_addr,
        runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH,
        user_args,
    )
}

pub fn runtime_proxy_codex_args_with_mount_path(
    listen_addr: std::net::SocketAddr,
    openai_mount_path: &str,
    user_args: &[OsString],
) -> Vec<OsString> {
    let proxy_chatgpt_base = format!("http://{listen_addr}/backend-api");
    let proxy_openai_base = format!("http://{listen_addr}{openai_mount_path}");
    let overrides = [
        format!(
            "chatgpt_base_url={}",
            toml_string_literal(&proxy_chatgpt_base)
        ),
        format!(
            "openai_base_url={}",
            toml_string_literal(&proxy_openai_base),
        ),
    ];

    let mut args = Vec::with_capacity((overrides.len() * 2) + user_args.len());
    for override_entry in overrides {
        args.push(OsString::from("-c"));
        args.push(OsString::from(override_entry));
    }
    args.extend(user_args.iter().cloned());
    args
}

pub fn prepare_codex_launch_args(
    codex_args: &[OsString],
    full_access_requested: bool,
) -> (Vec<OsString>, bool) {
    let (passthrough_full_access, codex_args) = extract_prodex_full_access_flag(codex_args);
    let codex_args = normalize_run_codex_args(&codex_args);
    let include_code_review = is_review_invocation(&codex_args);
    let codex_args = codex_launch_args_with_full_access(
        &codex_args,
        full_access_requested || passthrough_full_access,
    );
    (codex_args, include_code_review)
}

pub fn extract_prodex_dry_run_flag(codex_args: &[OsString]) -> (bool, Vec<OsString>) {
    let mut dry_run = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    for arg in codex_args {
        if arg == PRODEX_DRY_RUN_ARG {
            dry_run = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (dry_run, filtered)
}

pub fn prodex_dry_run_requested(codex_args: &[OsString]) -> bool {
    codex_args.iter().any(|arg| arg == PRODEX_DRY_RUN_ARG)
}

pub fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}

fn extract_prodex_full_access_flag(codex_args: &[OsString]) -> (bool, Vec<OsString>) {
    let mut full_access = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    for arg in codex_args {
        if arg == PRODEX_CODEX_FULL_ACCESS_ARG {
            full_access = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (full_access, filtered)
}

fn codex_launch_args_with_full_access(codex_args: &[OsString], full_access: bool) -> Vec<OsString> {
    if !full_access {
        return codex_args.to_vec();
    }

    let mut args = Vec::with_capacity(codex_args.len() + 1);
    args.push(OsString::from(CODEX_BYPASS_APPROVALS_AND_SANDBOX_ARG));
    args.extend(codex_args.iter().cloned());
    args
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Debug, Clone)]
pub struct RuntimeLaunchPlan {
    pub child: ChildProcessPlan,
    pub cleanup_paths: Vec<PathBuf>,
}

impl RuntimeLaunchPlan {
    pub fn new(child: ChildProcessPlan) -> Self {
        Self {
            child,
            cleanup_paths: Vec::new(),
        }
    }

    pub fn with_cleanup_path(mut self, path: PathBuf) -> Self {
        self.cleanup_paths.push(path);
        self
    }
}

pub fn cleanup_runtime_launch_plan(plan: &RuntimeLaunchPlan) {
    for path in &plan.cleanup_paths {
        let _ = fs::remove_dir_all(path);
    }
}

#[derive(Debug, Clone)]
pub enum RuntimeLaunchDryRunChild {
    Codex { codex_args: Vec<OsString> },
    Caveman { codex_args: Vec<OsString> },
}

pub fn runtime_launch_dry_run_plan(
    binary: OsString,
    base_codex_home: &Path,
    managed_profiles_root: &Path,
    runtime_proxy: Option<RuntimeProxyCodexEndpoint<'_>>,
    upstream_no_proxy: bool,
    local_provider_id: &str,
    child: RuntimeLaunchDryRunChild,
) -> RuntimeLaunchPlan {
    let runtime_args = match &child {
        RuntimeLaunchDryRunChild::Codex { codex_args }
        | RuntimeLaunchDryRunChild::Caveman { codex_args } => {
            runtime_proxy_codex_passthrough_args(runtime_proxy, codex_args)
        }
    };
    let (codex_home, cleanup_path) = match child {
        RuntimeLaunchDryRunChild::Codex { .. } => (base_codex_home.to_path_buf(), None),
        RuntimeLaunchDryRunChild::Caveman { .. } => {
            let caveman_home =
                dry_run_caveman_home_placeholder(managed_profiles_root, base_codex_home);
            (caveman_home.clone(), Some(caveman_home))
        }
    };
    let mut child = codex_child_plan(binary, codex_home, runtime_args, local_provider_id);
    if upstream_no_proxy && runtime_proxy.is_none() {
        remove_upstream_proxy_env(&mut child);
    }

    let plan = RuntimeLaunchPlan::new(child);
    if let Some(cleanup_path) = cleanup_path {
        plan.with_cleanup_path(cleanup_path)
    } else {
        plan
    }
}

pub fn runtime_launch_dry_run_report(
    flow: &str,
    base_codex_home: &Path,
    runtime_proxy: Option<RuntimeProxyCodexEndpoint<'_>>,
    plan: &RuntimeLaunchPlan,
) -> String {
    let child = &plan.child;
    let provider = dry_run_config_value(&child.args, base_codex_home, "model_provider")
        .unwrap_or_else(|| "openai".to_string());
    let model = dry_run_config_value(&child.args, base_codex_home, "model")
        .unwrap_or_else(|| "(codex default)".to_string());
    let mut output = String::new();
    output.push_str("Prodex dry run: launch diagnostics\n");
    output.push_str(&format!("Flow: {flow}\n"));
    output.push_str(&format!(
        "Binary: {}\n",
        redaction::redaction_display_os(&child.binary)
    ));
    output.push_str(&format!("Provider: {provider}\n"));
    output.push_str(&format!("Model: {model}\n"));
    output.push_str(&format!("CODEX_HOME: {}\n", child.codex_home.display()));
    output.push_str(&format!(
        "Runtime proxy: {}\n",
        runtime_proxy
            .map(|proxy| {
                if proxy.listen_addr.port() == 0 {
                    format!("would be enabled with mount {}", proxy.openai_mount_path)
                } else {
                    format!(
                        "enabled at http://{}{}",
                        proxy.listen_addr, proxy.openai_mount_path
                    )
                }
            })
            .unwrap_or_else(|| "disabled".to_string())
    ));
    output.push_str("Args:\n");
    if child.args.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for arg in redaction::redaction_redacted_cli_args(&child.args) {
            output.push_str(&format!("  {arg}\n"));
        }
    }
    output.push_str("Env:\n");
    output.push_str(&format!("  CODEX_HOME={}\n", child.codex_home.display()));
    for (key, value) in &child.extra_env {
        output.push_str(&format!(
            "  {}={}\n",
            redaction::redaction_display_os(key),
            redaction::redaction_redacted_env_value(key, value)
        ));
    }
    if !child.removed_env.is_empty() {
        output.push_str("Removed env:\n");
        for key in &child.removed_env {
            output.push_str(&format!("  {}\n", redaction::redaction_display_os(key)));
        }
    }
    output.push_str("Codex/TUI not started because --dry-run was set.\n");
    output
}

fn dry_run_config_value(args: &[OsString], codex_home: &Path, key: &str) -> Option<String> {
    codex_config::codex_cli_config_override_value(args, key)
        .or_else(|| codex_config::codex_config_value(codex_home, key))
}

pub fn scored_runtime_candidate_message(
    initial_profile_name: &str,
    best_candidate: &prodex_shared_types::ReadyProfileCandidate,
    selected_report: Option<&prodex_shared_types::RunProfileProbeReport>,
    include_code_review: bool,
) -> terminal_ui::RuntimeLaunchScoredCandidateOutput {
    let quota_summary = prodex_quota::format_main_windows_compact(&best_candidate.usage);
    let blocked_summary = selected_report.and_then(|report| {
        report.result.as_ref().ok().and_then(|usage| {
            let blocked = prodex_quota::collect_blocked_limits(usage, include_code_review);
            (!blocked.is_empty()).then(|| prodex_quota::format_blocked_limits(&blocked))
        })
    });
    let selected_profile_status = selected_report.map(|report| match &report.result {
        Ok(_) => blocked_summary
            .as_deref()
            .map(
                |blocked_summary| terminal_ui::RuntimeLaunchSelectedProfileStatus::Blocked {
                    blocked_summary,
                },
            )
            .unwrap_or(terminal_ui::RuntimeLaunchSelectedProfileStatus::Ready),
        Err(err) => terminal_ui::RuntimeLaunchSelectedProfileStatus::ProbeFailed { error: err },
    });

    terminal_ui::format_runtime_launch_scored_candidate_message(
        terminal_ui::RuntimeLaunchScoredCandidateMessage {
            initial_profile_name,
            candidate: terminal_ui::RuntimeLaunchCandidateDisplay {
                name: &best_candidate.name,
                quota_summary: &quota_summary,
            },
            selected_profile_status,
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchNoReadyProfilesPlan {
    Blocked {
        blocked_message: String,
        no_ready_message: String,
        inspect_hint: String,
        error_message: String,
    },
    ProbeFailed {
        warning_message: String,
        continue_message: String,
    },
}

pub fn no_ready_runtime_profiles_plan(
    report: &prodex_shared_types::RunProfileProbeReport,
    profile_name: &str,
    include_code_review: bool,
) -> RuntimeLaunchNoReadyProfilesPlan {
    match &report.result {
        Ok(usage) => {
            let blocked = prodex_quota::collect_blocked_limits(usage, include_code_review);
            RuntimeLaunchNoReadyProfilesPlan::Blocked {
                blocked_message: format!(
                    "Quota preflight blocked profile '{}': {}",
                    profile_name,
                    prodex_quota::format_blocked_limits(&blocked)
                ),
                no_ready_message: "No ready profile was found.".to_string(),
                inspect_hint: terminal_ui::format_runtime_launch_quota_inspect_hint(profile_name),
                error_message: format!(
                    "quota preflight blocked profile '{}' and no ready profile was found",
                    profile_name
                ),
            }
        }
        Err(err) => RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message: format!(
                "Warning: quota preflight failed for '{}': {err:#}",
                profile_name
            ),
            continue_message: "Continuing without quota gate.".to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchBlockedSelectedProfilePlan {
    Rotate {
        blocked_message: String,
        next_profile: String,
        rotate_message: String,
    },
    Stop {
        blocked_message: String,
        messages: Vec<String>,
        error_message: String,
    },
}

pub fn blocked_selected_runtime_profile_plan(
    profile_name: &str,
    blocked: &[prodex_quota::BlockedLimit],
    allow_auto_rotate: bool,
    alternatives: &[String],
) -> RuntimeLaunchBlockedSelectedProfilePlan {
    let blocked_message = format!(
        "Quota preflight blocked profile '{}': {}",
        profile_name,
        prodex_quota::format_blocked_limits(blocked)
    );

    if allow_auto_rotate {
        if let Some(next_profile) = alternatives.first() {
            let next_profile = next_profile.clone();
            return RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
                blocked_message,
                rotate_message: format!("Auto-rotating to profile '{}'.", next_profile),
                next_profile,
            };
        }

        return RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message,
            messages: vec![
                "No other ready profile was found.".to_string(),
                terminal_ui::format_runtime_launch_quota_inspect_hint(profile_name),
            ],
            error_message: format!(
                "quota preflight blocked profile '{}' and no other ready profile was found",
                profile_name
            ),
        };
    }

    let mut messages = Vec::new();
    if !alternatives.is_empty() {
        messages.push(format!(
            "Other profiles that look ready: {}",
            alternatives.join(", ")
        ));
        messages.push("Rerun without `--no-auto-rotate` to allow fallback.".to_string());
    }
    messages.push(terminal_ui::format_runtime_launch_quota_inspect_hint(
        profile_name,
    ));

    RuntimeLaunchBlockedSelectedProfilePlan::Stop {
        blocked_message,
        messages,
        error_message: format!(
            "quota preflight blocked profile '{}' with auto-rotate disabled",
            profile_name
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    thread_local! {
        static TEST_ENV_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    struct TestEnvLockGuard {
        _guard: Option<std::sync::MutexGuard<'static, ()>>,
    }

    fn acquire_test_env_lock() -> TestEnvLockGuard {
        let guard = TEST_ENV_LOCK_DEPTH.with(|depth| {
            let current = depth.get();
            depth.set(current + 1);
            if current == 0 {
                Some(
                    TEST_ENV_LOCK
                        .get_or_init(|| Mutex::new(()))
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                )
            } else {
                None
            }
        });

        TestEnvLockGuard { _guard: guard }
    }

    struct TestEnvVarGuard {
        _lock: Option<TestEnvLockGuard>,
        key: Option<&'static str>,
        previous: Option<OsString>,
    }

    impl TestEnvVarGuard {
        fn lock() -> Self {
            Self {
                _lock: Some(acquire_test_env_lock()),
                key: None,
                previous: None,
            }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let lock = acquire_test_env_lock();
            let previous = env::var_os(key);
            unsafe { env::set_var(key, value) };
            Self {
                _lock: Some(lock),
                key: Some(key),
                previous,
            }
        }

        fn unset(key: &'static str) -> Self {
            let lock = acquire_test_env_lock();
            let previous = env::var_os(key);
            unsafe { env::remove_var(key) };
            Self {
                _lock: Some(lock),
                key: Some(key),
                previous,
            }
        }
    }

    impl Drop for TestEnvVarGuard {
        fn drop(&mut self) {
            if let Some(key) = self.key {
                if let Some(value) = self.previous.as_ref() {
                    unsafe { env::set_var(key, value) };
                } else {
                    unsafe { env::remove_var(key) };
                }
            }
        }
    }

    fn test_profile(path: &str) -> prodex_state::ProfileEntry {
        prodex_state::ProfileEntry {
            codex_home: PathBuf::from(path),
            managed: true,
            email: None,
            provider: prodex_state::ProfileProvider::Openai,
        }
    }

    #[test]
    fn codex_sandbox_removed_env_strips_inherited_codex_sandbox_vars() {
        let _env_guard = TestEnvVarGuard::lock();
        let _sandbox_guard = TestEnvVarGuard::set("CODEX_SANDBOX", "workspace-write");
        let _network_guard = TestEnvVarGuard::set("CODEX_SANDBOX_NETWORK_DISABLED", "1");
        let _custom_guard = TestEnvVarGuard::set("CODEX_SANDBOX_PROFILE", "danger-full-access");
        let _other_guard = TestEnvVarGuard::set("PRODEX_TEST_KEEP_ENV", "1");

        let removed = codex_sandbox_removed_env();

        assert!(removed.iter().any(|key| key == "CODEX_SANDBOX"));
        assert!(
            removed
                .iter()
                .any(|key| key == "CODEX_SANDBOX_NETWORK_DISABLED")
        );
        assert!(removed.iter().any(|key| key == "CODEX_SANDBOX_PROFILE"));
        assert!(!removed.iter().any(|key| key == "PRODEX_TEST_KEEP_ENV"));
    }

    #[test]
    fn codex_child_plan_applies_codex_sandbox_removed_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _custom_guard = TestEnvVarGuard::set("CODEX_SANDBOX_PROFILE", "danger-full-access");
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
        let _lower_no_proxy_guard = TestEnvVarGuard::set("no_proxy", "internal.local");
        let binary = OsString::from("codex");
        let codex_home = PathBuf::from("/tmp/prodex-codex-home");
        let args = vec![OsString::from("login")];

        let plan = codex_child_plan(
            binary.clone(),
            codex_home.clone(),
            args.clone(),
            "prodex-local",
        );

        assert_eq!(plan.binary, binary);
        assert_eq!(plan.codex_home, codex_home);
        assert_eq!(plan.args, args);
        assert!(plan.removed_env.iter().any(|key| key == "CODEX_SANDBOX"));
        assert!(
            plan.removed_env
                .iter()
                .any(|key| key == "CODEX_SANDBOX_NETWORK_DISABLED")
        );
        assert!(
            plan.removed_env
                .iter()
                .any(|key| key == "CODEX_SANDBOX_PROFILE")
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("example.com,internal.local,127.0.0.1,localhost,::1".to_string())
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "no_proxy")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("example.com,internal.local,127.0.0.1,localhost,::1".to_string())
        );
    }

    #[test]
    fn local_proxy_bypass_env_deduplicates_existing_values() {
        let _env_guard = TestEnvVarGuard::lock();
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", " example.com,127.0.0.1 ");
        let _lower_no_proxy_guard = TestEnvVarGuard::set("no_proxy", "LOCALHOST,internal.local");

        let env = local_proxy_bypass_env();

        assert_eq!(
            env,
            vec![
                (
                    "NO_PROXY",
                    OsString::from("example.com,127.0.0.1,LOCALHOST,internal.local,::1")
                ),
                (
                    "no_proxy",
                    OsString::from("example.com,127.0.0.1,LOCALHOST,internal.local,::1")
                )
            ]
        );
    }

    #[test]
    fn codex_child_plan_adds_local_provider_host_to_proxy_bypass_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
        let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
        let args = vec![
            OsString::from("-c"),
            OsString::from(
                "model_providers.prodex-local.base_url=\"http://host.docker.internal:11434/v1\"",
            ),
        ];

        let plan = codex_child_plan(
            OsString::from("codex"),
            PathBuf::from("/tmp/prodex-codex-home"),
            args,
            "prodex-local",
        );

        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some(
                "example.com,127.0.0.1,localhost,::1,host.docker.internal,host.docker.internal:11434"
                    .to_string()
            )
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "no_proxy")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some(
                "example.com,127.0.0.1,localhost,::1,host.docker.internal,host.docker.internal:11434"
                    .to_string()
            )
        );
    }

    #[test]
    fn codex_child_plan_adds_runtime_proxy_ports_to_proxy_bypass_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _http_proxy_guard = TestEnvVarGuard::set("http_proxy", "http://127.0.0.1:1086");
        let _https_proxy_guard = TestEnvVarGuard::set("https_proxy", "http://127.0.0.1:1086");
        let _no_proxy_guard = TestEnvVarGuard::unset("NO_PROXY");
        let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
        let args = vec![
            OsString::from("-c"),
            OsString::from("chatgpt_base_url=\"http://127.0.0.1:64550/backend-api\""),
            OsString::from("-c"),
            OsString::from("openai_base_url=\"http://127.0.0.1:64550/backend-api/prodex\""),
        ];

        let plan = codex_child_plan(
            OsString::from("codex"),
            PathBuf::from("/tmp/prodex-codex-home"),
            args,
            "prodex-local",
        );

        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("127.0.0.1,localhost,::1,127.0.0.1:64550".to_string())
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "no_proxy")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("127.0.0.1,localhost,::1,127.0.0.1:64550".to_string())
        );
    }

    #[test]
    fn remove_upstream_proxy_env_preserves_local_proxy_bypass_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _http_proxy_guard = TestEnvVarGuard::set("HTTP_PROXY", "http://127.0.0.1:1086");
        let _https_proxy_guard = TestEnvVarGuard::set("https_proxy", "http://127.0.0.1:1086");
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
        let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
        let mut plan = codex_child_plan(
            OsString::from("codex"),
            PathBuf::from("/tmp/prodex-codex-home"),
            vec![],
            "prodex-local",
        );

        remove_upstream_proxy_env(&mut plan);

        assert!(plan.removed_env.iter().any(|key| key == "HTTP_PROXY"));
        assert!(plan.removed_env.iter().any(|key| key == "https_proxy"));
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("example.com,127.0.0.1,localhost,::1".to_string())
        );
    }

    #[test]
    fn runtime_proxy_codex_args_preserve_user_overrides_after_proxy_overrides() {
        let args = runtime_proxy_codex_args(
            "127.0.0.1:4455".parse().expect("socket addr"),
            &[
                OsString::from("exec"),
                OsString::from("-c"),
                OsString::from("service_tier=null"),
                OsString::from("--config=notice.fast_default_opt_out=true"),
                OsString::from("hello"),
            ],
        )
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

        assert_eq!(args[0], "-c");
        assert_eq!(
            args[1],
            "chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""
        );
        assert_eq!(args[2], "-c");
        assert_eq!(
            args[3],
            format!(
                "openai_base_url=\"http://127.0.0.1:4455{}\"",
                runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH
            )
        );
        assert_eq!(
            &args[4..],
            [
                "exec",
                "-c",
                "service_tier=null",
                "--config=notice.fast_default_opt_out=true",
                "hello"
            ]
        );
    }

    #[test]
    fn prepare_codex_launch_args_extracts_full_access_and_normalizes_resume() {
        let (args, include_code_review) = prepare_codex_launch_args(
            &[
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("--full-access"),
                OsString::from("review"),
            ],
            false,
        );

        assert_eq!(
            args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("resume"),
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("review"),
            ]
        );
        assert!(include_code_review);
    }

    #[test]
    fn prepare_codex_launch_args_extracts_prodex_full_access_passthrough_marker() {
        let (args, include_code_review) = prepare_codex_launch_args(
            &[
                OsString::from("exec"),
                OsString::from("--full-access"),
                OsString::from("review"),
            ],
            false,
        );

        assert_eq!(
            args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("exec"),
                OsString::from("review"),
            ]
        );
        assert!(include_code_review);
    }

    #[test]
    fn prepare_codex_launch_args_full_access_keeps_resume_normalization_and_review_detection() {
        let (args, include_code_review) = prepare_codex_launch_args(
            &[
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("review"),
            ],
            true,
        );

        assert_eq!(
            args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("resume"),
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("review"),
            ]
        );
        assert!(include_code_review);
    }

    #[test]
    fn profileless_local_home_requires_no_requested_profile_and_matching_provider() {
        assert!(allow_profileless_local_home(
            None,
            Some("PRODEX-LOCAL"),
            "prodex-local"
        ));
        assert!(!allow_profileless_local_home(
            Some("main"),
            Some("prodex-local"),
            "prodex-local"
        ));
        assert!(!allow_profileless_local_home(
            None,
            Some("openai"),
            "prodex-local"
        ));
        assert!(!allow_profileless_local_home(None, None, "prodex-local"));
    }

    #[test]
    fn resolve_profile_name_prefers_requested_then_active_then_single_profile() {
        let state = prodex_state::AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                ("backup".to_string(), test_profile("/tmp/backup")),
                ("main".to_string(), test_profile("/tmp/main")),
            ]),
            ..prodex_state::AppState::default()
        };

        assert_eq!(
            resolve_profile_name(&state, Some("backup")).expect("requested profile"),
            "backup"
        );
        assert_eq!(
            resolve_profile_name(&state, None).expect("active profile"),
            "main"
        );

        let single = prodex_state::AppState {
            profiles: BTreeMap::from([("only".to_string(), test_profile("/tmp/only"))]),
            ..prodex_state::AppState::default()
        };
        assert_eq!(
            resolve_profile_name(&single, None).expect("single profile"),
            "only"
        );
    }

    #[test]
    fn record_run_selection_prunes_missing_profiles_before_insert() {
        let mut state = prodex_state::AppState {
            profiles: BTreeMap::from([("main".to_string(), test_profile("/tmp/main"))]),
            last_run_selected_at: BTreeMap::from([
                ("deleted".to_string(), 10),
                ("main".to_string(), 20),
            ]),
            ..prodex_state::AppState::default()
        };

        record_run_selection_at(&mut state, "main", 30);

        assert_eq!(
            state.last_run_selected_at,
            BTreeMap::from([("main".to_string(), 30)])
        );
    }

    #[test]
    fn fixed_runtime_proxy_state_keeps_only_selected_profile() {
        let state = prodex_state::AppState {
            active_profile: Some("second".to_string()),
            profiles: BTreeMap::from([
                ("main".to_string(), test_profile("/tmp/main-home")),
                ("second".to_string(), test_profile("/tmp/second-home")),
            ]),
            last_run_selected_at: BTreeMap::from([
                ("main".to_string(), 1_000),
                ("second".to_string(), 2_000),
            ]),
            response_profile_bindings: BTreeMap::from([
                (
                    "resp-main".to_string(),
                    prodex_state::ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: 1_000,
                    },
                ),
                (
                    "resp-second".to_string(),
                    prodex_state::ResponseProfileBinding {
                        profile_name: "second".to_string(),
                        bound_at: 2_000,
                    },
                ),
            ]),
            session_profile_bindings: BTreeMap::from([
                (
                    "session-main".to_string(),
                    prodex_state::ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: 1_000,
                    },
                ),
                (
                    "session-second".to_string(),
                    prodex_state::ResponseProfileBinding {
                        profile_name: "second".to_string(),
                        bound_at: 2_000,
                    },
                ),
            ]),
        };

        let fixed = fixed_runtime_proxy_state(&state, "main").unwrap();

        assert_eq!(fixed.active_profile.as_deref(), Some("main"));
        assert_eq!(
            fixed
                .profiles
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["main"]
        );
        assert_eq!(
            fixed
                .last_run_selected_at
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["main"]
        );
        assert_eq!(
            fixed
                .response_profile_bindings
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["resp-main"]
        );
        assert_eq!(
            fixed
                .session_profile_bindings
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["session-main"]
        );
    }

    #[test]
    fn ensure_profile_path_is_unique_rejects_existing_profile_home() {
        let state = prodex_state::AppState {
            profiles: BTreeMap::from([("main".to_string(), test_profile("/tmp/main"))]),
            ..prodex_state::AppState::default()
        };

        let err = ensure_profile_path_is_unique(&state, Path::new("/tmp/main"))
            .expect_err("duplicate profile path should fail");

        assert!(err.to_string().contains("already used by profile 'main'"));
    }

    #[test]
    fn runtime_launch_dry_run_plan_builds_caveman_placeholder_cleanup() {
        let base_home = PathBuf::from("/tmp/prodex/profiles/main");
        let managed_root = PathBuf::from("/tmp/prodex/profiles");
        let plan = runtime_launch_dry_run_plan(
            OsString::from("codex"),
            &base_home,
            &managed_root,
            None,
            false,
            "prodex-local",
            RuntimeLaunchDryRunChild::Caveman {
                codex_args: vec![OsString::from("exec")],
            },
        );

        let expected_home = managed_root.join(".caveman-dry-run-from-main");
        assert_eq!(plan.child.codex_home, expected_home);
        assert_eq!(plan.child.args, vec![OsString::from("exec")]);
        assert_eq!(plan.cleanup_paths, vec![expected_home]);
    }

    #[test]
    fn runtime_launch_dry_run_report_redacts_secret_env_and_args() {
        let codex_home = PathBuf::from("/tmp/prodex-home");
        let plan = RuntimeLaunchPlan::new(
            ChildProcessPlan::new(OsString::from("codex"), codex_home.clone())
                .with_args(vec![
                    OsString::from("-c"),
                    OsString::from("model=\"gpt-5.4\""),
                    OsString::from("--config=api_key=\"secret-value\""),
                    OsString::from("--api-key"),
                    OsString::from("opaque-cli-value"),
                    OsString::from("--header"),
                    OsString::from("Authorization: Bearer dry-run-bearer-secret-12345"),
                    OsString::from("sk-proj-dry-run-secret-123456789"),
                ])
                .with_extra_env(vec![
                    ("ANTHROPIC_AUTH_TOKEN", OsString::from("secret-value")),
                    (
                        "PRODEX_VISIBLE_BEARER",
                        OsString::from("Bearer dry-run-env-bearer-secret-12345"),
                    ),
                    ("PRODEX_VISIBLE", OsString::from("1")),
                ]),
        );

        let report = runtime_launch_dry_run_report("run", &codex_home, None, &plan);

        assert!(report.contains("Model: gpt-5.4"));
        assert!(report.contains("ANTHROPIC_AUTH_TOKEN=<redacted>"));
        assert!(report.contains("PRODEX_VISIBLE=1"));
        assert!(report.contains("Bearer <redacted>"));
        assert!(report.contains("sk-proj-<redacted>"));
        assert!(report.contains("<redacted>"));
        assert!(!report.contains("secret-value"));
        assert!(!report.contains("opaque-cli-value"));
        assert!(!report.contains("dry-run-bearer-secret-12345"));
        assert!(!report.contains("dry-run-secret-123456789"));
        assert!(!report.contains("dry-run-env-bearer-secret-12345"));
        assert!(report.contains("Codex/TUI not started"));
    }

    #[test]
    fn no_ready_runtime_profiles_plan_formats_blocked_report() {
        let report = prodex_shared_types::RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: prodex_quota::AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(prodex_quota::UsageResponse {
                email: None,
                plan_type: None,
                rate_limit: Some(prodex_quota::WindowPair {
                    primary_window: Some(prodex_quota::UsageWindow {
                        used_percent: Some(100),
                        reset_at: Some(1_900_000_000),
                        limit_window_seconds: Some(18_000),
                    }),
                    secondary_window: None,
                }),
                code_review_rate_limit: None,
                additional_rate_limits: Vec::new(),
            }),
        };

        let RuntimeLaunchNoReadyProfilesPlan::Blocked {
            blocked_message,
            no_ready_message,
            inspect_hint,
            error_message,
        } = no_ready_runtime_profiles_plan(&report, "main", false)
        else {
            panic!("expected blocked plan");
        };

        assert!(blocked_message.contains("Quota preflight blocked profile 'main'"));
        assert_eq!(no_ready_message, "No ready profile was found.");
        assert!(inspect_hint.contains("prodex quota --profile main"));
        assert!(error_message.contains("no ready profile"));
    }

    #[test]
    fn blocked_selected_runtime_profile_plan_formats_rotation() {
        let blocked = vec![prodex_quota::BlockedLimit {
            message: "5h exhausted until later".to_string(),
        }];
        let plan =
            blocked_selected_runtime_profile_plan("main", &blocked, true, &["backup".to_string()]);

        assert_eq!(
            plan,
            RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
                blocked_message: "Quota preflight blocked profile 'main': 5h exhausted until later"
                    .to_string(),
                next_profile: "backup".to_string(),
                rotate_message: "Auto-rotating to profile 'backup'.".to_string(),
            }
        );
    }
}
