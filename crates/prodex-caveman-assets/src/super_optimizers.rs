use anyhow::{Context, Result};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::fs_ops::{read_text_file_limited, write_text_file};
use crate::localization::localize_text_file;
use crate::toml_helpers::ensure_child_table;
use crate::{AGENTS_MD, PRODEX_SUPER_OPTIMIZER_AWARENESS, SUPER_OPTIMIZERS_MD};

mod discovery;
mod ponytail;

pub const PRODEX_OPTIMIZERS_HOME_ENV: &str = "PRODEX_OPTIMIZERS_HOME";
const PRODEX_HOME_ENV: &str = "PRODEX_HOME";

pub fn configure_super_optimizer_codex_home(codex_home: &Path) -> Result<()> {
    configure_super_optimizer_codex_home_with_presidio(codex_home, false)
}

pub fn configure_super_optimizer_codex_home_with_presidio(
    codex_home: &Path,
    presidio_enabled: bool,
) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let path_dirs = discovery::path_dirs_from_env();
    let optimizer_roots = discovery::managed_optimizer_roots();
    let codebase_memory_command =
        find_optimizer_command("codebase-memory-mcp", &path_dirs, &optimizer_roots);
    let ponytail_checkout = ponytail::find_ponytail_checkout(&optimizer_roots);

    let optimizers_path = codex_home.join(SUPER_OPTIMIZERS_MD);
    let awareness = render_super_optimizer_awareness(
        &path_dirs,
        codebase_memory_command.as_deref(),
        ponytail_checkout.as_deref(),
        presidio_enabled,
    );
    write_text_file(&optimizers_path, &awareness)?;
    localize_text_file(&codex_home.join(AGENTS_MD))?;
    ensure_agents_reference(codex_home, &optimizers_path)?;
    configure_codebase_memory_mcp(codex_home, codebase_memory_command.as_deref())?;
    if let Some(checkout) = ponytail_checkout {
        ponytail::install_ponytail_plugin(codex_home, &checkout)?;
    }
    Ok(())
}

fn render_super_optimizer_awareness(
    path_dirs: &[PathBuf],
    codebase_memory_command: Option<&Path>,
    ponytail_checkout: Option<&Path>,
    presidio_enabled: bool,
) -> String {
    let mut awareness = PRODEX_SUPER_OPTIMIZER_AWARENESS.to_string();
    awareness.push_str("\n## Available Now\n\n");
    awareness.push_str(&format!(
        "- rtk: {}\n",
        availability_label(find_path_command("rtk", path_dirs).as_deref())
    ));
    awareness.push_str(&format!(
        "- codebase-memory-mcp: {}\n",
        availability_label(codebase_memory_command)
    ));
    awareness.push_str(&format!(
        "- ponytail plugin: {}\n",
        availability_label(ponytail_checkout)
    ));
    awareness.push_str(&format!(
        "- presidio: {}\n",
        if presidio_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));
    awareness
}

fn availability_label(path: Option<&Path>) -> String {
    path.map(|path| format!("yes ({})", path.display()))
        .unwrap_or_else(|| "no".to_string())
}

fn ensure_agents_reference(codex_home: &Path, reference_path: &Path) -> Result<()> {
    let agents_path = codex_home.join(AGENTS_MD);
    let reference = format!("@{}", reference_path.display());
    let contents = read_text_file_limited(&agents_path)?.unwrap_or_default();
    if contents.lines().any(|line| line.trim() == reference) {
        return Ok(());
    }

    let updated = if contents.trim().is_empty() {
        format!("{reference}\n")
    } else {
        format!("{}\n\n{reference}\n", contents.trim_end())
    };
    write_text_file(&agents_path, &updated)
}

