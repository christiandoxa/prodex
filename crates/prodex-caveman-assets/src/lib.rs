use anyhow::{Context, Result, bail};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod embedded_files;

use embedded_files::*;

pub const PRODEX_CAVEMAN_MARKETPLACE_NAME: &str = "prodex-caveman";
pub const PRODEX_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_CAVEMAN_PLUGIN_VERSION: &str = "0.1.0";
pub const PRODEX_CAVEMAN_PLUGIN_ID: &str = "caveman@prodex-caveman";
pub const PRODEX_CAVEMAN_SOURCE_REPO: &str = "https://github.com/JuliusBrussee/caveman.git";
pub const PRODEX_CAVEMAN_FULL_ASSETS_ENV: &str = "PRODEX_CAVEMAN_FULL_ASSETS";
pub const PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_RTK_SOURCE_REPO: &str = "https://github.com/rtk-ai/rtk.git";
const CLAUDE_MEM_PLUGIN_NAME: &str = "claude-mem";
const PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC: u64 = 600;
const PRODEX_CAVEMAN_HOOK_COMMAND: &str = "echo 'CAVEMAN MODE ACTIVE. $caveman full: terse, no filler, exact tech. Code/commits/security normal. Stop: stop caveman/normal mode.'";
const RTK_MD: &str = "RTK.md";
const AGENTS_MD: &str = "AGENTS.md";
const PRODEX_RTK_CODEX_AWARENESS: &str = r#"# RTK - Rust Token Killer (Codex CLI)

RTK is a token-optimized CLI proxy for shell commands.

## Rule

Prefix shell commands with `rtk` when RTK is available.

Examples:

```bash
rtk git status
rtk cargo test
rtk npm run build
rtk pytest -q
```

If `rtk` is not installed or `rtk gain` fails, run the command raw and tell the user RTK is unavailable.

## Meta Commands

```bash
rtk gain
rtk gain --history
rtk proxy <cmd>
```

## Verification

```bash
rtk --version
rtk gain
which rtk
```
"#;

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

pub fn install_caveman_marketplace(codex_home: &Path) -> Result<()> {
    let marketplace_root = caveman_marketplace_root(codex_home);
    let plugin_root = marketplace_root
        .join("plugins")
        .join(PRODEX_CAVEMAN_PLUGIN_NAME);
    fs::create_dir_all(marketplace_root.join(".agents/plugins")).with_context(|| {
        format!(
            "failed to create Caveman marketplace root {}",
            marketplace_root.display()
        )
    })?;
    write_caveman_plugin_tree(&plugin_root)?;
    let marketplace_manifest = serde_json::to_string_pretty(&serde_json::json!({
        "name": PRODEX_CAVEMAN_MARKETPLACE_NAME,
        "interface": {
            "displayName": "Prodex Caveman",
        },
        "plugins": [
            {
                "name": PRODEX_CAVEMAN_PLUGIN_NAME,
                "source": {
                    "source": "local",
                    "path": format!("./plugins/{PRODEX_CAVEMAN_PLUGIN_NAME}"),
                },
                "policy": {
                    "installation": "AVAILABLE",
                    "authentication": "ON_INSTALL",
                },
                "category": "Productivity",
            }
        ],
    }))
    .context("failed to serialize Caveman marketplace manifest")?;
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        marketplace_manifest,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            marketplace_root
                .join(".agents/plugins/marketplace.json")
                .display()
        )
    })?;
    Ok(())
}

pub fn install_caveman_plugin_cache(codex_home: &Path) -> Result<()> {
    let plugin_cache_base = codex_home
        .join("plugins/cache")
        .join(PRODEX_CAVEMAN_MARKETPLACE_NAME)
        .join(PRODEX_CAVEMAN_PLUGIN_NAME);
    if plugin_cache_base.exists() {
        fs::remove_dir_all(&plugin_cache_base)
            .with_context(|| format!("failed to clear {}", plugin_cache_base.display()))?;
    }
    write_caveman_plugin_tree(&plugin_cache_base.join(PRODEX_CAVEMAN_PLUGIN_VERSION))
}

