use super::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeProxyClaudeModelAlias {
    Opus,
    Sonnet,
    Haiku,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeProxyResponsesModelDescriptor {
    pub(super) id: &'static str,
    pub(super) display_name: &'static str,
    pub(super) description: &'static str,
    pub(super) claude_alias: Option<RuntimeProxyClaudeModelAlias>,
    pub(super) claude_picker_model: Option<&'static str>,
    pub(super) supports_xhigh: bool,
}

pub(super) fn handle_claude(args: ClaudeArgs) -> Result<()> {
    let (caveman_mode, claude_args) = runtime_proxy_claude_extract_caveman_mode(&args.claude_args);
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        include_code_review: false,
        force_runtime_proxy: true,
    })?;
    let runtime_proxy = prepared
        .runtime_proxy
        .context("Claude Code launch requires a local runtime proxy")?;
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
    let caveman_plugin_dir = caveman_mode
        .then(|| prepare_runtime_proxy_claude_caveman_plugin_dir(&prepared.paths))
        .transpose()?;
    let extra_env = runtime_proxy_claude_launch_env(
        runtime_proxy.listen_addr,
        &claude_config_dir,
        &prepared.codex_home,
    );
    let launch_args = runtime_proxy_claude_launch_args(&claude_args, caveman_plugin_dir.as_deref());
    let status = run_child(
        &claude_bin,
        &launch_args,
        &prepared.codex_home,
        &extra_env,
        runtime_proxy_claude_removed_env(),
        Some(&runtime_proxy),
    )?;
    drop(runtime_proxy);
    exit_with_status(status)
}

pub(super) fn runtime_proxy_claude_extract_caveman_mode(
    claude_args: &[OsString],
) -> (bool, Vec<OsString>) {
    let Some(first) = claude_args.first().and_then(|arg| arg.to_str()) else {
        return (false, claude_args.to_vec());
    };
    if first != "caveman" {
        return (false, claude_args.to_vec());
    }
    (true, claude_args[1..].to_vec())
}

pub(super) fn runtime_proxy_claude_launch_args(
    claude_args: &[OsString],
    plugin_dir: Option<&Path>,
) -> Vec<OsString> {
    let mut args = Vec::with_capacity(claude_args.len() + usize::from(plugin_dir.is_some()) * 2);
    if let Some(plugin_dir) = plugin_dir {
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

pub(super) fn runtime_proxy_claude_model_override() -> Option<String> {
    env::var("PRODEX_CLAUDE_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn parse_toml_string_assignment(contents: &str, key: &str) -> Option<String> {
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('=')?.trim_start();
        let rest = rest.strip_prefix('"')?;
        let mut value = String::new();
        let mut escaped = false;
        for ch in rest.chars() {
            if escaped {
                value.push(match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => return Some(value),
                other => value.push(other),
            }
        }
    }
    None
}

pub(super) fn runtime_proxy_claude_config_value(codex_home: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    parse_toml_string_assignment(&contents, key).filter(|value| !value.trim().is_empty())
}

pub(super) fn runtime_proxy_normalize_responses_reasoning_effort(
    effort: &str,
) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "none" => Some("none"),
        // Claude Code exposes `max`; treat it as the strongest explicit upstream tier.
        "max" => Some("xhigh"),
        _ => None,
    }
}

pub(super) fn runtime_proxy_claude_reasoning_effort_override() -> Option<String> {
    env::var("PRODEX_CLAUDE_REASONING_EFFORT")
        .ok()
        .and_then(|value| {
            runtime_proxy_normalize_responses_reasoning_effort(value.trim()).map(str::to_string)
        })
}

pub(super) fn runtime_proxy_claude_native_client_tool_enabled(enabled_tokens: &[&str]) -> bool {
    let Some(value) = env::var("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS").ok() else {
        return false;
    };
    let mut enabled = false;
    for token in value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        match token.to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" | "none" => return false,
            "1" | "true" | "yes" | "on" | "all" => enabled = true,
            value if enabled_tokens.contains(&value) => enabled = true,
            _ => {}
        }
    }
    enabled
}

pub(super) fn runtime_proxy_claude_native_shell_enabled() -> bool {
    runtime_proxy_claude_native_client_tool_enabled(&["shell", "bash"])
}

pub(super) fn runtime_proxy_claude_native_computer_enabled() -> bool {
    runtime_proxy_claude_native_client_tool_enabled(&["computer"])
}

