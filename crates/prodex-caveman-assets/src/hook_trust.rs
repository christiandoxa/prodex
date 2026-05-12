use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::toml_helpers::ensure_child_table;
use crate::{CLAUDE_MEM_PLUGIN_NAME, PRODEX_CAVEMAN_HOOK_COMMAND, PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC};

#[derive(Serialize)]
struct NormalizedHookIdentity {
    event_name: &'static str,
    #[serde(flatten)]
    group: CavemanHookMatcherGroup,
}

#[derive(Clone, Deserialize, Serialize)]
struct CavemanHookMatcherGroup {
    #[serde(default)]
    matcher: Option<String>,
    #[serde(default)]
    hooks: Vec<CavemanHookHandlerConfig>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
enum CavemanHookHandlerConfig {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default, rename = "timeout")]
        timeout_sec: Option<u64>,
        #[serde(default)]
        r#async: bool,
        #[serde(default, rename = "statusMessage")]
        status_message: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct CodexPluginManifest {
    #[serde(default)]
    hooks: Option<String>,
}

#[derive(Deserialize)]
struct CodexPluginHooksFile {
    #[serde(default)]
    hooks: BTreeMap<String, Vec<CavemanHookMatcherGroup>>,
}

pub fn trust_claude_mem_codex_plugin_hooks(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    if !config_path.is_file() {
        return Ok(());
    }
    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let mut table = if contents.trim().is_empty() {
        toml::Table::new()
    } else {
        match toml::from_str::<toml::Value>(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
        {
            toml::Value::Table(table) => table,
            _ => bail!("{} did not parse as a TOML table", config_path.display()),
        }
    };

    let plugin_ids = claude_mem_codex_plugin_ids(&table);
    if plugin_ids.is_empty() {
        return Ok(());
    }

    let mut changed = false;
    for plugin_id in plugin_ids {
        changed |= trust_claude_mem_codex_plugin_hooks_for_plugin(
            &mut table,
            codex_home,
            plugin_id.as_str(),
        )?;
    }
    if changed {
        let rendered = toml::to_string(&toml::Value::Table(table))
            .context("failed to render Claude-Mem hook trust overlay")?;
        fs::write(&config_path, rendered)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }
    Ok(())
}

pub(crate) fn configure_caveman_session_start_hook(table: &mut toml::Table) -> usize {
    let hooks = ensure_child_table(table, "hooks");
    let session_start = hooks
        .entry("SessionStart".to_string())
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if !session_start.is_array() {
        *session_start = toml::Value::Array(Vec::new());
    }
    let Some(session_start_groups) = session_start.as_array_mut() else {
        unreachable!("SessionStart should be an array after insertion");
    };
    let group_index = session_start_groups.len();

    let mut command_hook = toml::Table::new();
    command_hook.insert(
        "type".to_string(),
        toml::Value::String("command".to_string()),
    );
    command_hook.insert(
        "command".to_string(),
        toml::Value::String(PRODEX_CAVEMAN_HOOK_COMMAND.to_string()),
    );

    let mut group = toml::Table::new();
    group.insert(
        "hooks".to_string(),
        toml::Value::Array(vec![toml::Value::Table(command_hook)]),
    );
    session_start_groups.push(toml::Value::Table(group));
    group_index
}

pub(crate) fn configure_caveman_hook_trust_state(
    table: &mut toml::Table,
    config_path: &Path,
    group_index: usize,
) -> Result<()> {
    let hook_key = caveman_hook_trust_key(config_path, group_index, 0);
    let trusted_hash = caveman_session_start_hook_hash()?;
    let hooks = ensure_child_table(table, "hooks");
    let state = ensure_child_table(hooks, "state");
    let hook_state = ensure_child_table(state, &hook_key);
    hook_state.insert(
        "trusted_hash".to_string(),
        toml::Value::String(trusted_hash),
    );
    Ok(())
}

fn caveman_hook_trust_key(config_path: &Path, group_index: usize, hook_index: usize) -> String {
    format!(
        "{}:session_start:{group_index}:{hook_index}",
        config_path.display()
    )
}

fn caveman_session_start_hook_hash() -> Result<String> {
    // Codex 0.129 hashes the normalized hook identity, including default timeout.
    let group = CavemanHookMatcherGroup {
        matcher: None,
        hooks: vec![CavemanHookHandlerConfig::Command {
            command: PRODEX_CAVEMAN_HOOK_COMMAND.to_string(),
            timeout_sec: Some(PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC),
            r#async: false,
            status_message: None,
        }],
    };
    let identity = NormalizedHookIdentity {
        event_name: "session_start",
        group,
    };
    let value = toml::Value::try_from(identity)
        .context("failed to serialize Caveman hook trust identity")?;
    Ok(version_for_toml(&value))
}

fn claude_mem_codex_plugin_ids(table: &toml::Table) -> Vec<String> {
    let Some(toml::Value::Table(plugins)) = table.get("plugins") else {
        return Vec::new();
    };
    plugins
        .iter()
        .filter_map(|(plugin_id, config)| {
            let (name, _) = plugin_id.split_once('@')?;
            if name != CLAUDE_MEM_PLUGIN_NAME {
                return None;
            }
            let explicitly_disabled = config
                .as_table()
                .and_then(|table| table.get("enabled"))
                .and_then(toml::Value::as_bool)
                == Some(false);
            (!explicitly_disabled).then(|| plugin_id.clone())
        })
        .collect()
}

fn trust_claude_mem_codex_plugin_hooks_for_plugin(
    table: &mut toml::Table,
    codex_home: &Path,
    plugin_id: &str,
) -> Result<bool> {
    let Some(plugin_root) = latest_claude_mem_plugin_cache_dir(codex_home, plugin_id) else {
        return Ok(false);
    };
    let Some(hooks_relative_path) = codex_plugin_hook_relative_path(&plugin_root)? else {
        return Ok(false);
    };
    let hooks_path = plugin_root.join(Path::new(&hooks_relative_path));
    if !hooks_path.is_file() {
        return Ok(false);
    }
    let Ok(contents) = fs::read_to_string(&hooks_path) else {
        return Ok(false);
    };
    let Ok(hooks_file) = serde_json::from_str::<CodexPluginHooksFile>(&contents) else {
        return Ok(false);
    };
    trust_codex_plugin_command_hooks(table, plugin_id, &hooks_relative_path, &hooks_file)
}

fn latest_claude_mem_plugin_cache_dir(codex_home: &Path, plugin_id: &str) -> Option<PathBuf> {
    let marketplace = plugin_key_marketplace(plugin_id)?;
    let plugin_cache_root = codex_home
        .join("plugins/cache")
        .join(marketplace)
        .join(CLAUDE_MEM_PLUGIN_NAME);
    let mut versions = fs::read_dir(plugin_cache_root)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry.file_type().ok()?.is_dir().then(|| entry.path())
        })
        .collect::<Vec<_>>();
    versions.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    versions.pop()
}

