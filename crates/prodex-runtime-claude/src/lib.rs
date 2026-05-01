use anyhow::{Context, Result};
use dirs::home_dir;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const PRODEX_CLAUDE_PROXY_API_KEY: &str = "prodex-runtime-proxy";
pub const PRODEX_CLAUDE_CONFIG_DIR_NAME: &str = ".claude-code";
pub const PRODEX_SHARED_CLAUDE_DIR_NAME: &str = "claude";
pub const DEFAULT_CLAUDE_CONFIG_DIR_NAME: &str = ".claude";
pub const DEFAULT_CLAUDE_CONFIG_FILE_NAME: &str = ".claude.json";
pub const DEFAULT_CLAUDE_SETTINGS_FILE_NAME: &str = "settings.json";
pub const PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME: &str = ".prodex-legacy-imported";
pub const PRODEX_CLAUDE_DEFAULT_WEB_TOOLS: &[&str] = &["WebSearch", "WebFetch"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeProxyClaudeLaunchModes {
    pub caveman_mode: bool,
    pub mem_mode: bool,
}

pub fn runtime_proxy_claude_extract_launch_modes(
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

pub fn runtime_proxy_claude_launch_args(
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

pub fn runtime_proxy_claude_launch_model(codex_home: &Path) -> String {
    runtime_anthropic_crate::runtime_proxy_claude_model_override()
        .or_else(|| runtime_proxy_claude_config_value(codex_home, "model"))
        .unwrap_or_else(|| runtime_anthropic_crate::DEFAULT_PRODEX_CLAUDE_MODEL.to_string())
}

pub fn runtime_proxy_claude_launch_env(
    listen_addr: SocketAddr,
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
            OsString::from(runtime_anthropic_crate::runtime_proxy_claude_picker_model(
                &target_model,
            )),
        ),
    ];
    if let Some(plugin_root) = mem_plugin_root {
        env.push((
            "CLAUDE_PLUGIN_ROOT",
            OsString::from(plugin_root.as_os_str()),
        ));
    }
    if runtime_anthropic_crate::runtime_proxy_claude_use_foundry_compat() {
        env.push(("CLAUDE_CODE_USE_FOUNDRY", OsString::from("1")));
        env.push(("ANTHROPIC_FOUNDRY_BASE_URL", OsString::from(base_url)));
        env.push((
            "ANTHROPIC_FOUNDRY_API_KEY",
            OsString::from(PRODEX_CLAUDE_PROXY_API_KEY),
        ));
    }
    env.extend(prodex_runtime_launch::local_proxy_bypass_env());
    env.extend(runtime_anthropic_crate::runtime_proxy_claude_pinned_alias_env());
    env.extend(
        runtime_anthropic_crate::runtime_proxy_claude_custom_model_option_env(&target_model),
    );
    env
}

pub fn runtime_proxy_claude_removed_env() -> &'static [&'static str] {
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

pub fn runtime_proxy_claude_config_value(codex_home: &Path, key: &str) -> Option<String> {
    codex_config::codex_config_value(codex_home, key)
}

pub fn runtime_proxy_claude_config_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(PRODEX_CLAUDE_CONFIG_DIR_NAME)
}

pub fn runtime_proxy_shared_claude_config_dir(prodex_root: &Path) -> PathBuf {
    prodex_root.join(PRODEX_SHARED_CLAUDE_DIR_NAME)
}

pub fn runtime_proxy_claude_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_CONFIG_FILE_NAME)
}

pub fn runtime_proxy_claude_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_SETTINGS_FILE_NAME)
}

pub fn runtime_proxy_claude_legacy_import_marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME)
}

pub fn legacy_default_claude_config_dir() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_DIR_NAME))
}

pub fn legacy_default_claude_config_path() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_FILE_NAME))
}

pub fn runtime_proxy_claude_binary_version(binary: &OsString) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stdout)).or_else(
        || parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stderr)),
    )
}

pub fn parse_runtime_proxy_claude_version_text(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}

