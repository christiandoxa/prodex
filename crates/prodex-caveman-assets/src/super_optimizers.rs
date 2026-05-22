use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::localization::localize_text_file;
use crate::toml_helpers::ensure_child_table;
use crate::{AGENTS_MD, PRODEX_SUPER_OPTIMIZER_AWARENESS, SUPER_OPTIMIZERS_MD};

pub fn configure_super_optimizer_codex_home(codex_home: &Path) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let optimizers_path = codex_home.join(SUPER_OPTIMIZERS_MD);
    fs::write(&optimizers_path, PRODEX_SUPER_OPTIMIZER_AWARENESS)
        .with_context(|| format!("failed to write {}", optimizers_path.display()))?;
    localize_text_file(&codex_home.join(AGENTS_MD))?;
    ensure_agents_reference(codex_home, &optimizers_path)?;
    configure_super_optimizer_mcp_servers(codex_home)
}

fn ensure_agents_reference(codex_home: &Path, reference_path: &Path) -> Result<()> {
    let agents_path = codex_home.join(AGENTS_MD);
    let reference = format!("@{}", reference_path.display());
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

fn configure_super_optimizer_mcp_servers(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let contents = fs::read_to_string(&config_path).unwrap_or_default();
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
    if let Some(command) = find_path_command("sqz-mcp") {
        configure_stdio_mcp_server(
            &mut table,
            "prodex-sqz",
            command,
            &["--transport", "stdio"],
            &[],
        );
    }

    if let Some(command) = find_path_command("token-savior") {
        let workspace_roots = env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let token_savior_env = [
            ("TOKEN_SAVIOR_CLIENT", "codex"),
            ("TOKEN_SAVIOR_PROFILE", "optimized"),
            ("TS_CAPTURE_DISABLED", "1"),
            ("TS_MEMORY_DISABLE", "1"),
            ("TS_AUTO_EXTRACT", "0"),
            ("WORKSPACE_ROOTS", workspace_roots.as_str()),
        ];
        configure_stdio_mcp_server(
            &mut table,
            "prodex-token-savior",
            command,
            &[],
            &token_savior_env,
        );
    }

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Super optimizer config overlay")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

fn configure_stdio_mcp_server(
    table: &mut toml::Table,
    server_name: &str,
    command: PathBuf,
    args: &[&str],
    env_vars: &[(&str, &str)],
) {
    let Some(mcp_servers) = super_optimizer_mcp_servers_table(table) else {
        return;
    };
    if mcp_servers.contains_key(server_name) {
        return;
    }
    let server = ensure_child_table(mcp_servers, server_name);
    server.insert(
        "command".to_string(),
        toml::Value::String(command.display().to_string()),
    );
    if args.is_empty() {
        server.remove("args");
    } else {
        server.insert(
            "args".to_string(),
            toml::Value::Array(
                args.iter()
                    .map(|arg| toml::Value::String((*arg).to_string()))
                    .collect(),
            ),
        );
    }
    if env_vars.is_empty() {
        server.remove("env");
    } else {
        let env_table = ensure_child_table(server, "env");
        for (key, value) in env_vars {
            env_table.insert(
                (*key).to_string(),
                toml::Value::String((*value).to_string()),
            );
        }
    }
}

fn super_optimizer_mcp_servers_table(table: &mut toml::Table) -> Option<&mut toml::Table> {
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

fn find_path_command(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(command);
        if executable_file(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{command}.exe"));
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_mcp_server_config_adds_missing_entry() {
        let mut table = toml::Table::new();
        configure_stdio_mcp_server(
            &mut table,
            "prodex-sqz",
            PathBuf::from("/tmp/sqz-mcp"),
            &["--transport", "stdio"],
            &[],
        );

        let mcp_servers = table
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .expect("mcp servers table should exist");
        let server = mcp_servers
            .get("prodex-sqz")
            .and_then(toml::Value::as_table)
            .expect("sqz server should exist");
        assert_eq!(
            server.get("command").and_then(toml::Value::as_str),
            Some("/tmp/sqz-mcp")
        );
        let args = server
            .get("args")
            .and_then(toml::Value::as_array)
            .expect("sqz args should exist")
            .iter()
            .map(|arg| arg.as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(args, vec!["--transport", "stdio"]);
    }

    #[test]
    fn stdio_mcp_server_config_preserves_existing_entries() {
        let mut table = toml::Table::new();
        let mcp_servers = ensure_child_table(&mut table, "mcp_servers");
        mcp_servers.insert(
            "prodex-sqz".to_string(),
            toml::Value::Table({
                let mut table = toml::Table::new();
                table.insert(
                    "command".to_string(),
                    toml::Value::String("/custom/sqz-mcp".to_string()),
                );
                table
            }),
        );
        mcp_servers.insert("custom".to_string(), toml::Value::Table(toml::Table::new()));

        configure_stdio_mcp_server(
            &mut table,
            "prodex-sqz",
            PathBuf::from("/tmp/sqz-mcp"),
            &["--transport", "stdio"],
            &[],
        );

        let mcp_servers = table
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .expect("mcp servers table should still exist");
        let server = mcp_servers
            .get("prodex-sqz")
            .and_then(toml::Value::as_table)
            .expect("sqz server should still exist");
        assert_eq!(
            server.get("command").and_then(toml::Value::as_str),
            Some("/custom/sqz-mcp")
        );
        assert!(mcp_servers.contains_key("custom"));
    }

    #[test]
    fn stdio_mcp_server_config_skips_non_table_mcp_servers() {
        let mut table = toml::Table::new();
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::String("invalid".to_string()),
        );

        configure_stdio_mcp_server(
            &mut table,
            "prodex-sqz",
            PathBuf::from("/tmp/sqz-mcp"),
            &["--transport", "stdio"],
            &[],
        );

        assert_eq!(
            table.get("mcp_servers").and_then(toml::Value::as_str),
            Some("invalid")
        );
    }
}
