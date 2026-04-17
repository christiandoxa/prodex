use super::*;

mod config;
mod models;
mod state_merge;

pub(super) use self::config::*;
pub(super) use self::models::*;
pub(super) use self::state_merge::*;

const PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME: &str = "caveman";

struct EmbeddedClaudeCavemanFile {
    relative_path: &'static str,
    contents: &'static str,
}

const CLAUDE_CAVEMAN_PLUGIN_FILES: &[EmbeddedClaudeCavemanFile] = &[
    EmbeddedClaudeCavemanFile {
        relative_path: ".claude-plugin/plugin.json",
        contents: include_str!("caveman_assets/claude/.claude-plugin/plugin.json"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "commands/caveman.toml",
        contents: include_str!("caveman_assets/claude/commands/caveman.toml"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "commands/caveman-commit.toml",
        contents: include_str!("caveman_assets/claude/commands/caveman-commit.toml"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "commands/caveman-review.toml",
        contents: include_str!("caveman_assets/claude/commands/caveman-review.toml"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "hooks/caveman-activate.js",
        contents: include_str!("caveman_assets/claude/hooks/caveman-activate.js"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "hooks/caveman-config.js",
        contents: include_str!("caveman_assets/claude/hooks/caveman-config.js"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "hooks/caveman-mode-tracker.js",
        contents: include_str!("caveman_assets/claude/hooks/caveman-mode-tracker.js"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "hooks/caveman-statusline.ps1",
        contents: include_str!("caveman_assets/claude/hooks/caveman-statusline.ps1"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "hooks/caveman-statusline.sh",
        contents: include_str!("caveman_assets/claude/hooks/caveman-statusline.sh"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "skills/caveman/SKILL.md",
        contents: include_str!("caveman_assets/skills/caveman/SKILL.md"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "skills/caveman-commit/SKILL.md",
        contents: include_str!("caveman_assets/claude/skills/caveman-commit/SKILL.md"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "skills/caveman-help/SKILL.md",
        contents: include_str!("caveman_assets/claude/skills/caveman-help/SKILL.md"),
    },
    EmbeddedClaudeCavemanFile {
        relative_path: "skills/caveman-review/SKILL.md",
        contents: include_str!("caveman_assets/claude/skills/caveman-review/SKILL.md"),
    },
];

struct ClaudeLaunchStrategy {
    args: ClaudeArgs,
    claude_args: Vec<OsString>,
    launch_modes: RuntimeProxyClaudeLaunchModes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RuntimeProxyClaudeLaunchModes {
    pub(super) caveman_mode: bool,
    pub(super) mem_mode: bool,
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
            include_code_review: false,
            force_runtime_proxy: true,
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

pub(super) fn runtime_proxy_claude_extract_launch_modes(
    claude_args: &[OsString],
) -> (RuntimeProxyClaudeLaunchModes, Vec<OsString>) {
    let mut launch_modes = RuntimeProxyClaudeLaunchModes::default();
    let mut prefix_len = 0;
    while let Some(arg) = claude_args.get(prefix_len).and_then(|value| value.to_str()) {
        match arg {
            "caveman" => launch_modes.caveman_mode = true,
            "mem" => launch_modes.mem_mode = true,
            _ => break,
        }
        prefix_len += 1;
    }
    (launch_modes, claude_args[prefix_len..].to_vec())
}

pub(super) fn runtime_proxy_claude_launch_args(
    claude_args: &[OsString],
    plugin_dirs: &[PathBuf],
) -> Vec<OsString> {
    let mut args = Vec::with_capacity(claude_args.len() + plugin_dirs.len() * 2);
    for plugin_dir in plugin_dirs {
        args.push(OsString::from("--plugin-dir"));
        args.push(plugin_dir.as_os_str().to_os_string());
    }
    args.extend(claude_args.iter().cloned());
    args
}

pub(super) fn prepare_runtime_proxy_claude_caveman_plugin_dir(paths: &AppPaths) -> Result<PathBuf> {
    let plugin_dir = paths
        .root
        .join("claude-plugins")
        .join(PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME);
    for file in CLAUDE_CAVEMAN_PLUGIN_FILES {
        let path = plugin_dir.join(file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, file.contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(plugin_dir)
}

pub(super) fn runtime_proxy_claude_launch_env(
    listen_addr: std::net::SocketAddr,
    config_dir: &Path,
    codex_home: &Path,
    mem_plugin_root: Option<&Path>,
) -> Vec<(&'static str, OsString)> {
    let target_model = runtime_proxy_claude_launch_model(codex_home);
    let base_url = format!("http://{listen_addr}");
    let mut env = vec![
        ("CLAUDE_CONFIG_DIR", OsString::from(config_dir.as_os_str())),
        ("ANTHROPIC_BASE_URL", OsString::from(base_url.as_str())),
        (
            "ANTHROPIC_AUTH_TOKEN",
            OsString::from(PRODEX_CLAUDE_PROXY_API_KEY),
        ),
        (
            "ANTHROPIC_MODEL",
            OsString::from(runtime_proxy_claude_picker_model(&target_model)),
        ),
    ];
    if let Some(plugin_root) = mem_plugin_root {
        env.push((
            "CLAUDE_PLUGIN_ROOT",
            OsString::from(plugin_root.as_os_str()),
        ));
    }
    if runtime_proxy_claude_use_foundry_compat() {
        env.push(("CLAUDE_CODE_USE_FOUNDRY", OsString::from("1")));
        env.push(("ANTHROPIC_FOUNDRY_BASE_URL", OsString::from(base_url)));
        env.push((
            "ANTHROPIC_FOUNDRY_API_KEY",
            OsString::from(PRODEX_CLAUDE_PROXY_API_KEY),
        ));
    }
    env.extend(runtime_proxy_claude_pinned_alias_env());
    env.extend(runtime_proxy_claude_custom_model_option_env(&target_model));
    env
}

pub(super) fn runtime_proxy_claude_removed_env() -> &'static [&'static str] {
    &[
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_USE_ANTHROPIC_AWS",
        "ANTHROPIC_BEDROCK_BASE_URL",
        "ANTHROPIC_VERTEX_BASE_URL",
        "ANTHROPIC_FOUNDRY_BASE_URL",
        "ANTHROPIC_AWS_BASE_URL",
        "ANTHROPIC_FOUNDRY_RESOURCE",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "ANTHROPIC_AWS_WORKSPACE_ID",
        "CLOUD_ML_REGION",
        "ANTHROPIC_FOUNDRY_API_KEY",
        "ANTHROPIC_AWS_API_KEY",
        "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
        "CLAUDE_CODE_SKIP_VERTEX_AUTH",
        "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
        "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
        "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
        "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
        "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
        "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
        "ANTHROPIC_CUSTOM_MODEL_OPTION",
        "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
        "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION",
    ]
}

pub(super) fn runtime_proxy_claude_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "x-claude-code-session-id")
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "session_id"))
        .map(str::to_string)
}