fn configure_codebase_memory_mcp(codex_home: &Path, command: Option<&Path>) -> Result<()> {
    let Some(command) = command else {
        return Ok(());
    };
    let Some((bridge, bridge_args)) = mcp_jsonl_bridge_command_args(command) else {
        return Ok(());
    };

    let config_path = codex_home.join("config.toml");
    let contents = read_text_file_limited(&config_path)?.unwrap_or_default();
    let mut table = if contents.trim().is_empty() {
        toml::Table::new()
    } else {
        match toml::from_str::<toml::Value>(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
        {
            toml::Value::Table(table) => table,
            _ => anyhow::bail!("{} did not parse as a TOML table", config_path.display()),
        }
    };
    configure_stdio_mcp_server(
        &mut table,
        "codebase-memory-mcp",
        bridge,
        &bridge_args,
        &codebase_memory_mcp_env(),
    );
    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Super optimizer config overlay")?;
    write_text_file(&config_path, &rendered)
}

fn configure_stdio_mcp_server(
    table: &mut toml::Table,
    name: &str,
    command: PathBuf,
    args: &[String],
    env_vars: &[(&str, String)],
) {
    let Some(mcp_servers) = mcp_servers_table(table) else {
        return;
    };
    let server = ensure_child_table(mcp_servers, name);
    server.insert(
        "command".to_string(),
        toml::Value::String(command.display().to_string()),
    );
    if args.is_empty() {
        server.remove("args");
    } else {
        server.insert(
            "args".to_string(),
            toml::Value::Array(args.iter().cloned().map(toml::Value::String).collect()),
        );
    }
    if env_vars.is_empty() {
        server.remove("env");
    } else {
        let env = env_vars
            .iter()
            .map(|(key, value)| (key.to_string(), toml::Value::String(value.clone())))
            .collect();
        server.insert("env".to_string(), toml::Value::Table(env));
    }
}

fn mcp_servers_table(table: &mut toml::Table) -> Option<&mut toml::Table> {
    if !table.contains_key("mcp_servers") {
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
    }
    match table.get_mut("mcp_servers") {
        Some(toml::Value::Table(table)) => Some(table),
        _ => None,
    }
}

fn codebase_memory_mcp_env() -> Vec<(&'static str, String)> {
    let prodex_home = env::var_os(PRODEX_HOME_ENV)
        .map(PathBuf::from)
        .or_else(|| discovery::home_dir_from_env().map(|home| home.join(".prodex")));
    prodex_home
        .map(|home| {
            vec![(
                "CBM_CACHE_DIR",
                home.join("optimizer-state")
                    .join("codebase-memory")
                    .join("cache")
                    .display()
                    .to_string(),
            )]
        })
        .unwrap_or_default()
}

fn find_optimizer_command(
    command: &str,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
) -> Option<PathBuf> {
    find_managed_optimizer_command(command, optimizer_roots)
        .or_else(|| find_path_command(command, path_dirs))
}

fn find_path_command(command: &str, path_dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in path_dirs {
        let candidate = dir.join(command);
        if optimizer_command_ready(command, &candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{command}.exe"));
            if optimizer_command_ready(command, &candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_managed_optimizer_command(command: &str, optimizer_roots: &[PathBuf]) -> Option<PathBuf> {
    optimizer_roots.iter().find_map(|root| {
        discovery::managed_optimizer_command_candidates(root, command)
            .into_iter()
            .find(|candidate| optimizer_command_ready(command, candidate))
    })
}

fn find_prodex_binary() -> Option<PathBuf> {
    env::current_exe().ok().filter(|path| path.is_file())
}

fn mcp_jsonl_bridge_command_args(command: &Path) -> Option<(PathBuf, Vec<String>)> {
    Some((
        find_prodex_binary()?,
        vec![
            "__mcp-jsonl-bridge".to_string(),
            command.display().to_string(),
        ],
    ))
}

fn optimizer_command_ready(command: &str, path: &Path) -> bool {
    path.is_file() && (command != "codebase-memory-mcp" || mcp_tools_list_non_empty_jsonl(path))
}

pub fn super_optimizer_command_ready(command: &str, path: &Path) -> bool {
    optimizer_command_ready(command, path)
}

fn mcp_tools_list_non_empty_jsonl(path: &Path) -> bool {
    let mut child = match Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        return false;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return false;
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let messages = [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prodex-probe","version":"0"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    ];
    for message in messages {
        let Ok(line) = serde_json::to_string(&message) else {
            let _ = child.kill();
            return false;
        };
        if writeln!(stdin, "{line}").is_err() {
            let _ = child.kill();
            return false;
        }
    }
    drop(stdin);

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut ready = false;
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let Ok(line) = receiver.recv_timeout(remaining.min(Duration::from_millis(200))) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if value.get("id").and_then(serde_json::Value::as_i64) == Some(2) {
            ready = value
                .get("result")
                .and_then(|result| result.get("tools"))
                .and_then(serde_json::Value::as_array)
                .is_some_and(|tools| !tools.is_empty());
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    ready
}

#[cfg(test)]
#[path = "super_optimizers_tests.rs"]
mod tests;