pub(super) fn runtime_proxy_claude_launch_model(codex_home: &Path) -> String {
    runtime_proxy_claude_model_override()
        .or_else(|| runtime_proxy_claude_config_value(codex_home, "model"))
        .unwrap_or_else(|| DEFAULT_PRODEX_CLAUDE_MODEL.to_string())
}

pub(super) fn runtime_proxy_claude_alias_env_keys(
    alias: RuntimeProxyClaudeModelAlias,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match alias {
        RuntimeProxyClaudeModelAlias::Opus => (
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
        ),
        RuntimeProxyClaudeModelAlias::Sonnet => (
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
        ),
        RuntimeProxyClaudeModelAlias::Haiku => (
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
        ),
    }
}

pub(super) fn runtime_proxy_claude_alias_model(
    alias: RuntimeProxyClaudeModelAlias,
) -> &'static RuntimeProxyResponsesModelDescriptor {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .find(|descriptor| descriptor.claude_alias == Some(alias))
        .expect("Claude alias model should exist")
}

pub(super) fn runtime_proxy_claude_picker_model_descriptor(
    picker_model: &str,
) -> Option<&'static RuntimeProxyResponsesModelDescriptor> {
    let normalized = picker_model.trim();
    let without_extended_context = normalized.strip_suffix("[1m]").unwrap_or(normalized);
    runtime_proxy_responses_model_descriptor(without_extended_context).or_else(|| {
        runtime_proxy_responses_model_descriptors()
            .iter()
            .find(|descriptor| {
                descriptor
                    .claude_picker_model
                    .is_some_and(|value| value.eq_ignore_ascii_case(without_extended_context))
                    || descriptor.claude_alias.is_some_and(|alias| {
                        runtime_proxy_claude_alias_picker_value(alias)
                            .eq_ignore_ascii_case(without_extended_context)
                    })
            })
    })
}

pub(super) fn runtime_proxy_responses_model_descriptor(
    model_id: &str,
) -> Option<&'static RuntimeProxyResponsesModelDescriptor> {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .find(|descriptor| descriptor.id.eq_ignore_ascii_case(model_id))
}

pub(super) fn runtime_proxy_responses_model_capabilities(model_id: &str) -> &'static str {
    if runtime_proxy_responses_model_supports_xhigh(model_id) {
        "effort,max_effort,thinking,adaptive_thinking,interleaved_thinking"
    } else {
        "effort,thinking,adaptive_thinking,interleaved_thinking"
    }
}

pub(super) fn runtime_proxy_responses_model_supported_effort_levels(
    model_id: &str,
) -> &'static [&'static str] {
    if runtime_proxy_responses_model_supports_xhigh(model_id) {
        &["low", "medium", "high", "max"]
    } else {
        &["low", "medium", "high"]
    }
}

pub(super) fn runtime_proxy_responses_model_supports_xhigh(model_id: &str) -> bool {
    runtime_proxy_responses_model_descriptor(model_id)
        .map(|descriptor| descriptor.supports_xhigh)
        .unwrap_or_else(|| {
            matches!(
                model_id.trim().to_ascii_lowercase().as_str(),
                value
                    if value.starts_with("gpt-5.2")
                        || value.starts_with("gpt-5.3")
                        || value.starts_with("gpt-5.4")
            )
        })
}

pub(super) fn runtime_proxy_claude_use_foundry_compat() -> bool {
    true
}

pub(super) fn runtime_proxy_claude_alias_picker_value(
    alias: RuntimeProxyClaudeModelAlias,
) -> &'static str {
    match alias {
        RuntimeProxyClaudeModelAlias::Opus => "opus",
        RuntimeProxyClaudeModelAlias::Sonnet => "sonnet",
        RuntimeProxyClaudeModelAlias::Haiku => "haiku",
    }
}

pub(super) fn runtime_proxy_claude_pinned_alias_env() -> Vec<(&'static str, OsString)> {
    let mut env = Vec::new();
    for alias in [
        RuntimeProxyClaudeModelAlias::Opus,
        RuntimeProxyClaudeModelAlias::Sonnet,
        RuntimeProxyClaudeModelAlias::Haiku,
    ] {
        let descriptor = runtime_proxy_claude_alias_model(alias);
        let (model_key, name_key, description_key, caps_key) =
            runtime_proxy_claude_alias_env_keys(alias);
        env.push((model_key, OsString::from(descriptor.id)));
        env.push((name_key, OsString::from(descriptor.id)));
        env.push((description_key, OsString::from(descriptor.description)));
        env.push((
            caps_key,
            OsString::from(runtime_proxy_responses_model_capabilities(descriptor.id)),
        ));
    }
    env
}