pub fn caveman_marketplace_root(codex_home: &Path) -> PathBuf {
    codex_home
        .join(".tmp/marketplaces")
        .join(PRODEX_CAVEMAN_MARKETPLACE_NAME)
}

pub fn prepare_caveman_launch_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    let caveman_home = create_temporary_caveman_home(managed_profiles_root)?;
    if let Err(err) = prodex_shared_codex_fs::copy_codex_home(base_codex_home, &caveman_home)
        .and_then(|_| configure_caveman_launch_home(&caveman_home))
    {
        let _ = fs::remove_dir_all(&caveman_home);
        return Err(err);
    }
    Ok(caveman_home)
}

pub fn configure_caveman_launch_home(codex_home: &Path) -> Result<()> {
    localize_caveman_config_file(codex_home)?;
    configure_caveman_config(codex_home)?;
    install_caveman_marketplace(codex_home)?;
    install_caveman_plugin_cache(codex_home)?;
    Ok(())
}

pub fn configure_rtk_codex_home(codex_home: &Path) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let rtk_md_path = codex_home.join(RTK_MD);
    fs::write(&rtk_md_path, PRODEX_RTK_CODEX_AWARENESS)
        .with_context(|| format!("failed to write {}", rtk_md_path.display()))?;
    localize_rtk_agents_file(codex_home)?;
    ensure_rtk_agents_reference(codex_home, &rtk_md_path)
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

pub fn install_claude_caveman_plugin(prodex_root: &Path) -> Result<PathBuf> {
    let plugin_dir = prodex_root
        .join("claude-plugins")
        .join(PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME);
    write_caveman_plugin_files(&plugin_dir, CLAUDE_CAVEMAN_PLUGIN_FILES)?;
    Ok(plugin_dir)
}

fn create_temporary_caveman_home(managed_profiles_root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(managed_profiles_root).with_context(|| {
        format!(
            "failed to create managed profile root {}",
            managed_profiles_root.display()
        )
    })?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = managed_profiles_root
            .join(format!(".caveman-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        prodex_shared_codex_fs::create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for Caveman")
}

fn localize_caveman_config_file(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    match fs::symlink_metadata(&config_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let contents = read_optional_link_target_contents(&config_path)?;
            fs::remove_file(&config_path)
                .with_context(|| format!("failed to remove {}", config_path.display()))?;
            fs::write(&config_path, contents)
                .with_context(|| format!("failed to write {}", config_path.display()))?;
        }
        Ok(metadata) if metadata.is_dir() => {
            bail!("{} is a directory, expected a file", config_path.display());
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::write(&config_path, "")
                .with_context(|| format!("failed to write {}", config_path.display()))?;
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", config_path.display()));
        }
    }
    Ok(())
}

fn localize_rtk_agents_file(codex_home: &Path) -> Result<()> {
    let agents_path = codex_home.join(AGENTS_MD);
    match fs::symlink_metadata(&agents_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let contents = read_optional_link_target_contents(&agents_path)?;
            fs::remove_file(&agents_path)
                .with_context(|| format!("failed to remove {}", agents_path.display()))?;
            fs::write(&agents_path, contents)
                .with_context(|| format!("failed to write {}", agents_path.display()))?;
        }
        Ok(metadata) if metadata.is_dir() => {
            bail!("{} is a directory, expected a file", agents_path.display());
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::write(&agents_path, "")
                .with_context(|| format!("failed to write {}", agents_path.display()))?;
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", agents_path.display()));
        }
    }
    Ok(())
}

