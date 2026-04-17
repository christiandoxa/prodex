use super::*;

pub(crate) fn parse_toml_string_assignment(contents: &str, key: &str) -> Option<String> {
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

pub(crate) fn runtime_proxy_claude_config_value(codex_home: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    parse_toml_string_assignment(&contents, key).filter(|value| !value.trim().is_empty())
}

pub(crate) fn runtime_proxy_claude_config_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(PRODEX_CLAUDE_CONFIG_DIR_NAME)
}

pub(crate) fn runtime_proxy_shared_claude_config_dir(paths: &AppPaths) -> PathBuf {
    paths.root.join(PRODEX_SHARED_CLAUDE_DIR_NAME)
}

pub(crate) fn runtime_proxy_claude_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_CONFIG_FILE_NAME)
}

pub(crate) fn runtime_proxy_claude_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_SETTINGS_FILE_NAME)
}

pub(crate) fn runtime_proxy_claude_legacy_import_marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME)
}

pub(crate) fn legacy_default_claude_config_dir() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_DIR_NAME))
}

pub(crate) fn legacy_default_claude_config_path() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_FILE_NAME))
}

pub(crate) fn runtime_proxy_claude_binary_version(binary: &OsString) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stdout)).or_else(
        || parse_runtime_proxy_claude_version_text(&String::from_utf8_lossy(&output.stderr)),
    )
}

pub(crate) fn parse_runtime_proxy_claude_version_text(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}

pub(crate) fn ensure_runtime_proxy_claude_launch_config(
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

pub(crate) fn ensure_runtime_proxy_claude_settings(config_dir: &Path) -> Result<()> {
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