pub(super) fn runtime_proxy_claude_picker_model(target_model: &str) -> String {
    runtime_proxy_responses_model_descriptor(target_model)
        .map(|descriptor| {
            if runtime_proxy_claude_use_foundry_compat() {
                descriptor
                    .claude_alias
                    .map(runtime_proxy_claude_alias_picker_value)
                    .unwrap_or(descriptor.id)
            } else {
                descriptor.id
            }
        })
        .unwrap_or(target_model)
        .to_string()
}

pub(super) fn runtime_proxy_claude_custom_model_option_env(
    target_model: &str,
) -> Vec<(&'static str, OsString)> {
    if runtime_proxy_responses_model_descriptor(target_model).is_some() {
        return Vec::new();
    }

    let descriptor = runtime_proxy_responses_model_descriptor(target_model);
    let display_name = descriptor
        .map(|descriptor| descriptor.display_name)
        .unwrap_or(target_model);
    let description = descriptor
        .map(|descriptor| descriptor.description.to_string())
        .unwrap_or_else(|| format!("Custom OpenAI model routed through prodex ({target_model})"));

    vec![
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION",
            OsString::from(target_model),
        ),
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
            OsString::from(display_name),
        ),
        (
            "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION",
            OsString::from(description),
        ),
    ]
}

pub(super) fn runtime_proxy_claude_additional_model_option_entries() -> Vec<serde_json::Value> {
    runtime_proxy_responses_model_descriptors()
        .iter()
        .filter(|descriptor| {
            !(runtime_proxy_claude_use_foundry_compat() && descriptor.claude_alias.is_some())
        })
        .map(|descriptor| {
            let supported_effort_levels =
                runtime_proxy_responses_model_supported_effort_levels(descriptor.id);
            serde_json::json!({
                "value": descriptor.id,
                "label": descriptor.id,
                "description": descriptor.description,
                "supportsEffort": true,
                "supportedEffortLevels": supported_effort_levels,
            })
        })
        .collect()
}

pub(super) fn runtime_proxy_claude_managed_model_option_value(value: &str) -> bool {
    runtime_proxy_claude_picker_model_descriptor(value).is_some()
}

pub(super) fn runtime_proxy_claude_config_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(PRODEX_CLAUDE_CONFIG_DIR_NAME)
}

pub(super) fn runtime_proxy_shared_claude_config_dir(paths: &AppPaths) -> PathBuf {
    paths.root.join(PRODEX_SHARED_CLAUDE_DIR_NAME)
}

pub(super) fn runtime_proxy_claude_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_CONFIG_FILE_NAME)
}

pub(super) fn runtime_proxy_claude_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_SETTINGS_FILE_NAME)
}

pub(super) fn runtime_proxy_claude_legacy_import_marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME)
}

pub(super) fn legacy_default_claude_config_dir() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_DIR_NAME))
}

pub(super) fn legacy_default_claude_config_path() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_FILE_NAME))
}

pub(super) fn prepare_runtime_proxy_claude_config_dir(
    paths: &AppPaths,
    codex_home: &Path,
    managed: bool,
) -> Result<PathBuf> {
    let profile_dir = runtime_proxy_claude_config_dir(codex_home);
    if !managed {
        prepare_runtime_proxy_claude_import_target(&profile_dir)?;
        return Ok(profile_dir);
    }

    let shared_dir = runtime_proxy_shared_claude_config_dir(paths);
    prepare_runtime_proxy_claude_import_target(&shared_dir)?;
    migrate_runtime_proxy_claude_profile_dir_to_target(&profile_dir, &shared_dir)?;
    ensure_runtime_proxy_claude_profile_link(&profile_dir, &shared_dir)?;
    Ok(profile_dir)
}

pub(super) fn prepare_runtime_proxy_claude_import_target(target_dir: &Path) -> Result<()> {
    create_codex_home_if_missing(target_dir)?;
    maybe_import_runtime_proxy_claude_legacy_home(target_dir)
}