pub fn ensure_runtime_proxy_claude_launch_config(
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
    let mut additional_model_options =
        runtime_anthropic_crate::runtime_proxy_claude_additional_model_option_entries();
    if let Some(existing) = object
        .get("additionalModelOptionsCache")
        .and_then(serde_json::Value::as_array)
    {
        for entry in existing {
            let existing_value = entry.get("value").and_then(serde_json::Value::as_str);
            if existing_value.is_some_and(
                runtime_anthropic_crate::runtime_proxy_claude_managed_model_option_value,
            ) {
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

pub fn ensure_runtime_proxy_claude_settings(config_dir: &Path) -> Result<()> {
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

pub fn prepare_runtime_proxy_claude_config_dir(
    prodex_root: &Path,
    codex_home: &Path,
    managed: bool,
) -> Result<PathBuf> {
    let profile_dir = runtime_proxy_claude_config_dir(codex_home);
    if !managed {
        prepare_runtime_proxy_claude_import_target(&profile_dir)?;
        return Ok(profile_dir);
    }

    let shared_dir = runtime_proxy_shared_claude_config_dir(prodex_root);
    prepare_runtime_proxy_claude_import_target(&shared_dir)?;
    migrate_runtime_proxy_claude_profile_dir_to_target(&profile_dir, &shared_dir)?;
    ensure_runtime_proxy_claude_profile_link(&profile_dir, &shared_dir)?;
    Ok(profile_dir)
}

pub fn prepare_runtime_proxy_claude_import_target(target_dir: &Path) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(target_dir)?;
    maybe_import_runtime_proxy_claude_legacy_home(target_dir)
}

pub fn maybe_import_runtime_proxy_claude_legacy_home(target_dir: &Path) -> Result<()> {
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

pub fn migrate_runtime_proxy_claude_profile_dir_to_target(
    profile_dir: &Path,
    target_dir: &Path,
) -> Result<()> {
    if prodex_core::same_path(profile_dir, target_dir) {
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
        if !source_dir.exists() || prodex_core::same_path(&source_dir, target_dir) {
            return Ok(());
        }
        if !source_dir.is_dir() {
            anyhow::bail!(
                "expected {} to point to a Claude config directory",
                profile_dir.display()
            );
        }
        merge_runtime_proxy_claude_directory_contents(&source_dir, target_dir)?;
        runtime_proxy_remove_path(profile_dir)?;
        return Ok(());
    }

    if !metadata.is_dir() {
        anyhow::bail!(
            "expected {} to be a Claude config directory",
            profile_dir.display()
        );
    }

    merge_runtime_proxy_claude_directory_contents(profile_dir, target_dir)?;
    fs::remove_dir_all(profile_dir)
        .with_context(|| format!("failed to remove {}", profile_dir.display()))?;
    Ok(())
}

pub fn ensure_runtime_proxy_claude_profile_link(link_path: &Path, target_dir: &Path) -> Result<()> {
    if prodex_core::same_path(link_path, target_dir) {
        return Ok(());
    }

    match fs::symlink_metadata(link_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let existing_target = runtime_proxy_resolve_symlink_target(link_path)?;
                if prodex_core::same_path(&existing_target, target_dir) {
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

pub fn merge_runtime_proxy_claude_directory_contents(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    if prodex_core::same_path(source, destination) {
        return Ok(());
    }
    prodex_shared_codex_fs::create_codex_home_if_missing(destination)?;

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

pub fn merge_runtime_proxy_claude_file(source: &Path, destination: &Path) -> Result<()> {
    if prodex_core::same_path(source, destination) {
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
        anyhow::bail!(
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

pub fn merge_runtime_proxy_claude_json_file(source: &Path, destination: &Path) -> Result<()> {
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

pub fn runtime_proxy_merge_json_defaults(
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

pub fn merge_runtime_proxy_claude_jsonl_file(source: &Path, destination: &Path) -> Result<()> {
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

pub fn merge_runtime_proxy_claude_symlink(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() || fs::symlink_metadata(destination).is_ok() {
        return Ok(());
    }

    let target = fs::read_link(source)
        .with_context(|| format!("failed to read symlink {}", source.display()))?;
    runtime_proxy_create_symlink(&target, destination, true)
}

pub fn runtime_proxy_resolve_symlink_target(path: &Path) -> Result<PathBuf> {
    let target = fs::read_link(path)
        .with_context(|| format!("failed to read symlink {}", path.display()))?;
    Ok(if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(target)
    })
}

pub fn runtime_proxy_remove_path(path: &Path) -> Result<()> {
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

pub fn runtime_proxy_create_directory_symlink(target: &Path, link: &Path) -> Result<()> {
    runtime_proxy_create_symlink(target, link, true)
}

pub fn runtime_proxy_create_symlink(target: &Path, link: &Path, is_dir: bool) -> Result<()> {
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
        anyhow::bail!("Claude state links are not supported on this platform");
    }

    Ok(())
}
