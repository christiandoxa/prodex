pub mod settings;

use anyhow::{Context, Result, bail};
pub use settings::{
    GeminiSettingsSource, gemini_cli_config_home_for, gemini_settings_source_paths,
    gemini_settings_source_paths_for, gemini_settings_source_paths_for_config_home,
    gemini_settings_sources, gemini_settings_sources_for_config_home, parse_gemini_settings_json,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod assets;
mod fs_utils;
mod hooks;
mod prompts;
mod utils;

use assets::{write_gemini_admin_helpers, write_gemini_agents, write_gemini_skills};
use fs_utils::*;
use hooks::write_gemini_hooks;
use prompts::write_gemini_prompts;
use utils::*;

const GEMINI_EXTENSION_SCAN_LIMIT: usize = 64;
pub(crate) const GEMINI_COMPAT_FILE_LIMIT: usize = 512 * 1024;
const GENERATED_MARKER: &str = "prodex-gemini-cli-compat";
pub(crate) const GENERATED_PROMPT_MARKER: &str =
    "<!-- prodex-gemini-cli-compat generated prompt -->";
const GENERATED_SKILL_MARKER_FILE: &str = ".prodex-gemini-cli-compat";
const GEMINI_PROMPT_PREFIX: &str = "gemini";

#[derive(Debug, Clone)]
pub(crate) struct GeminiExtension {
    pub(crate) directory: PathBuf,
    pub(crate) name: String,
    pub(crate) value: serde_json::Value,
}

pub fn prepare_gemini_cli_compat(codex_home: &Path) -> Result<()> {
    if gemini_env_bool("PRODEX_GEMINI_DISABLE_CLI_COMPAT") == Some(true) {
        return Ok(());
    }

    fs::create_dir_all(codex_home)
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    let cwd = env::current_dir().ok();
    let extensions = active_extension_manifests_from_roots(
        &gemini_extension_roots(cwd.as_deref()),
        cwd.as_deref(),
    );

    write_gemini_mcp_config(codex_home, &extensions, cwd.as_deref())?;
    write_gemini_hooks(codex_home, &extensions, cwd.as_deref())?;
    write_gemini_prompts(codex_home, &extensions, cwd.as_deref())?;
    write_gemini_skills(codex_home, &extensions)?;
    write_gemini_agents(codex_home, &extensions)?;
    write_gemini_admin_helpers(codex_home)?;
    Ok(())
}

fn write_gemini_mcp_config(
    codex_home: &Path,
    extensions: &[GeminiExtension],
    cwd: Option<&Path>,
) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let mut table = read_toml_table(&config_path)?;
    remove_generated_mcp_servers(&mut table);
    let settings_sources = gemini_settings_sources(cwd);
    let (allowed_mcp_servers, excluded_mcp_servers) =
        gemini_mcp_settings_filters(&settings_sources);
    let settings_server_names = settings_sources
        .iter()
        .flat_map(|source| source.mcp_servers.keys())
        .map(|name| name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();

    for extension in extensions {
        let Some(servers) = extension
            .value
            .get("mcpServers")
            .or_else(|| extension.value.get("mcp_servers"))
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let extension_env = read_extension_env(&extension.directory);
        for (server_name, server_value) in servers {
            if !gemini_mcp_server_enabled_by_filters(
                server_name,
                allowed_mcp_servers.as_ref(),
                &excluded_mcp_servers,
            ) {
                continue;
            }
            if settings_server_names.contains(&server_name.to_ascii_lowercase()) {
                continue;
            }
            let Some(server_object) = server_value.as_object() else {
                continue;
            };
            let codex_server_name = format!(
                "{}_{}_{}",
                GEMINI_PROMPT_PREFIX,
                safe_key(&extension.name),
                safe_key(server_name)
            );
            configure_gemini_mcp_server(
                &mut table,
                &codex_server_name,
                &extension.name,
                &extension.directory,
                server_object,
                &extension_env,
                cwd,
            );
        }
    }

    for settings in settings_sources {
        for (server_name, server_value) in settings.mcp_servers {
            if !gemini_mcp_server_enabled_by_filters(
                &server_name,
                allowed_mcp_servers.as_ref(),
                &excluded_mcp_servers,
            ) {
                continue;
            }
            let Some(server_object) = server_value.as_object() else {
                continue;
            };
            let codex_server_name = format!("{}_{}", GEMINI_PROMPT_PREFIX, safe_key(&server_name));
            configure_gemini_mcp_server(
                &mut table,
                &codex_server_name,
                &settings.name,
                &settings.directory,
                server_object,
                &BTreeMap::new(),
                cwd,
            );
        }
    }

    write_toml_table(
        &config_path,
        table,
        "Gemini CLI compatibility config overlay",
    )
}

fn configure_gemini_mcp_server(
    root_table: &mut toml::Table,
    server_name: &str,
    source_name: &str,
    source_directory: &Path,
    server: &serde_json::Map<String, serde_json::Value>,
    extension_env: &BTreeMap<String, String>,
    cwd: Option<&Path>,
) {
    let Some(mcp_servers) = ensure_child_table(root_table, "mcp_servers") else {
        return;
    };
    if mcp_servers
        .get(server_name)
        .and_then(toml::Value::as_table)
        .is_some_and(|server| server.get(GENERATED_MARKER).is_none())
    {
        return;
    }

    let mut codex_server = toml::Table::new();
    codex_server.insert(
        GENERATED_MARKER.to_string(),
        toml::Value::String(source_name.to_string()),
    );
    let vars = GeminiCompatVars::new(source_directory, cwd).with_env(extension_env);

    if let Some(url) = json_string(server, &["httpUrl", "http_url", "url"]) {
        codex_server.insert("url".to_string(), toml::Value::String(vars.expand(&url)));
        insert_optional_string(
            &mut codex_server,
            "bearer_token_env_var",
            json_string(
                server,
                &[
                    "bearerTokenEnvVar",
                    "bearer_token_env_var",
                    "oauthTokenEnvVar",
                ],
            )
            .as_deref(),
            &vars,
        );
        insert_string_map(
            &mut codex_server,
            "http_headers",
            server
                .get("http_headers")
                .or_else(|| server.get("httpHeaders"))
                .or_else(|| server.get("headers")),
            &vars,
        );
        insert_string_map(
            &mut codex_server,
            "env_http_headers",
            server
                .get("env_http_headers")
                .or_else(|| server.get("envHttpHeaders")),
            &vars,
        );
    } else if let Some(command) = json_string(server, &["command", "cmd"]) {
        codex_server.insert(
            "command".to_string(),
            toml::Value::String(vars.expand(&command)),
        );
        if let Some(args) = json_string_array(server.get("args")) {
            codex_server.insert(
                "args".to_string(),
                toml::Value::Array(
                    args.into_iter()
                        .map(|arg| toml::Value::String(vars.expand(&arg)))
                        .collect(),
                ),
            );
        }
        insert_optional_string(
            &mut codex_server,
            "cwd",
            json_string(server, &["cwd", "workingDirectory", "working_directory"]).as_deref(),
            &vars,
        );
    } else {
        return;
    }

    if let Some(enabled) = json_bool(server, &["enabled"]) {
        codex_server.insert("enabled".to_string(), toml::Value::Boolean(enabled));
    } else if let Some(disabled) = json_bool(server, &["disabled"]) {
        codex_server.insert("enabled".to_string(), toml::Value::Boolean(!disabled));
    }
    if let Some(required) = json_bool(server, &["required"]) {
        codex_server.insert("required".to_string(), toml::Value::Boolean(required));
    }
    insert_optional_number(
        &mut codex_server,
        "startup_timeout_sec",
        gemini_mcp_timeout_seconds(
            server,
            &["startupTimeoutSec", "startup_timeout_sec", "timeoutSec"],
        ),
    );
    insert_optional_number(
        &mut codex_server,
        "tool_timeout_sec",
        gemini_mcp_timeout_seconds(server, &["toolTimeoutSec", "tool_timeout_sec"]),
    );
    insert_optional_string_array(
        &mut codex_server,
        "enabled_tools",
        server
            .get("enabled_tools")
            .or_else(|| server.get("enabledTools"))
            .or_else(|| server.get("includeTools")),
        &vars,
    );
    insert_optional_string_array(
        &mut codex_server,
        "disabled_tools",
        server
            .get("disabled_tools")
            .or_else(|| server.get("disabledTools"))
            .or_else(|| server.get("excludeTools")),
        &vars,
    );
    if let Some(mode) = json_string(
        server,
        &["defaultToolsApprovalMode", "default_tools_approval_mode"],
    ) {
        codex_server.insert(
            "default_tools_approval_mode".to_string(),
            toml::Value::String(vars.expand(&mode)),
        );
    } else if json_bool(server, &["trust"]).unwrap_or(false) {
        codex_server.insert(
            "default_tools_approval_mode".to_string(),
            toml::Value::String("approve".to_string()),
        );
    }

    let mut env_table = toml::Table::new();
    insert_env_map(&mut env_table, server.get("env"), &vars);
    for name in json_string_array(
        server
            .get("envVars")
            .or_else(|| server.get("env_vars"))
            .or_else(|| server.get("env")),
    )
    .unwrap_or_default()
    {
        if let Some(value) = extension_env
            .get(&name)
            .cloned()
            .or_else(|| env::var(&name).ok())
        {
            env_table.insert(name, toml::Value::String(vars.expand(&value)));
        }
    }
    for (key, value) in extension_env {
        if placeholder_mentioned(server, key) {
            env_table
                .entry(key.clone())
                .or_insert_with(|| toml::Value::String(vars.expand(value)));
        }
    }
    if !env_table.is_empty() {
        codex_server.insert("env".to_string(), toml::Value::Table(env_table));
    }

    mcp_servers.insert(server_name.to_string(), toml::Value::Table(codex_server));
}

pub(crate) fn extension_skill_dirs(extension: &GeminiExtension) -> Vec<PathBuf> {
    let skills_dir = extension.directory.join("skills");
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return Vec::new();
    };
    let mut dirs = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let Ok(metadata) = fs::symlink_metadata(path) else {
                return false;
            };
            metadata.is_dir() && !metadata.file_type().is_symlink()
        })
        .take(GEMINI_EXTENSION_SCAN_LIMIT)
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn gemini_extension_roots(cwd: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(configured) = env::var_os("PRODEX_GEMINI_EXTENSION_DIRS") {
        roots.extend(env::split_paths(&configured));
    }
    if let Some(gemini_home) = gemini_cli_config_home_for(dirs::home_dir().as_deref()) {
        roots.push(gemini_home.join("extensions"));
    }
    if let Some(cwd) = cwd {
        roots.push(cwd.join(".gemini").join("extensions"));
    }
    dedupe_paths(roots)
}