pub(super) fn maybe_import_runtime_proxy_claude_legacy_home(target_dir: &Path) -> Result<()> {
    let marker_path = runtime_proxy_claude_legacy_import_marker_path(target_dir);
    if marker_path.exists() {
        return Ok(());
    }

    let mut imported = false;
    if let Ok(legacy_dir) = legacy_default_claude_config_dir()
        && legacy_dir.is_dir()
    {
        merge_runtime_proxy_claude_directory_contents(&legacy_dir, target_dir)?;
        imported = true;
    }
    if let Ok(legacy_config_path) = legacy_default_claude_config_path()
        && legacy_config_path.is_file()
    {
        merge_runtime_proxy_claude_file(
            &legacy_config_path,
            &runtime_proxy_claude_config_path(target_dir),
        )?;
        imported = true;
    }

    if imported {
        fs::write(&marker_path, "imported\n").with_context(|| {
            format!(
                "failed to write Claude legacy import marker at {}",
                marker_path.display()
            )
        })?;
    }

    Ok(())
}

pub(super) fn migrate_runtime_proxy_claude_profile_dir_to_target(
    profile_dir: &Path,
    target_dir: &Path,
) -> Result<()> {
    if same_path(profile_dir, target_dir) {
        return Ok(());
    }

    let metadata = match fs::symlink_metadata(profile_dir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", profile_dir.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        let source_dir = runtime_proxy_resolve_symlink_target(profile_dir)?;
        if !source_dir.exists() || same_path(&source_dir, target_dir) {
            return Ok(());
        }
        if !source_dir.is_dir() {
            bail!(
                "expected {} to point to a Claude config directory",
                profile_dir.display()
            );
        }
        merge_runtime_proxy_claude_directory_contents(&source_dir, target_dir)?;
        runtime_proxy_remove_path(profile_dir)?;
        return Ok(());
    }

    if !metadata.is_dir() {
        bail!(
            "expected {} to be a Claude config directory",
            profile_dir.display()
        );
    }

    merge_runtime_proxy_claude_directory_contents(profile_dir, target_dir)?;
    fs::remove_dir_all(profile_dir)
        .with_context(|| format!("failed to remove {}", profile_dir.display()))?;
    Ok(())
}

pub(super) fn ensure_runtime_proxy_claude_profile_link(
    link_path: &Path,
    target_dir: &Path,
) -> Result<()> {
    if same_path(link_path, target_dir) {
        return Ok(());
    }

    match fs::symlink_metadata(link_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let existing_target = runtime_proxy_resolve_symlink_target(link_path)?;
                if same_path(&existing_target, target_dir) {
                    return Ok(());
                }
            }
            runtime_proxy_remove_path(link_path)?;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", link_path.display()));
        }
    }

    runtime_proxy_create_directory_symlink(target_dir, link_path)
}

