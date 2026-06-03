use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

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
    let args = crate::scope_codex_exec_config_args(&args);
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