fn plugin_key_marketplace(plugin_id: &str) -> Option<&str> {
    let (name, marketplace) = plugin_id.split_once('@')?;
    (name == CLAUDE_MEM_PLUGIN_NAME && !marketplace.is_empty()).then_some(marketplace)
}

fn codex_plugin_hook_relative_path(plugin_root: &Path) -> Result<Option<String>> {
    let manifest_path = plugin_root.join(".codex-plugin/plugin.json");
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let Ok(contents) = fs::read_to_string(&manifest_path) else {
        return Ok(None);
    };
    let Ok(manifest) = serde_json::from_str::<CodexPluginManifest>(&contents) else {
        return Ok(None);
    };
    let Some(raw_hooks_path) = manifest.hooks.as_deref() else {
        return Ok(None);
    };
    Ok(clean_plugin_relative_path(raw_hooks_path))
}

fn clean_plugin_relative_path(raw_path: &str) -> Option<String> {
    let path = Path::new(raw_path.trim());
    if path.is_absolute() {
        return None;
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => clean.push(part),
            _ => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then(|| clean.to_string_lossy().replace('\\', "/"))
}

fn trust_codex_plugin_command_hooks(
    table: &mut toml::Table,
    plugin_id: &str,
    hooks_relative_path: &str,
    hooks_file: &CodexPluginHooksFile,
) -> Result<bool> {
    let mut changed = false;
    for (event_name, groups) in &hooks_file.hooks {
        let Some(event_label) = hook_event_key_label_from_config_name(event_name) else {
            continue;
        };
        for (group_index, group) in groups.iter().enumerate() {
            for (handler_index, handler) in group.hooks.iter().enumerate() {
                let Some(normalized_handler) = normalized_command_hook_for_trust(handler) else {
                    continue;
                };
                let trusted_hash = command_hook_hash(event_label, group, normalized_handler)?;
                let hook_key = codex_plugin_hook_trust_key(
                    plugin_id,
                    hooks_relative_path,
                    event_label,
                    group_index,
                    handler_index,
                );
                changed |= set_trusted_hook_hash(table, &hook_key, trusted_hash);
            }
        }
    }
    Ok(changed)
}

fn hook_event_key_label_from_config_name(event_name: &str) -> Option<&'static str> {
    match event_name {
        "Notification" => Some("notification"),
        "PermissionRequest" => Some("permission_request"),
        "PostCompact" => Some("post_compact"),
        "PostToolUse" => Some("post_tool_use"),
        "PreCompact" => Some("pre_compact"),
        "PreToolUse" => Some("pre_tool_use"),
        "SessionStart" => Some("session_start"),
        "Stop" => Some("stop"),
        "SubagentStop" => Some("subagent_stop"),
        "UserPromptSubmit" => Some("user_prompt_submit"),
        _ => None,
    }
}

