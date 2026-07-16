use super::*;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

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
                dry_run_prodex_overlay_home_placeholder(managed_profiles_root, base_codex_home);
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
) -> codex_config::CodexConfigResult<String> {
    let child = &plan.child;
    let provider = dry_run_config_value(&child.args, base_codex_home, "model_provider")?
        .unwrap_or_else(|| "openai".to_string());
    let model = dry_run_config_value(&child.args, base_codex_home, "model")?
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
    Ok(output)
}

fn dry_run_config_value(
    args: &[OsString],
    codex_home: &Path,
    key: &str,
) -> codex_config::CodexConfigResult<Option<String>> {
    if key == "model"
        && let Some(model) = dry_run_cli_model(args)
    {
        return Ok(Some(model));
    }
    if let Some(value) = codex_config::codex_cli_config_override_value(args, key) {
        return Ok(Some(value));
    }
    codex_config::codex_config_value_for_args(codex_home, args, key)
}

fn dry_run_cli_model(args: &[OsString]) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            index += 1;
            continue;
        };
        let model = if matches!(arg, "--model" | "-m") {
            index += 1;
            args.get(index).and_then(|value| value.to_str())
        } else if let Some(value) = arg.strip_prefix("--model=") {
            Some(value)
        } else if let Some(value) = arg.strip_prefix("-m") {
            (!value.is_empty()).then_some(value.trim_start_matches('='))
        } else {
            None
        };
        if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
            return Some(model.to_string());
        }
        index += 1;
    }
    None
}