fn active_extension_manifests_from_roots(
    roots: &[PathBuf],
    cwd: Option<&Path>,
) -> Vec<GeminiExtension> {
    let mut manifests = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if manifests.len() >= GEMINI_EXTENSION_SCAN_LIMIT {
            break;
        }
        if path_has_symlink_component(root) {
            continue;
        }
        if root.join("gemini-extension.json").is_file() {
            if let Some(manifest) = load_extension_manifest(root, root.parent(), cwd)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if manifests.len() >= GEMINI_EXTENSION_SCAN_LIMIT {
                break;
            }
            let directory = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&directory) else {
                continue;
            };
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || !directory.join("gemini-extension.json").is_file()
            {
                continue;
            }
            if let Some(manifest) = load_extension_manifest(&directory, Some(root), cwd)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
        }
    }
    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    manifests
}

fn load_extension_manifest(
    directory: &Path,
    root: Option<&Path>,
    cwd: Option<&Path>,
) -> Option<GeminiExtension> {
    let text = read_text_limited(
        &directory.join("gemini-extension.json"),
        GEMINI_COMPAT_FILE_LIMIT,
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })?;
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| directory.parent().unwrap_or(directory).to_path_buf());
    if !extension_is_enabled(&name, cwd, &root) {
        return None;
    }
    Some(GeminiExtension {
        directory: directory.to_path_buf(),
        name,
        value,
    })
}