fn hook_event_uses_matcher(event_label: &str) -> bool {
    matches!(
        event_label,
        "permission_request"
            | "post_compact"
            | "post_tool_use"
            | "pre_compact"
            | "pre_tool_use"
            | "session_start"
    )
}

fn normalized_command_hook_for_trust(
    handler: &CavemanHookHandlerConfig,
) -> Option<CavemanHookHandlerConfig> {
    match handler {
        CavemanHookHandlerConfig::Command {
            command,
            timeout_sec,
            r#async,
            status_message,
        } if !*r#async => Some(CavemanHookHandlerConfig::Command {
            command: command.clone(),
            timeout_sec: Some(
                timeout_sec
                    .unwrap_or(PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC)
                    .max(1),
            ),
            r#async: false,
            status_message: status_message.clone(),
        }),
        _ => None,
    }
}

fn command_hook_hash(
    event_label: &'static str,
    group: &CavemanHookMatcherGroup,
    handler: CavemanHookHandlerConfig,
) -> Result<String> {
    let identity = NormalizedHookIdentity {
        event_name: event_label,
        group: CavemanHookMatcherGroup {
            matcher: hook_event_uses_matcher(event_label)
                .then(|| group.matcher.clone())
                .flatten(),
            hooks: vec![handler],
        },
    };
    let value = toml::Value::try_from(identity)
        .context("failed to serialize Codex plugin hook trust identity")?;
    Ok(version_for_toml(&value))
}

fn codex_plugin_hook_trust_key(
    plugin_id: &str,
    hooks_relative_path: &str,
    event_label: &str,
    group_index: usize,
    handler_index: usize,
) -> String {
    format!("{plugin_id}:{hooks_relative_path}:{event_label}:{group_index}:{handler_index}")
}

fn set_trusted_hook_hash(table: &mut toml::Table, hook_key: &str, trusted_hash: String) -> bool {
    let hooks = ensure_child_table(table, "hooks");
    let state = ensure_child_table(hooks, "state");
    let hook_state = ensure_child_table(state, hook_key);
    let value = toml::Value::String(trusted_hash);
    if hook_state.get("trusted_hash") == Some(&value) {
        return false;
    }
    hook_state.insert("trusted_hash".to_string(), value);
    true
}

fn version_for_toml(value: &toml::Value) -> String {
    let json = serde_json::to_value(value).unwrap_or(JsonValue::Null);
    let canonical = canonical_json(&json);
    let serialized = serde_json::to_vec(&canonical).unwrap_or_default();
    let hash = Sha256::digest(serialized);
    let hex = hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn canonical_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(value) = map.get(&key) {
                    sorted.insert(key, canonical_json(value));
                }
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}