pub(super) fn merge_runtime_proxy_claude_directory_contents(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    if same_path(source, destination) {
        return Ok(());
    }
    create_codex_home_if_missing(destination)?;

    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", source_path.display()))?;

        if file_type.is_dir() {
            merge_runtime_proxy_claude_directory_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            merge_runtime_proxy_claude_file(&source_path, &destination_path)?;
        } else if file_type.is_symlink() {
            merge_runtime_proxy_claude_symlink(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

pub(super) fn merge_runtime_proxy_claude_file(source: &Path, destination: &Path) -> Result<()> {
    if same_path(source, destination) {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if !destination.exists() {
        fs::copy(source, destination).with_context(|| {
            format!(
                "failed to copy Claude state file {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        return Ok(());
    }

    if destination.is_dir() {
        bail!(
            "expected {} to be a file for Claude state",
            destination.display()
        );
    }

    let file_name = source.file_name().and_then(|name| name.to_str());
    if file_name == Some(DEFAULT_CLAUDE_CONFIG_FILE_NAME) {
        return merge_runtime_proxy_claude_json_file(source, destination);
    }
    if file_name == Some(DEFAULT_CLAUDE_SETTINGS_FILE_NAME) {
        return merge_runtime_proxy_claude_json_file(source, destination);
    }
    if source
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
    {
        return merge_runtime_proxy_claude_jsonl_file(source, destination);
    }

    Ok(())
}

pub(super) fn merge_runtime_proxy_claude_json_file(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    let source_raw = fs::read_to_string(source)
        .with_context(|| format!("failed to read {}", source.display()))?;
    let destination_raw = fs::read_to_string(destination)
        .with_context(|| format!("failed to read {}", destination.display()))?;
    let source_value = match serde_json::from_str::<serde_json::Value>(&source_raw) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    let mut destination_value = match serde_json::from_str::<serde_json::Value>(&destination_raw) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    runtime_proxy_merge_json_defaults(&mut destination_value, &source_value);
    let rendered = serde_json::to_string_pretty(&destination_value)
        .context("failed to render merged Claude config")?;
    fs::write(destination, rendered)
        .with_context(|| format!("failed to write {}", destination.display()))
}

pub(super) fn runtime_proxy_merge_json_defaults(
    destination: &mut serde_json::Value,
    source_defaults: &serde_json::Value,
) {
    if destination.is_null() {
        *destination = source_defaults.clone();
        return;
    }

    if let (Some(destination), Some(source_defaults)) =
        (destination.as_object_mut(), source_defaults.as_object())
    {
        for (key, source_value) in source_defaults {
            if let Some(destination_value) = destination.get_mut(key) {
                runtime_proxy_merge_json_defaults(destination_value, source_value);
            } else {
                destination.insert(key.clone(), source_value.clone());
            }
        }
    }
}

pub(super) fn merge_runtime_proxy_claude_jsonl_file(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    fn load_jsonl_lines(
        path: &Path,
        merged: &mut Vec<String>,
        seen: &mut BTreeSet<String>,
    ) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for raw_line in content.lines() {
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() || !seen.insert(line.to_string()) {
                continue;
            }
            merged.push(line.to_string());
        }
        Ok(())
    }

    let mut merged = Vec::new();
    let mut seen = BTreeSet::new();
    load_jsonl_lines(destination, &mut merged, &mut seen)?;
    load_jsonl_lines(source, &mut merged, &mut seen)?;
    fs::write(destination, merged.join("\n"))
        .with_context(|| format!("failed to write {}", destination.display()))
}

pub(super) fn merge_runtime_proxy_claude_symlink(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() || fs::symlink_metadata(destination).is_ok() {
        return Ok(());
    }

    let target = fs::read_link(source)
        .with_context(|| format!("failed to read symlink {}", source.display()))?;
    runtime_proxy_create_symlink(&target, destination, true)
}

pub(super) fn runtime_proxy_resolve_symlink_target(path: &Path) -> Result<PathBuf> {
    let target = fs::read_link(path)
        .with_context(|| format!("failed to read symlink {}", path.display()))?;
    Ok(if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(target)
    })
}

pub(super) fn runtime_proxy_remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove symbolic link {}", path.display()))?;
        return Ok(());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
    }
}

pub(super) fn runtime_proxy_create_directory_symlink(target: &Path, link: &Path) -> Result<()> {
    runtime_proxy_create_symlink(target, link, true)
}

pub(super) fn runtime_proxy_create_symlink(target: &Path, link: &Path, is_dir: bool) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    #[cfg(unix)]
    {
        let _ = is_dir;
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link Claude state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        if is_dir {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
        .with_context(|| {
            format!(
                "failed to link Claude state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = is_dir;
        bail!("Claude state links are not supported on this platform");
    }

    Ok(())
}

pub(super) fn runtime_proxy_claude_binary_version(binary: &OsString) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stdout)).or_else(
        || parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stderr)),
    )
}

pub(super) fn parse_runtime_proxy_claude_version_text(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}

pub(super) fn ensure_runtime_proxy_claude_launch_config(
    config_dir: &Path,
    cwd: &Path,
    claude_version: Option<&str>,
) -> Result<()> {
    fs::create_dir_all(config_dir).with_context(|| {
        format!(
            "failed to create Claude Code config dir at {}",
            config_dir.display()
        )
    })?;
    let config_path = runtime_proxy_claude_config_path(config_dir);
    let raw = fs::read_to_string(&config_path).ok();
    let mut config = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !config.is_object() {
        config = serde_json::json!({});
    }

    let object = config
        .as_object_mut()
        .expect("Claude Code config should be normalized to an object");
    object.remove("skipWebFetchPreflight");
    let num_startups = object
        .get("numStartups")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .max(1);
    object.insert("numStartups".to_string(), serde_json::json!(num_startups));
    object.insert(
        "hasCompletedOnboarding".to_string(),
        serde_json::json!(true),
    );
    if let Some(version) = claude_version {
        object.insert(
            "lastOnboardingVersion".to_string(),
            serde_json::json!(version),
        );
    }
    let mut additional_model_options = runtime_proxy_claude_additional_model_option_entries();
    if let Some(existing) = object
        .get("additionalModelOptionsCache")
        .and_then(serde_json::Value::as_array)
    {
        for entry in existing {
            let existing_value = entry.get("value").and_then(serde_json::Value::as_str);
            if existing_value.is_some_and(runtime_proxy_claude_managed_model_option_value) {
                continue;
            }
            additional_model_options.push(entry.clone());
        }
    }
    object.insert(
        "additionalModelOptionsCache".to_string(),
        serde_json::Value::Array(additional_model_options),
    );

    let projects = object
        .entry("projects".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !projects.is_object() {
        *projects = serde_json::json!({});
    }
    let projects = projects
        .as_object_mut()
        .expect("Claude Code projects config should be an object");
    let project_key = cwd.to_string_lossy().into_owned();
    let project = projects
        .entry(project_key)
        .or_insert_with(|| serde_json::json!({}));
    if !project.is_object() {
        *project = serde_json::json!({});
    }
    let project = project
        .as_object_mut()
        .expect("Claude Code project config should be an object");
    project.insert(
        "hasTrustDialogAccepted".to_string(),
        serde_json::json!(true),
    );
    let project_onboarding_seen_count = project
        .get("projectOnboardingSeenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .max(1);
    project.insert(
        "projectOnboardingSeenCount".to_string(),
        serde_json::json!(project_onboarding_seen_count),
    );
    for key in [
        "allowedTools",
        "mcpContextUris",
        "enabledMcpjsonServers",
        "disabledMcpjsonServers",
        "exampleFiles",
    ] {
        if !project.get(key).is_some_and(serde_json::Value::is_array) {
            project.insert(key.to_string(), serde_json::json!([]));
        }
    }
    if let Some(allowed_tools) = project
        .get_mut("allowedTools")
        .and_then(serde_json::Value::as_array_mut)
    {
        let mut seen = BTreeSet::new();
        for entry in allowed_tools.iter() {
            if let Some(tool_name) = entry.as_str() {
                seen.insert(tool_name.to_string());
            }
        }
        for tool_name in PRODEX_CLAUDE_DEFAULT_WEB_TOOLS {
            if seen.insert((*tool_name).to_string()) {
                allowed_tools.push(serde_json::Value::String((*tool_name).to_string()));
            }
        }
    }
    if !project
        .get("mcpServers")
        .is_some_and(serde_json::Value::is_object)
    {
        project.insert("mcpServers".to_string(), serde_json::json!({}));
    }
    project.insert(
        "hasClaudeMdExternalIncludesApproved".to_string(),
        serde_json::json!(
            project
                .get("hasClaudeMdExternalIncludesApproved")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        ),
    );
    project.insert(
        "hasClaudeMdExternalIncludesWarningShown".to_string(),
        serde_json::json!(
            project
                .get("hasClaudeMdExternalIncludesWarningShown")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        ),
    );

    let rendered =
        serde_json::to_string_pretty(&config).context("failed to render Claude Code config")?;
    fs::write(&config_path, rendered).with_context(|| {
        format!(
            "failed to write Claude Code config at {}",
            config_path.display()
        )
    })?;
    ensure_runtime_proxy_claude_settings(config_dir)?;
    Ok(())
}

pub(super) fn ensure_runtime_proxy_claude_settings(config_dir: &Path) -> Result<()> {
    let settings_path = runtime_proxy_claude_settings_path(config_dir);
    let raw = fs::read_to_string(&settings_path).ok();
    let mut settings = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !settings.is_object() {
        settings = serde_json::json!({});
    }

    let object = settings
        .as_object_mut()
        .expect("Claude Code settings should be normalized to an object");
    object.insert("skipWebFetchPreflight".to_string(), serde_json::json!(true));
    let permissions = object
        .entry("permissions".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !permissions.is_object() {
        *permissions = serde_json::json!({});
    }
    let permissions = permissions
        .as_object_mut()
        .expect("Claude Code permissions should be normalized to an object");
    let allow = permissions
        .entry("allow".to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !allow.is_array() {
        *allow = serde_json::json!([]);
    }
    if let Some(allow) = allow.as_array_mut() {
        let mut seen = BTreeSet::new();
        for entry in allow.iter() {
            if let Some(tool_name) = entry.as_str() {
                seen.insert(tool_name.to_string());
            }
        }
        for tool_name in PRODEX_CLAUDE_DEFAULT_WEB_TOOLS {
            if seen.insert((*tool_name).to_string()) {
                allow.push(serde_json::Value::String((*tool_name).to_string()));
            }
        }
    }

    let rendered =
        serde_json::to_string_pretty(&settings).context("failed to render Claude Code settings")?;
    fs::write(&settings_path, rendered).with_context(|| {
        format!(
            "failed to write Claude Code settings at {}",
            settings_path.display()
        )
    })?;
    Ok(())
}
pub(super) fn runtime_proxy_responses_model_descriptors()
-> &'static [RuntimeProxyResponsesModelDescriptor] {
    &[
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.4",
            display_name: "GPT-5.4",
            description: "Latest frontier agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Opus),
            claude_picker_model: Some("claude-opus-4-6"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.4-mini",
            display_name: "GPT-5.4 Mini",
            description: "Smaller frontier agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Haiku),
            claude_picker_model: Some("claude-haiku-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.3-codex",
            display_name: "GPT-5.3 Codex",
            description: "Frontier Codex-optimized agentic coding model.",
            claude_alias: Some(RuntimeProxyClaudeModelAlias::Sonnet),
            claude_picker_model: Some("claude-sonnet-4-6"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.2-codex",
            display_name: "GPT-5.2 Codex",
            description: "Frontier agentic coding model.",
            claude_alias: None,
            claude_picker_model: Some("claude-sonnet-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.2",
            display_name: "GPT-5.2",
            description: "Optimized for professional work and long-running agents.",
            claude_alias: None,
            claude_picker_model: Some("claude-opus-4-5"),
            supports_xhigh: true,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.1-codex-max",
            display_name: "GPT-5.1 Codex Max",
            description: "Codex-optimized model for deep and fast reasoning.",
            claude_alias: None,
            claude_picker_model: Some("claude-opus-4-1"),
            supports_xhigh: false,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5.1-codex-mini",
            display_name: "GPT-5.1 Codex Mini",
            description: "Optimized for Codex. Cheaper, faster, but less capable.",
            claude_alias: None,
            claude_picker_model: Some("claude-haiku-4"),
            supports_xhigh: false,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5",
            display_name: "GPT-5",
            description: "General-purpose GPT-5 model.",
            claude_alias: None,
            claude_picker_model: Some("claude-sonnet-4"),
            supports_xhigh: false,
        },
        RuntimeProxyResponsesModelDescriptor {
            id: "gpt-5-mini",
            display_name: "GPT-5 Mini",
            description: "Smaller GPT-5 model for fast, lower-cost tasks.",
            claude_alias: None,
            claude_picker_model: Some("claude-haiku-3-5"),
            supports_xhigh: false,
        },
    ]
}
pub(super) fn runtime_proxy_claude_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_proxy_request_header_value(&request.headers, "x-claude-code-session-id")
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "session_id"))
        .map(str::to_string)
}

pub(super) fn runtime_proxy_claude_target_model(requested_model: &str) -> String {
    if let Some(override_model) = runtime_proxy_claude_model_override() {
        return override_model;
    }

    let normalized = requested_model.trim();
    if let Some(descriptor) = runtime_proxy_responses_model_descriptor(normalized) {
        return descriptor.id.to_string();
    }
    if let Some(descriptor) = runtime_proxy_claude_picker_model_descriptor(normalized) {
        return descriptor.id.to_string();
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("gpt-")
        || lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("codex")
    {
        normalized.to_string()
    } else if lower == "best" || lower == "default" || lower.contains("opus") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Opus)
            .id
            .to_string()
    } else if lower.contains("sonnet") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Sonnet)
            .id
            .to_string()
    } else if lower.contains("haiku") {
        runtime_proxy_claude_alias_model(RuntimeProxyClaudeModelAlias::Haiku)
            .id
            .to_string()
    } else {
        DEFAULT_PRODEX_CLAUDE_MODEL.to_string()
    }
}

pub(super) fn runtime_proxy_responses_model_supports_native_computer_tool(model_id: &str) -> bool {
    model_id.trim().eq_ignore_ascii_case("gpt-5.4")
}