fn extension_is_enabled(name: &str, cwd: Option<&Path>, extension_root: &Path) -> bool {
    if let Some(enabled) = extension_name_override(name) {
        return enabled;
    }
    let Some(cwd) = cwd else {
        return true;
    };
    let path = extension_root.join("extension-enablement.json");
    let Some(text) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    let Some(overrides) = value
        .get(name)
        .and_then(|extension| extension.get("overrides"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    let mut enabled = true;
    for rule in overrides.iter().filter_map(serde_json::Value::as_str) {
        if let Some(disable) = extension_override_matches(rule, cwd) {
            enabled = !disable;
        }
    }
    enabled
}

fn extension_name_override(name: &str) -> Option<bool> {
    let value = env::var("PRODEX_GEMINI_EXTENSIONS").ok()?;
    let requested = value
        .split([',', ';', ' ', '\n', '\t'])
        .filter_map(|item| {
            let item = item.trim().to_ascii_lowercase();
            (!item.is_empty()).then_some(item)
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return None;
    }
    if requested.len() == 1 && requested[0] == "none" {
        return Some(false);
    }
    Some(
        requested
            .iter()
            .any(|item| item == &name.to_ascii_lowercase()),
    )
}

fn extension_override_matches(rule: &str, cwd: &Path) -> Option<bool> {
    let mut rule = rule.trim();
    if rule.is_empty() {
        return None;
    }
    let disable = rule.starts_with('!');
    if disable {
        rule = &rule[1..];
    }
    let include_subdirs = rule.ends_with('*');
    if include_subdirs {
        rule = &rule[..rule.len().saturating_sub(1)];
    }
    let rule = normalize_enablement_path(rule);
    let cwd = normalize_enablement_path(&cwd.to_string_lossy());
    let matches = if include_subdirs {
        cwd.starts_with(&rule)
    } else {
        cwd == rule
    };
    matches.then_some(disable)
}

fn normalize_enablement_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    if !value.starts_with('/') {
        value.insert(0, '/');
    }
    if !value.ends_with('/') {
        value.push('/');
    }
    value
}

fn gemini_mcp_settings_filters(
    settings_sources: &[GeminiSettingsSource],
) -> (Option<BTreeSet<String>>, BTreeSet<String>) {
    let mut allowed = BTreeSet::new();
    let mut excluded = BTreeSet::new();
    for source in settings_sources {
        insert_mcp_filter_values(source.value.pointer("/mcp/allowed"), &mut allowed);
        insert_mcp_filter_values(source.value.pointer("/mcp/excluded"), &mut excluded);
    }
    let allowed = (!allowed.is_empty()).then_some(allowed);
    (allowed, excluded)
}

fn insert_mcp_filter_values(value: Option<&serde_json::Value>, output: &mut BTreeSet<String>) {
    for name in json_string_array(value).unwrap_or_default() {
        let name = name.trim().to_ascii_lowercase();
        if !name.is_empty() {
            output.insert(name);
        }
    }
}

fn gemini_mcp_server_enabled_by_filters(
    server_name: &str,
    allowed: Option<&BTreeSet<String>>,
    excluded: &BTreeSet<String>,
) -> bool {
    let name = server_name.trim().to_ascii_lowercase();
    if name.is_empty() || excluded.contains(&name) {
        return false;
    }
    if let Some(allowed) = allowed
        && !allowed.contains(&name)
    {
        return false;
    }
    true
}

pub(crate) fn read_toml_table(path: &Path) -> Result<toml::Table> {
    let contents = read_text_limited(path, GEMINI_COMPAT_FILE_LIMIT).unwrap_or_default();
    if contents.trim().is_empty() {
        return Ok(toml::Table::new());
    }
    match toml::from_str::<toml::Value>(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?
    {
        toml::Value::Table(table) => Ok(table),
        _ => bail!("{} did not parse as a TOML table", path.display()),
    }
}

pub(crate) fn write_toml_table(path: &Path, table: toml::Table, context: &str) -> Result<()> {
    let rendered = toml::to_string(&toml::Value::Table(table))
        .with_context(|| format!("failed to render {context}"))?;
    write_file_atomic_no_symlink(path, rendered)
}

fn remove_generated_mcp_servers(table: &mut toml::Table) {
    let Some(mcp_servers) = table
        .get_mut("mcp_servers")
        .and_then(toml::Value::as_table_mut)
    else {
        return;
    };
    mcp_servers.retain(|_, value| {
        value
            .as_table()
            .is_none_or(|server| server.get(GENERATED_MARKER).is_none())
    });
}

fn read_extension_env(directory: &Path) -> BTreeMap<String, String> {
    let Some(text) = read_text_limited(&directory.join(".env"), GEMINI_COMPAT_FILE_LIMIT) else {
        return BTreeMap::new();
    };
    parse_env_file(&text)
}

fn parse_env_file(text: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for line in text.lines() {
        let mut line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim_start();
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }
        env.insert(key.to_string(), unquote_env_value(value.trim()));
    }
    env
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn insert_env_map(
    table: &mut toml::Table,
    value: Option<&serde_json::Value>,
    vars: &GeminiCompatVars,
) {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return;
    };
    for (key, value) in object {
        if let Some(value) = value.as_str() {
            table.insert(key.clone(), toml::Value::String(vars.expand(value)));
        }
    }
}

fn insert_string_map(
    table: &mut toml::Table,
    key: &str,
    value: Option<&serde_json::Value>,
    vars: &GeminiCompatVars,
) {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return;
    };
    let mut output = toml::Table::new();
    for (key, value) in object {
        if let Some(value) = value.as_str() {
            output.insert(key.clone(), toml::Value::String(vars.expand(value)));
        }
    }
    if !output.is_empty() {
        table.insert(key.to_string(), toml::Value::Table(output));
    }
}

fn insert_optional_string(
    table: &mut toml::Table,
    key: &str,
    value: Option<&str>,
    vars: &GeminiCompatVars,
) {
    if let Some(value) = value {
        table.insert(key.to_string(), toml::Value::String(vars.expand(value)));
    }
}

fn insert_optional_number(table: &mut toml::Table, key: &str, value: Option<u64>) {
    if let Some(value) = value {
        table.insert(key.to_string(), toml::Value::Integer(value as i64));
    }
}

fn insert_optional_string_array(
    table: &mut toml::Table,
    key: &str,
    value: Option<&serde_json::Value>,
    vars: &GeminiCompatVars,
) {
    if let Some(items) = json_string_array(value) {
        table.insert(
            key.to_string(),
            toml::Value::Array(
                items
                    .into_iter()
                    .map(|item| toml::Value::String(vars.expand(&item)))
                    .collect(),
            ),
        );
    }
}

pub(crate) fn ensure_child_table<'a>(
    table: &'a mut toml::Table,
    key: &str,
) -> Option<&'a mut toml::Table> {
    if !table.contains_key(key) {
        table.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    match table.get_mut(key) {
        Some(toml::Value::Table(table)) => Some(table),
        _ => None,
    }
}

