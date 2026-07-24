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
const PLAYWRIGHT_MCP_PACKAGE: &str = "@playwright/mcp@0.0.78";

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
    let npx_command = find_playwright_npx_command(&path_dirs);
    let ponytail_checkout = ponytail::find_ponytail_checkout(&optimizer_roots);

    let optimizers_path = codex_home.join(SUPER_OPTIMIZERS_MD);
    let awareness = render_super_optimizer_awareness(
        &path_dirs,
        codebase_memory_command.as_deref(),
        npx_command.as_deref(),
        ponytail_checkout.as_deref(),
        presidio_enabled,
    );
    write_text_file(&optimizers_path, &awareness)?;
    localize_text_file(&codex_home.join(AGENTS_MD))?;
    ensure_agents_reference(codex_home, &optimizers_path)?;
    configure_super_mcp_servers(
        codex_home,
        codebase_memory_command.as_deref(),
        npx_command.as_deref(),
    )?;
    if let Some(checkout) = ponytail_checkout {
        ponytail::install_ponytail_plugin(codex_home, &checkout)?;
    }
    Ok(())
}

fn render_super_optimizer_awareness(
    path_dirs: &[PathBuf],
    codebase_memory_command: Option<&Path>,
    npx_command: Option<&Path>,
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
        "- playwright-mcp: {}\n",
        availability_label(npx_command)
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

fn configure_super_mcp_servers(
    codex_home: &Path,
    codebase_memory_command: Option<&Path>,
    npx_command: Option<&Path>,
) -> Result<()> {
    if codebase_memory_command.is_none() && npx_command.is_none() {
        return Ok(());
    }
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
    if let Some((bridge, bridge_args)) =
        codebase_memory_command.and_then(mcp_jsonl_bridge_command_args)
    {
        let env_vars = codebase_memory_mcp_env()?;
        configure_stdio_mcp_server(
            &mut table,
            "codebase-memory-mcp",
            bridge,
            &bridge_args,
            &env_vars,
        );
    }
    if let Some(command) = npx_command {
        configure_default_playwright_mcp_server(&mut table, command);
    }
    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Super optimizer config overlay")?;
    write_text_file(&config_path, &rendered)
}

fn configure_default_playwright_mcp_server(table: &mut toml::Table, command: &Path) {
    let Some(mcp_servers) = mcp_servers_table(table) else {
        return;
    };
    if mcp_servers.contains_key("playwright") {
        return;
    }
    let server = ensure_child_table(mcp_servers, "playwright");
    configure_stdio_mcp_server_fields(
        server,
        command.to_path_buf(),
        &[
            "-y".to_string(),
            PLAYWRIGHT_MCP_PACKAGE.to_string(),
            "--headless".to_string(),
            "--isolated".to_string(),
        ],
        &[],
    );
    server.insert("enabled".to_string(), toml::Value::Boolean(true));
    server.insert("startup_timeout_sec".to_string(), toml::Value::Integer(60));
    server.insert(
        "default_tools_approval_mode".to_string(),
        toml::Value::String("writes".to_string()),
    );
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
    configure_stdio_mcp_server_fields(server, command, args, env_vars);
}

fn configure_stdio_mcp_server_fields(
    server: &mut toml::Table,
    command: PathBuf,
    args: &[String],
    env_vars: &[(&str, String)],
) {
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

fn codebase_memory_mcp_env() -> Result<Vec<(&'static str, String)>> {
    let Some(prodex_home) = env::var_os(PRODEX_HOME_ENV)
        .map(PathBuf::from)
        .or_else(|| discovery::home_dir_from_env().map(|home| home.join(".prodex")))
    else {
        return Ok(Vec::new());
    };
    let cache_dir = prepare_codebase_memory_cache_dir(&prodex_home)?;
    Ok(vec![("CBM_CACHE_DIR", cache_dir.display().to_string())])
}

fn prepare_codebase_memory_cache_dir(prodex_home: &Path) -> Result<PathBuf> {
    let optimizer_state = prodex_home.join("optimizer-state");
    let codebase_memory = optimizer_state.join("codebase-memory");
    let cache_dir = codebase_memory.join("cache");
    for path in [&optimizer_state, &codebase_memory, &cache_dir] {
        prodex_shared_codex_fs::create_codex_home_if_missing(path)?;
    }
    Ok(cache_dir)
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
            for suffix in [".exe", ".cmd", ".bat"] {
                let candidate = dir.join(format!("{command}{suffix}"));
                if optimizer_command_ready(command, &candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

pub fn super_playwright_npx_command() -> Option<PathBuf> {
    find_playwright_npx_command(&discovery::path_dirs_from_env())
}

fn find_playwright_npx_command(path_dirs: &[PathBuf]) -> Option<PathBuf> {
    let node = find_path_command("node", path_dirs)?;
    let output = Command::new(node).arg("--version").output().ok()?;
    if !output.status.success() || node_major_version(&output.stdout)? < 18 {
        return None;
    }
    find_path_command("npx", path_dirs)
}

fn node_major_version(output: &[u8]) -> Option<u64> {
    std::str::from_utf8(output)
        .ok()?
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()
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
    let Ok(env_vars) = codebase_memory_mcp_env() else {
        return false;
    };
    let mut command = Command::new(path);
    command.envs(env_vars);
    let mut child = match command
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
