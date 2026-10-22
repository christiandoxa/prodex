use super::*;
use prodex_mojo_core::launch::{
    RuntimeLaunchReportInput, RuntimeLaunchReportProxy, render_runtime_launch_report,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimeLaunchPlan {
    pub child: ChildProcessPlan,
    pub companion: Option<ChildProcessPlan>,
    pub companion_unix_socket: Option<PathBuf>,
    pub cleanup_paths: Vec<PathBuf>,
    /// Launch-owned root-session notification, never a global latest-session lookup.
    pub saved_session_marker: Option<PathBuf>,
}

impl RuntimeLaunchPlan {
    pub fn new(child: ChildProcessPlan) -> Self {
        Self {
            child,
            companion: None,
            companion_unix_socket: None,
            cleanup_paths: Vec::new(),
            saved_session_marker: None,
        }
    }

    #[cfg(unix)]
    pub fn with_unix_companion(mut self, companion: ChildProcessPlan, socket: PathBuf) -> Self {
        self.companion = Some(companion);
        self.companion_unix_socket = Some(socket);
        self
    }

    pub fn with_saved_session_marker(mut self, path: PathBuf) -> Self {
        self.saved_session_marker = Some(path);
        self
    }

    pub fn with_cleanup_path(mut self, path: PathBuf) -> Self {
        self.cleanup_paths.push(path);
        self
    }
}

pub fn cleanup_runtime_launch_plan(plan: &RuntimeLaunchPlan) {
    for path in &plan.cleanup_paths {
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
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
                dry_run_prodex_overlay_home_placeholder(managed_profiles_root, base_codex_home);
            (caveman_home.clone(), Some(caveman_home))
        }
    };
    let mut child = codex_tui_child_plan(binary, codex_home, runtime_args, local_provider_id);
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
) -> codex_config::CodexConfigResult<String> {
    let child = &plan.child;
    let current_dir = std::env::current_dir().ok();
    let redact = |value: &str| {
        dry_run_redact_paths(
            value,
            [
                Some(child.codex_home.as_path()),
                Some(base_codex_home),
                current_dir.as_deref(),
            ],
        )
    };
    let provider = dry_run_config_value(&child.args, base_codex_home, "model_provider")?;
    let model = dry_run_config_value(&child.args, base_codex_home, "model")?;
    let binary = redaction::redaction_display_os(
        Path::new(&child.binary)
            .file_name()
            .unwrap_or(child.binary.as_os_str()),
    );
    let args = redaction::redaction_redacted_cli_args(&child.args)
        .into_iter()
        .map(|arg| redact(&arg))
        .collect::<Vec<_>>();
    let env = child
        .extra_env
        .iter()
        .map(|(key, value)| {
            let value = redaction::redaction_redacted_env_value(key, value);
            (redaction::redaction_display_os(key), redact(&value))
        })
        .collect::<Vec<_>>();
    let removed_env = child
        .removed_env
        .iter()
        .map(|key| redaction::redaction_display_os(key))
        .collect::<Vec<_>>();
    let proxy_address = runtime_proxy.map(|proxy| proxy.listen_addr.to_string());
    // As with path redaction, native formatting failures are internal invariants.
    Ok(render_runtime_launch_report(RuntimeLaunchReportInput {
        flow,
        binary: &binary,
        provider: provider.as_deref(),
        model: model.as_deref(),
        proxy: runtime_proxy
            .zip(proxy_address.as_deref())
            .map(|(proxy, address)| RuntimeLaunchReportProxy {
                address,
                mount: proxy.openai_mount_path,
                port: proxy.listen_addr.port(),
            }),
        args: &args,
        env: &env,
        removed_env: &removed_env,
    })
    .expect("Mojo runtime launch report returned invalid output"))
}

fn dry_run_redact_paths<'a>(
    value: &str,
    paths: impl IntoIterator<Item = Option<&'a Path>>,
) -> String {
    let display_paths = paths
        .into_iter()
        .flatten()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    prodex_mojo_core::redaction::redact_dry_run_paths(value, &display_paths)
        .expect("Mojo dry-run path redaction policy returned invalid output")
}

fn dry_run_config_value(
    args: &[OsString],
    codex_home: &Path,
    key: &str,
) -> codex_config::CodexConfigResult<Option<String>> {
    if key == "model"
        && let Some(model) = runtime_launch_cli_model(args)
    {
        return Ok(Some(model));
    }
    if let Some(value) = codex_config::codex_cli_config_override_value(args, key) {
        return Ok(Some(value));
    }
    codex_config::codex_config_value_for_args(codex_home, args, key)
}
