use super::*;

mod config;
#[cfg(test)]
mod models;
mod state_merge;

pub(super) use self::config::*;
#[cfg(test)]
pub(super) use self::models::*;
pub(super) use self::state_merge::*;

struct ClaudeLaunchStrategy {
    args: ClaudeArgs,
    claude_args: Vec<OsString>,
    launch_modes: RuntimeProxyClaudeLaunchModes,
}

impl ClaudeLaunchStrategy {
    fn new(args: ClaudeArgs) -> Self {
        let (launch_modes, claude_args) =
            runtime_proxy_claude_extract_launch_modes(&args.claude_args);
        Self {
            args,
            claude_args,
            launch_modes,
        }
    }
}

impl RuntimeLaunchStrategy for ClaudeLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: false,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: None,
            force_runtime_proxy: true,
            model_provider_override: None,
            profile_v2_name: None,
            external_provider: None,
            external_provider_api_key: None,
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let runtime_proxy =
            runtime_proxy.context("Claude Code launch requires a local runtime proxy")?;
        let claude_bin = claude_bin();
        let claude_config_dir = prepare_runtime_proxy_claude_config_dir(
            &prepared.paths,
            &prepared.codex_home,
            prepared.managed,
        )?;
        let current_dir =
            env::current_dir().context("failed to determine current directory for Claude Code")?;
        let claude_version = runtime_proxy_claude_binary_version(&claude_bin);
        ensure_runtime_proxy_claude_launch_config(
            &claude_config_dir,
            &current_dir,
            claude_version.as_deref(),
        )?;
        let mem_plugin_dir = self
            .launch_modes
            .mem_mode
            .then(runtime_mem_claude_plugin_dir)
            .transpose()?;
        if self.launch_modes.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
        }
        let caveman_plugin_dir = self
            .launch_modes
            .caveman_mode
            .then(|| prepare_runtime_proxy_claude_caveman_plugin_dir(&prepared.paths))
            .transpose()?;
        let mut plugin_dirs = Vec::new();
        if let Some(plugin_dir) = mem_plugin_dir.as_ref() {
            plugin_dirs.push(plugin_dir.clone());
        }
        if let Some(plugin_dir) = caveman_plugin_dir.as_ref() {
            plugin_dirs.push(plugin_dir.clone());
        }
        let launch_args = runtime_proxy_claude_launch_args(&self.claude_args, &plugin_dirs);
        let extra_env = runtime_proxy_claude_launch_env(
            runtime_proxy.listen_addr,
            &claude_config_dir,
            &prepared.codex_home,
            mem_plugin_dir.as_deref(),
        );
        Ok(RuntimeLaunchPlan::new(
            ChildProcessPlan::new(claude_bin, prepared.codex_home.clone())
                .with_args(launch_args)
                .with_extra_env(extra_env)
                .with_removed_env(runtime_proxy_claude_removed_env().to_vec()),
        ))
    }
}

pub(super) fn handle_claude(args: ClaudeArgs) -> Result<()> {
    execute_runtime_launch(ClaudeLaunchStrategy::new(args))
}

pub(super) fn prepare_runtime_proxy_claude_caveman_plugin_dir(paths: &AppPaths) -> Result<PathBuf> {
    prodex_caveman_assets::install_claude_caveman_plugin(&paths.root)
}

#[cfg(test)]
pub(super) fn runtime_proxy_claude_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "x-claude-code-session-id")
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "session_id"))
        .map(str::to_string)
}
