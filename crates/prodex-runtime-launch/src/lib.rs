use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
