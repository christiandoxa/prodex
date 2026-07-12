use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};

const GEMINI_SETTINGS_FILE_LIMIT: usize = 512 * 1024;

#[derive(Debug, Clone)]
pub struct GeminiSettingsSource {
    pub name: String,
    pub directory: PathBuf,
    pub value: serde_json::Value,
    pub mcp_servers: BTreeMap<String, serde_json::Value>,
}

pub fn gemini_settings_sources(cwd: Option<&Path>) -> Vec<GeminiSettingsSource> {
    gemini_settings_sources_from_paths(gemini_settings_source_paths(cwd))
}

pub fn gemini_settings_sources_for_config_home(
    config_home: Option<&Path>,
    cwd: Option<&Path>,
    system_settings: Option<&Path>,
    system_defaults: Option<&Path>,
) -> Vec<GeminiSettingsSource> {
    gemini_settings_sources_from_paths(gemini_settings_source_paths_for_config_home(
        config_home,
        cwd,
        system_settings,
        system_defaults,
    ))
}

fn gemini_settings_sources_from_paths(paths: Vec<(String, PathBuf)>) -> Vec<GeminiSettingsSource> {
    let mut sources = Vec::new();
    for (name, path) in paths {
        let Some(text) = crate::fs_utils::read_text_limited(&path, GEMINI_SETTINGS_FILE_LIMIT)
        else {
            continue;
        };
        let Some(value) = parse_gemini_settings_json(&text) else {
            continue;
        };
        let mcp_servers = value
            .get("mcpServers")
            .or_else(|| value.get("mcp_servers"))
            .and_then(serde_json::Value::as_object)
            .map(|servers| {
                servers
                    .iter()
                    .map(|(server_name, value)| (server_name.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let has_model_configs = value
            .get("modelConfigs")
            .or_else(|| value.get("model_configs"))
            .is_some();
        if mcp_servers.is_empty() && value.get("hooks").is_none() && !has_model_configs {
            continue;
        }
        sources.push(GeminiSettingsSource {
            name,
            directory: path.parent().unwrap_or(Path::new("")).to_path_buf(),
            value,
            mcp_servers,
        });
    }
    sources
}

pub fn gemini_settings_source_paths(cwd: Option<&Path>) -> Vec<(String, PathBuf)> {
    gemini_settings_source_paths_for(dirs::home_dir().as_deref(), cwd)
}

pub fn gemini_settings_source_paths_for(
    home: Option<&Path>,
    cwd: Option<&Path>,
) -> Vec<(String, PathBuf)> {
    let config_home = gemini_cli_config_home_for(home);
    let system_settings = env::var_os("GEMINI_CLI_SYSTEM_SETTINGS_PATH").map(PathBuf::from);
    let system_defaults = env::var_os("GEMINI_CLI_SYSTEM_DEFAULTS_PATH").map(PathBuf::from);
    gemini_settings_source_paths_for_config_home(
        config_home.as_deref(),
        cwd,
        system_settings.as_deref(),
        system_defaults.as_deref(),
    )
}

pub fn gemini_settings_source_paths_for_config_home(
    config_home: Option<&Path>,
    cwd: Option<&Path>,
    system_settings: Option<&Path>,
    system_defaults: Option<&Path>,
) -> Vec<(String, PathBuf)> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_unique = |name: String, path: PathBuf| {
        let key = path.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            paths.push((name, path));
        }
    };
    let system_settings = system_settings
        .map(Path::to_path_buf)
        .unwrap_or_else(gemini_default_system_settings_path);
    let system_defaults = system_defaults.map(Path::to_path_buf).unwrap_or_else(|| {
        system_settings
            .parent()
            .unwrap_or(Path::new(""))
            .join("system-defaults.json")
    });
    push_unique("system-defaults".to_string(), system_defaults);
    if let Some(gemini_home) = config_home {
        push_unique("global".to_string(), gemini_home.join("settings.json"));
    }
    if let Some(cwd) = cwd {
        let mut ancestors = cwd.ancestors().collect::<Vec<_>>();
        ancestors.reverse();
        for directory in ancestors {
            push_unique(
                format!("project:{}", directory.display()),
                directory.join(".gemini").join("settings.json"),
            );
        }
        push_unique(
            format!("project-local:{}", cwd.display()),
            cwd.join(".gemini").join("settings.local.json"),
        );
    }
    push_unique("system".to_string(), system_settings);
    paths
}

fn gemini_default_system_settings_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        return PathBuf::from("/Library/Application Support/GeminiCli/settings.json");
    }
    #[cfg(target_os = "windows")]
    {
        return PathBuf::from(r"C:\ProgramData\gemini-cli\settings.json");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from("/etc/gemini-cli/settings.json")
    }
}

pub fn gemini_cli_config_home_for(home: Option<&Path>) -> Option<PathBuf> {
    if let Some(root) = env::var_os("GEMINI_CLI_HOME")
        && !root.is_empty()
    {
        return Some(PathBuf::from(root).join(".gemini"));
    }
    home.map(|home| home.join(".gemini"))
}

pub fn parse_gemini_settings_json(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .or_else(|| serde_json::from_str::<serde_json::Value>(&strip_json_comments(text)).ok())
}

fn strip_json_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == '/' {
            match chars.peek().copied() {
                Some('/') => {
                    let _ = chars.next();
                    for next in chars.by_ref() {
                        if next == '\n' {
                            output.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    let _ = chars.next();
                    let mut previous = '\0';
                    for next in chars.by_ref() {
                        if previous == '*' && next == '/' {
                            break;
                        }
                        if next == '\n' {
                            output.push('\n');
                        }
                        previous = next;
                    }
                    continue;
                }
                _ => {}
            }
        }
        output.push(ch);
    }
    output
}
