use super::*;

const CLAUDE_MEM_DATA_DIR_NAME: &str = ".claude-mem";
const CLAUDE_MEM_SETTINGS_FILE_NAME: &str = "settings.json";
const CLAUDE_MEM_TRANSCRIPT_WATCH_FILE_NAME: &str = "transcript-watch.json";
const CLAUDE_MEM_TRANSCRIPT_WATCH_STATE_FILE_NAME: &str = "transcript-watch-state.json";
const CLAUDE_MEM_PLUGIN_MARKETPLACE_OWNER: &str = "thedotmack";
const CLAUDE_MEM_CODEX_SCHEMA_NAME: &str = "codex";
const CLAUDE_MEM_PRODEX_WATCH_NAME_PREFIX: &str = "prodex-codex-";
const CLAUDE_MEM_CLAUDE_CODE_PATH_SETTING: &str = "CLAUDE_CODE_PATH";
const PRODEX_CLAUDE_MEM_DIR_NAME: &str = "claude-mem";
const PRODEX_CLAUDE_MEM_WRAPPER_NAME: &str = "prodex-claude";

pub(super) fn runtime_mem_extract_mode(args: &[OsString]) -> (bool, Vec<OsString>) {
    let Some(first) = args.first().and_then(|arg| arg.to_str()) else {
        return (false, args.to_vec());
    };
    if first != "mem" {
        return (false, args.to_vec());
    }
    (true, args[1..].to_vec())
}

pub(super) fn runtime_mem_claude_plugin_dir() -> Result<PathBuf> {
    let home = home_dir().context("failed to determine home directory for claude-mem")?;
    let plugin_dir = runtime_mem_claude_plugin_dir_from_home(&home);
    let manifest_path = runtime_mem_claude_plugin_manifest_path(&plugin_dir);
    if !manifest_path.is_file() {
        bail!(
            "claude-mem is not installed for Claude Code; run `npx claude-mem install --ide claude-code` first"
        );
    }
    Ok(plugin_dir)
}

pub(super) fn ensure_runtime_mem_codex_watch_for_home(codex_home: &Path) -> Result<()> {
    let home = home_dir().context("failed to determine home directory for claude-mem")?;
    let config_path = runtime_mem_transcript_watch_config_path_from_home(&home);
    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, codex_home)
}

pub(super) fn ensure_runtime_mem_prodex_observer(paths: &AppPaths) -> Result<PathBuf> {
    let home = home_dir().context("failed to determine home directory for claude-mem")?;
    let prodex_exe = env::current_exe().context("failed to determine current prodex executable")?;
    ensure_runtime_mem_prodex_observer_for_home(&home, paths, &prodex_exe)
}

pub(super) fn ensure_runtime_mem_prodex_observer_for_home(
    home: &Path,
    paths: &AppPaths,
    prodex_exe: &Path,
) -> Result<PathBuf> {
    let wrapper_path = runtime_mem_prodex_claude_wrapper_path(paths);
    write_runtime_mem_prodex_claude_wrapper(&wrapper_path, prodex_exe)?;
    let settings_path = runtime_mem_settings_path_from_home(home);
    update_runtime_mem_claude_code_path_setting(&settings_path, &wrapper_path)?;
    Ok(wrapper_path)
}

pub(super) fn runtime_mem_claude_plugin_dir_from_home(home: &Path) -> PathBuf {
    home.join(DEFAULT_CLAUDE_CONFIG_DIR_NAME)
        .join("plugins")
        .join("marketplaces")
        .join(CLAUDE_MEM_PLUGIN_MARKETPLACE_OWNER)
        .join("plugin")
}

pub(super) fn runtime_mem_claude_plugin_manifest_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(".claude-plugin").join("plugin.json")
}

pub(super) fn runtime_mem_data_dir_from_home(home: &Path) -> PathBuf {
    home.join(CLAUDE_MEM_DATA_DIR_NAME)
}

pub(super) fn runtime_mem_settings_path_from_home(home: &Path) -> PathBuf {
    runtime_mem_data_dir_from_home(home).join(CLAUDE_MEM_SETTINGS_FILE_NAME)
}