fn json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn json_bool(object: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_bool))
}

fn json_u64(object: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_u64))
}

fn gemini_mcp_timeout_seconds(
    object: &serde_json::Map<String, serde_json::Value>,
    explicit_second_keys: &[&str],
) -> Option<u64> {
    json_u64(object, explicit_second_keys)
        .filter(|seconds| *seconds > 0)
        .or_else(|| json_u64(object, &["timeout"]).and_then(gemini_mcp_timeout_millis_to_seconds))
}

fn gemini_mcp_timeout_millis_to_seconds(millis: u64) -> Option<u64> {
    if millis == 0 {
        return None;
    }
    Some(millis.saturating_add(999) / 1_000).filter(|seconds| *seconds > 0)
}

fn json_string_array(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    match value? {
        serde_json::Value::String(text) => Some(vec![text.to_string()]),
        serde_json::Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            (!values.is_empty()).then_some(values)
        }
        _ => None,
    }
}

pub(crate) fn toml_string(value: &toml::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(toml::Value::as_str))
        .map(str::to_string)
}

pub(crate) fn toml_string_array(value: &toml::Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .map(|value| match value {
            toml::Value::String(text) => vec![text.to_string()],
            toml::Value::Array(items) => items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

fn placeholder_mentioned(value: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    let needle = format!("${{{key}}}");
    value
        .values()
        .any(|value| value.to_string().contains(&needle))
}

#[cfg(test)]
mod tests;
