use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::toml_helpers::ensure_child_table;
use crate::{PRODEX_CAVEMAN_HOOK_COMMAND, PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC};

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

pub(crate) fn configure_caveman_session_start_hook(table: &mut toml::Table) -> usize {
    configure_session_start_command_hook(table, PRODEX_CAVEMAN_HOOK_COMMAND)
}

pub(crate) fn configure_trusted_session_start_command_hook(
    table: &mut toml::Table,
    config_path: &Path,
    command: &str,
) -> Result<usize> {
    let group_index = configure_session_start_command_hook(table, command);
    configure_session_start_command_hook_trust_state(table, config_path, group_index, command)?;
    Ok(group_index)
}

fn configure_session_start_command_hook(table: &mut toml::Table, command: &str) -> usize {
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
    if let Some(group_index) =
        session_start_command_hook_group_index(session_start_groups.as_slice(), command)
    {
        return group_index;
    }

    let group_index = session_start_groups.len();

    let mut command_hook = toml::Table::new();
    command_hook.insert(
        "type".to_string(),
        toml::Value::String("command".to_string()),
    );
    command_hook.insert(
        "command".to_string(),
        toml::Value::String(command.to_string()),
    );

    let mut group = toml::Table::new();
    group.insert(
        "hooks".to_string(),
        toml::Value::Array(vec![toml::Value::Table(command_hook)]),
    );
    session_start_groups.push(toml::Value::Table(group));
    group_index
}

fn session_start_command_hook_group_index(
    session_start_groups: &[toml::Value],
    command: &str,
) -> Option<usize> {
    session_start_groups
        .iter()
        .enumerate()
        .find_map(|(group_index, group)| {
            let contains_command = group
                .get("hooks")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .any(|hook| hook.get("command").and_then(toml::Value::as_str) == Some(command));
            contains_command.then_some(group_index)
        })
}

pub(crate) fn configure_caveman_hook_trust_state(
    table: &mut toml::Table,
    config_path: &Path,
    group_index: usize,
) -> Result<()> {
    configure_session_start_command_hook_trust_state(
        table,
        config_path,
        group_index,
        PRODEX_CAVEMAN_HOOK_COMMAND,
    )
}

fn configure_session_start_command_hook_trust_state(
    table: &mut toml::Table,
    config_path: &Path,
    group_index: usize,
    command: &str,
) -> Result<()> {
    let hook_key = caveman_hook_trust_key(config_path, group_index, 0);
    let trusted_hash = session_start_command_hook_hash(command)?;
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

fn session_start_command_hook_hash(command: &str) -> Result<String> {
    // Codex 0.129 hashes the normalized hook identity, including default timeout.
    let group = CavemanHookMatcherGroup {
        matcher: None,
        hooks: vec![CavemanHookHandlerConfig::Command {
            command: command.to_string(),
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