fn read_optional_link_target_contents(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn ensure_rtk_agents_reference(codex_home: &Path, rtk_md_path: &Path) -> Result<()> {
    let agents_path = codex_home.join(AGENTS_MD);
    let reference = format!("@{}", rtk_md_path.display());
    let contents = fs::read_to_string(&agents_path)
        .with_context(|| format!("failed to read {}", agents_path.display()))?;
    if contents.lines().any(|line| line.trim() == reference) {
        return Ok(());
    }

    let mut updated = String::new();
    if contents.trim().is_empty() {
        updated.push_str(&reference);
        updated.push('\n');
    } else {
        updated.push_str(contents.trim_end());
        updated.push_str("\n\n");
        updated.push_str(&reference);
        updated.push('\n');
    }
    fs::write(&agents_path, updated)
        .with_context(|| format!("failed to write {}", agents_path.display()))?;
    Ok(())
}

fn configure_caveman_config(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let contents = fs::read_to_string(&config_path).unwrap_or_default();
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

    let features = ensure_child_table(&mut table, "features");
    features.insert("plugins".to_string(), toml::Value::Boolean(true));

    let caveman_hook_group_index = configure_caveman_session_start_hook(&mut table);
    configure_caveman_hook_trust_state(&mut table, &config_path, caveman_hook_group_index)?;

    let marketplaces = ensure_child_table(&mut table, "marketplaces");
    let caveman_marketplace = ensure_child_table(marketplaces, PRODEX_CAVEMAN_MARKETPLACE_NAME);
    caveman_marketplace.insert(
        "last_updated".to_string(),
        toml::Value::String(Local::now().to_rfc3339()),
    );
    caveman_marketplace.insert(
        "source_type".to_string(),
        toml::Value::String("git".to_string()),
    );
    caveman_marketplace.insert(
        "source".to_string(),
        toml::Value::String(PRODEX_CAVEMAN_SOURCE_REPO.to_string()),
    );
    caveman_marketplace.insert("ref".to_string(), toml::Value::String("main".to_string()));

    let plugins = ensure_child_table(&mut table, "plugins");
    let caveman_plugin = ensure_child_table(plugins, PRODEX_CAVEMAN_PLUGIN_ID);
    caveman_plugin.insert("enabled".to_string(), toml::Value::Boolean(true));

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Caveman config overlay")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

fn configure_caveman_session_start_hook(table: &mut toml::Table) -> usize {
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

fn configure_caveman_hook_trust_state(
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

fn ensure_child_table<'a>(parent: &'a mut toml::Table, key: &str) -> &'a mut toml::Table {
    if !matches!(parent.get(key), Some(toml::Value::Table(_))) {
        parent.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    match parent.get_mut(key) {
        Some(toml::Value::Table(table)) => table,
        _ => unreachable!("child table should exist after insertion"),
    }
}

fn write_caveman_plugin_tree(root: &Path) -> Result<()> {
    write_caveman_plugin_files(root, CAVEMAN_CORE_PLUGIN_FILES)?;
    if caveman_full_assets_enabled() {
        write_caveman_plugin_files(root, CAVEMAN_COMPRESS_PLUGIN_FILES)?;
    }
    Ok(())
}

fn write_caveman_plugin_files(root: &Path, files: &[EmbeddedCavemanFile]) -> Result<()> {
    for file in files {
        let path = root.join(file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, file.contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn caveman_full_assets_enabled() -> bool {
    env::var(PRODEX_CAVEMAN_FULL_ASSETS_ENV)
        .ok()
        .is_some_and(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no" | "off"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!(
            "prodex-caveman-assets-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn configure_rtk_codex_home_writes_awareness_and_agents_reference() {
        let dir = temp_dir("rtk");
        configure_rtk_codex_home(&dir).expect("rtk codex home should configure");

        let rtk_md = fs::read_to_string(dir.join("RTK.md")).expect("RTK.md should exist");
        assert!(rtk_md.contains("RTK - Rust Token Killer"));
        assert!(rtk_md.contains("rtk gain"));

        let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
        assert_eq!(agents, format!("@{}\n", dir.join("RTK.md").display()));

        configure_rtk_codex_home(&dir).expect("rtk codex home should be idempotent");
        let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
        assert_eq!(agents.matches("@").count(), 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn configure_rtk_codex_home_localizes_agents_symlink() {
        let source = temp_dir("rtk-source");
        let overlay = temp_dir("rtk-overlay");
        fs::create_dir_all(&source).expect("source dir");
        fs::create_dir_all(&overlay).expect("overlay dir");
        fs::write(source.join("AGENTS.md"), "# Shared\n").expect("source AGENTS.md");
        std::os::unix::fs::symlink(source.join("AGENTS.md"), overlay.join("AGENTS.md"))
            .expect("agents symlink");

        configure_rtk_codex_home(&overlay).expect("rtk codex home should configure");

        assert!(
            !fs::symlink_metadata(overlay.join("AGENTS.md"))
                .expect("overlay AGENTS metadata")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(source.join("AGENTS.md")).expect("source AGENTS.md"),
            "# Shared\n"
        );
        let overlay_agents =
            fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
        assert!(overlay_agents.contains("# Shared"));
        assert!(overlay_agents.contains(&format!("@{}", overlay.join("RTK.md").display())));

        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(overlay);
    }

    #[cfg(unix)]
    #[test]
    fn configure_rtk_codex_home_replaces_broken_agents_symlink() {
        let overlay = temp_dir("rtk-broken-overlay");
        fs::create_dir_all(&overlay).expect("overlay dir");
        std::os::unix::fs::symlink(overlay.join("missing-AGENTS.md"), overlay.join("AGENTS.md"))
            .expect("broken agents symlink");

        configure_rtk_codex_home(&overlay).expect("rtk codex home should configure");

        assert!(
            !fs::symlink_metadata(overlay.join("AGENTS.md"))
                .expect("overlay AGENTS metadata")
                .file_type()
                .is_symlink()
        );
        let overlay_agents =
            fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
        assert_eq!(
            overlay_agents,
            format!("@{}\n", overlay.join("RTK.md").display())
        );

        let _ = fs::remove_dir_all(overlay);
    }

    #[cfg(unix)]
    #[test]
    fn prepare_caveman_home_then_rtk_handles_broken_agents_symlink() {
        let base = temp_dir("rtk-broken-base");
        let managed_root = temp_dir("rtk-broken-managed");
        fs::create_dir_all(&base).expect("base dir");
        std::os::unix::fs::symlink(base.join("missing-AGENTS.md"), base.join("AGENTS.md"))
            .expect("broken base agents symlink");

        let overlay = prepare_caveman_launch_home(&managed_root, &base)
            .expect("caveman launch home should prepare");
        configure_rtk_codex_home(&overlay).expect("rtk codex home should configure");

        assert!(
            !fs::symlink_metadata(overlay.join("AGENTS.md"))
                .expect("overlay AGENTS metadata")
                .file_type()
                .is_symlink()
        );
        let overlay_agents =
            fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
        assert_eq!(
            overlay_agents,
            format!("@{}\n", overlay.join("RTK.md").display())
        );

        let _ = fs::remove_dir_all(base);
        let _ = fs::remove_dir_all(managed_root);
    }

    #[cfg(unix)]
    #[test]
    fn prepare_caveman_home_handles_broken_config_symlink() {
        let base = temp_dir("broken-config-base");
        let managed_root = temp_dir("broken-config-managed");
        fs::create_dir_all(&base).expect("base dir");
        std::os::unix::fs::symlink(base.join("missing-config.toml"), base.join("config.toml"))
            .expect("broken config symlink");

        let overlay = prepare_caveman_launch_home(&managed_root, &base)
            .expect("caveman launch home should prepare");

        assert!(
            !fs::symlink_metadata(overlay.join("config.toml"))
                .expect("overlay config metadata")
                .file_type()
                .is_symlink()
        );
        let overlay_config =
            fs::read_to_string(overlay.join("config.toml")).expect("overlay config.toml");
        assert!(overlay_config.contains("prodex-caveman"));

        let _ = fs::remove_dir_all(base);
        let _ = fs::remove_dir_all(managed_root);
    }
}