pub(super) fn runtime_mem_transcript_watch_config_path_from_home(home: &Path) -> PathBuf {
    let settings_path = runtime_mem_settings_path_from_home(home);
    let default_path =
        runtime_mem_data_dir_from_home(home).join(CLAUDE_MEM_TRANSCRIPT_WATCH_FILE_NAME);
    let Some(raw) = fs::read_to_string(&settings_path).ok() else {
        return default_path;
    };
    let Some(settings) = serde_json::from_str::<serde_json::Value>(&raw).ok() else {
        return default_path;
    };
    let flat = settings
        .get("CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH")
        .and_then(serde_json::Value::as_str);
    let nested = settings
        .get("env")
        .and_then(serde_json::Value::as_object)
        .and_then(|env| env.get("CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH"))
        .and_then(serde_json::Value::as_str);
    flat.or(nested).map(PathBuf::from).unwrap_or(default_path)
}

pub(super) fn ensure_runtime_mem_codex_watch_for_home_at_path(
    config_path: &Path,
    codex_home: &Path,
) -> Result<()> {
    let sessions_root = runtime_mem_codex_sessions_root(codex_home);
    ensure_runtime_mem_codex_watch_for_sessions_root(config_path, &sessions_root)
}

pub(super) fn ensure_runtime_mem_codex_watch_for_sessions_root(
    config_path: &Path,
    sessions_root: &Path,
) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw = fs::read_to_string(config_path).ok();
    let mut config = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !config.is_object() {
        config = serde_json::json!({});
    }

    let object = config
        .as_object_mut()
        .expect("transcript watch config should be normalized to an object");
    object.insert("version".to_string(), serde_json::json!(1));
    if !object
        .get("stateFile")
        .is_some_and(serde_json::Value::is_string)
    {
        let state_file = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(CLAUDE_MEM_TRANSCRIPT_WATCH_STATE_FILE_NAME);
        object.insert(
            "stateFile".to_string(),
            serde_json::json!(state_file.display().to_string()),
        );
    }

    let schemas = object
        .entry("schemas".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !schemas.is_object() {
        *schemas = serde_json::json!({});
    }
    let schemas = schemas
        .as_object_mut()
        .expect("transcript watch schemas should be an object");
    schemas
        .entry(CLAUDE_MEM_CODEX_SCHEMA_NAME.to_string())
        .or_insert_with(runtime_mem_default_codex_schema);

    let watch_glob = runtime_mem_codex_watch_glob(sessions_root);
    let watch_name = runtime_mem_prodex_watch_name(sessions_root);
    let desired_watch = runtime_mem_codex_watch_definition(&watch_name, &watch_glob);

    let watches = object
        .entry("watches".to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !watches.is_array() {
        *watches = serde_json::json!([]);
    }
    let watches = watches
        .as_array_mut()
        .expect("transcript watches should be an array");

    if watches.iter().any(|watch| {
        watch.get("schema").and_then(serde_json::Value::as_str)
            == Some(CLAUDE_MEM_CODEX_SCHEMA_NAME)
            && watch.get("path").and_then(serde_json::Value::as_str) == Some(watch_glob.as_str())
    }) {
        let rendered = serde_json::to_string_pretty(&config)
            .context("failed to render claude-mem transcript watch config")?;
        fs::write(config_path, format!("{rendered}\n"))
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        return Ok(());
    }

    if let Some(existing) = watches.iter_mut().find(|watch| {
        watch.get("name").and_then(serde_json::Value::as_str) == Some(watch_name.as_str())
    }) {
        *existing = desired_watch;
    } else {
        watches.push(desired_watch);
    }

    let rendered = serde_json::to_string_pretty(&config)
        .context("failed to render claude-mem transcript watch config")?;
    fs::write(config_path, format!("{rendered}\n"))
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

pub(super) fn runtime_mem_prodex_claude_wrapper_path(paths: &AppPaths) -> PathBuf {
    let file_name = if cfg!(windows) {
        format!("{PRODEX_CLAUDE_MEM_WRAPPER_NAME}.cmd")
    } else {
        PRODEX_CLAUDE_MEM_WRAPPER_NAME.to_string()
    };
    paths.root.join(PRODEX_CLAUDE_MEM_DIR_NAME).join(file_name)
}

fn write_runtime_mem_prodex_claude_wrapper(wrapper_path: &Path, prodex_exe: &Path) -> Result<()> {
    if let Some(parent) = wrapper_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let contents = if cfg!(windows) {
        format!(
            "@echo off\r\n\"{}\" claude --skip-quota-check -- %*\r\n",
            prodex_exe.display()
        )
    } else {
        format!(
            "#!/bin/sh\nexec {} claude --skip-quota-check -- \"$@\"\n",
            shell_single_quote_path(prodex_exe)
        )
    };
    fs::write(wrapper_path, contents)
        .with_context(|| format!("failed to write {}", wrapper_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(wrapper_path)
            .with_context(|| format!("failed to inspect {}", wrapper_path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(wrapper_path, permissions)
            .with_context(|| format!("failed to chmod {}", wrapper_path.display()))?;
    }
    Ok(())
}

fn update_runtime_mem_claude_code_path_setting(
    settings_path: &Path,
    wrapper_path: &Path,
) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw = fs::read_to_string(settings_path).ok();
    let mut settings = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    settings
        .as_object_mut()
        .expect("claude-mem settings should be normalized to an object")
        .insert(
            CLAUDE_MEM_CLAUDE_CODE_PATH_SETTING.to_string(),
            serde_json::json!(wrapper_path.display().to_string()),
        );

    let rendered =
        serde_json::to_string_pretty(&settings).context("failed to render claude-mem settings")?;
    fs::write(settings_path, format!("{rendered}\n"))
        .with_context(|| format!("failed to write {}", settings_path.display()))?;
    Ok(())
}

fn shell_single_quote_path(path: &Path) -> String {
    let raw = path.display().to_string();
    format!("'{}'", raw.replace('\'', "'\"'\"'"))
}

fn runtime_mem_codex_sessions_root(codex_home: &Path) -> PathBuf {
    let sessions_root = codex_home.join("sessions");
    fs::canonicalize(&sessions_root).unwrap_or(sessions_root)
}

fn runtime_mem_prodex_watch_name(sessions_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    sessions_root.hash(&mut hasher);
    format!(
        "{CLAUDE_MEM_PRODEX_WATCH_NAME_PREFIX}{:016x}",
        hasher.finish()
    )
}

fn runtime_mem_codex_watch_glob(sessions_root: &Path) -> String {
    let mut root = sessions_root.display().to_string();
    while root.ends_with(std::path::MAIN_SEPARATOR) {
        root.pop();
    }
    let sep = std::path::MAIN_SEPARATOR;
    format!("{root}{sep}**{sep}*.jsonl")
}

fn runtime_mem_codex_watch_definition(name: &str, path: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "path": path,
        "schema": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "startAtEnd": true,
        "context": {
            "mode": "agents",
            "updateOn": ["session_start", "session_end"],
        }
    })
}

pub(super) fn runtime_mem_default_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.3",
        "description": "Schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            {
                "name": "session-meta",
                "match": { "path": "type", "equals": "session_meta" },
                "action": "session_context",
                "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" }
            },
            {
                "name": "turn-context",
                "match": { "path": "type", "equals": "turn_context" },
                "action": "session_context",
                "fields": { "cwd": "payload.cwd" }
            },
            {
                "name": "user-message",
                "match": { "path": "payload.type", "equals": "user_message" },
                "action": "session_init",
                "fields": { "prompt": "payload.message" }
            },
            {
                "name": "assistant-message",
                "match": { "path": "payload.type", "equals": "agent_message" },
                "action": "assistant_message",
                "fields": { "message": "payload.message" }
            },
            {
                "name": "tool-use",
                "match": {
                    "path": "payload.type",
                    "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command"]
                },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": {
                        "coalesce": ["payload.name", "payload.type", { "value": "web_search" }]
                    },
                    "toolInput": {
                        "coalesce": ["payload.arguments", "payload.input", "payload.command", "payload.action"]
                    }
                }
            },
            {
                "name": "tool-result",
                "match": {
                    "path": "payload.type",
                    "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"]
                },
                "action": "tool_result",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolResponse": "payload.output"
                }
            },
            {
                "name": "session-end",
                "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed"] },
                "action": "session_end"
            }
        ]
    })
}
