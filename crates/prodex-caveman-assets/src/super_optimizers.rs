use anyhow::{Context, Result};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::localization::localize_text_file;
use crate::toml_helpers::ensure_child_table;
use crate::{AGENTS_MD, PRODEX_SUPER_OPTIMIZER_AWARENESS, SUPER_OPTIMIZERS_MD};

mod claw;

pub const PRODEX_OPTIMIZERS_HOME_ENV: &str = "PRODEX_OPTIMIZERS_HOME";
const PRODEX_HOME_ENV: &str = "PRODEX_HOME";
const PRODEX_OPTIMIZERS_DIR_NAME: &str = "prodex-optimizers";
const TOKEN_SAVIOR_STATE_DIR_NAME: &str = "token-savior";

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
    let path_dirs = path_dirs_from_env();
    let optimizer_roots = managed_optimizer_roots();
    configure_super_optimizer_mcp_servers_with_sources(codex_home, &path_dirs, &optimizer_roots)?;
    configure_super_optimizer_command_wrappers(codex_home, &path_dirs, &optimizer_roots)?;
    claw::configure_session_hook(codex_home, &path_dirs, &optimizer_roots)
}

fn configure_super_optimizer_mcp_servers_with_sources(
    codex_home: &Path,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
) -> Result<()> {
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
    if let Some(command) = find_optimizer_command("sqz-mcp", path_dirs, optimizer_roots) {
        configure_stdio_mcp_server(
            &mut table,
            "prodex-sqz",
            command,
            &["--transport", "stdio"],
            &[],
        );
    }

    if let Some(command) = find_optimizer_command("token-savior", path_dirs, optimizer_roots) {
        let workspace_roots = env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let token_savior_state = token_savior_state_dirs_from_env();
        let token_savior_env = token_savior_mcp_env(&workspace_roots, token_savior_state.as_ref());
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

fn configure_super_optimizer_command_wrappers(
    codex_home: &Path,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
) -> Result<()> {
    let bin_dir = codex_home.join("bin");
    if let Some(command) = find_optimizer_command("sqz", path_dirs, optimizer_roots) {
        write_shell_wrapper(&bin_dir.join("sqz"), &command, &[])?;
        write_shell_wrapper(&bin_dir.join("prodex-sqz-cli"), &command, &[])?;
    }
    if let Some(command) = find_optimizer_command("sqz-mcp", path_dirs, optimizer_roots) {
        write_shell_wrapper(&bin_dir.join("sqz-mcp"), &command, &[])?;
    }
    claw::configure_command_wrappers(&bin_dir, path_dirs, optimizer_roots)
}

fn write_shell_wrapper(path: &Path, command: &Path, args: &[&str]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let args = args
        .iter()
        .map(|arg| format!(" '{}'", arg.replace('\'', "'\\''")))
        .collect::<String>();
    let script = format!(
        "#!/usr/bin/env sh\nexec '{}'{} \"$@\"\n",
        command.display().to_string().replace('\'', "'\\''"),
        args
    );
    fs::write(path, script).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

fn configure_stdio_mcp_server(
    table: &mut toml::Table,
    server_name: &str,
    command: PathBuf,
    args: &[&str],
    env_vars: &[(&str, String)],
) {
    let Some(mcp_servers) = super_optimizer_mcp_servers_table(table) else {
        return;
    };
    let is_new_server = !mcp_servers.contains_key(server_name);
    let server = if is_new_server {
        ensure_child_table(mcp_servers, server_name)
    } else {
        match mcp_servers.get_mut(server_name) {
            Some(toml::Value::Table(server)) => server,
            _ => return,
        }
    };
    if is_new_server {
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
    }
    if !env_vars.is_empty() {
        let env_table = ensure_child_table(server, "env");
        for (key, value) in env_vars {
            env_table.insert((*key).to_string(), toml::Value::String(value.clone()));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenSaviorStateDirs {
    cache_dir: PathBuf,
    stats_dir: PathBuf,
}

fn token_savior_mcp_env(
    workspace_roots: &str,
    state_dirs: Option<&TokenSaviorStateDirs>,
) -> Vec<(&'static str, String)> {
    let mut env_vars = vec![
        ("TOKEN_SAVIOR_CLIENT", "codex".to_string()),
        ("TOKEN_SAVIOR_PROFILE", "optimized".to_string()),
        ("TS_CAPTURE_DISABLED", "1".to_string()),
        ("TS_MEMORY_DISABLE", "1".to_string()),
        ("TS_AUTO_EXTRACT", "0".to_string()),
        ("WORKSPACE_ROOTS", workspace_roots.to_string()),
    ];
    if let Some(state_dirs) = state_dirs {
        env_vars.push((
            "TOKEN_SAVIOR_CACHE_DIR",
            state_dirs.cache_dir.display().to_string(),
        ));
        env_vars.push((
            "TOKEN_SAVIOR_STATS_DIR",
            state_dirs.stats_dir.display().to_string(),
        ));
    }
    env_vars
}

fn token_savior_state_dirs_from_env() -> Option<TokenSaviorStateDirs> {
    let prodex_home = env::var_os(PRODEX_HOME_ENV)
        .map(PathBuf::from)
        .map(absolutize_path_lossy)
        .or_else(|| home_dir_from_env().map(|home| home.join(".prodex")))?;
    Some(token_savior_state_dirs_from_prodex_home(&prodex_home))
}

fn absolutize_path_lossy(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    env::current_dir()
        .map(|current_dir| current_dir.join(&path))
        .unwrap_or(path)
}

fn token_savior_state_dirs_from_prodex_home(prodex_home: &Path) -> TokenSaviorStateDirs {
    let root = prodex_home
        .join("optimizer-state")
        .join(TOKEN_SAVIOR_STATE_DIR_NAME);
    TokenSaviorStateDirs {
        cache_dir: root.join("cache"),
        stats_dir: root.join("stats"),
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

fn find_optimizer_command(
    command: &str,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
) -> Option<PathBuf> {
    if prefer_managed_optimizer(command) {
        return find_managed_optimizer_command(command, optimizer_roots)
            .or_else(|| find_path_command(command, path_dirs));
    }
    find_path_command(command, path_dirs)
        .or_else(|| find_managed_optimizer_command(command, optimizer_roots))
}

fn prefer_managed_optimizer(command: &str) -> bool {
    matches!(command, "token-savior")
}

fn find_path_command(command: &str, path_dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in path_dirs {
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

fn find_managed_optimizer_command(command: &str, optimizer_roots: &[PathBuf]) -> Option<PathBuf> {
    for root in optimizer_roots {
        for candidate in managed_optimizer_command_candidates(root, command) {
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn path_dirs_from_env() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn managed_optimizer_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = env::var_os(PRODEX_OPTIMIZERS_HOME_ENV) {
        push_unique_path(&mut roots, PathBuf::from(path));
    }
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        push_unique_path(
            &mut roots,
            PathBuf::from(path).join(PRODEX_OPTIMIZERS_DIR_NAME),
        );
    }
    if let Some(home) = home_dir_from_env() {
        push_unique_path(
            &mut roots,
            home.join(".local")
                .join("share")
                .join(PRODEX_OPTIMIZERS_DIR_NAME),
        );
    }
    roots
}

fn home_dir_from_env() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn managed_optimizer_command_candidates(root: &Path, command: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    push_command_candidate(&mut candidates, root.join(command));
    match command {
        "sqz-mcp" => {
            push_sqz_workspace_candidates(&mut candidates, root, command);
        }
        "sqz" => {
            push_sqz_workspace_candidates(&mut candidates, root, command);
        }
        "token-savior" => {
            push_python_tool_candidates(&mut candidates, root, "token-savior", command);
        }
        "claw-compactor" => {
            push_python_tool_candidates(&mut candidates, root, "claw-compactor", command);
        }
        _ => {}
    }
    candidates
}

fn push_sqz_workspace_candidates(candidates: &mut Vec<PathBuf>, root: &Path, command: &str) {
    let checkout = root.join("sqz");
    push_command_candidate(candidates, checkout.join(command));
    push_command_candidate(
        candidates,
        checkout.join("target").join("release").join(command),
    );
    push_command_candidate(
        candidates,
        checkout.join("target").join("debug").join(command),
    );
}

fn push_python_tool_candidates(
    candidates: &mut Vec<PathBuf>,
    root: &Path,
    checkout_name: &str,
    command: &str,
) {
    let checkout = root.join(checkout_name);
    push_command_candidate(candidates, checkout.join(".venv").join("bin").join(command));
    push_command_candidate(candidates, checkout.join("venv").join("bin").join(command));
    push_command_candidate(candidates, checkout.join("bin").join(command));
    push_command_candidate(candidates, checkout.join(command));
}

fn push_command_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    candidates.push(path.clone());
    #[cfg(windows)]
    if path.extension().is_none() {
        if let Some(file_name) = path.file_name() {
            let mut exe_name = file_name.to_os_string();
            exe_name.push(".exe");
            candidates.push(path.with_file_name(exe_name));
        }
    }
}

fn executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!(
            "prodex-super-optimizers-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn command_path(path: PathBuf) -> PathBuf {
        #[cfg(windows)]
        if path.extension().is_none() {
            return path.with_extension("exe");
        }
        path
    }

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
    fn stdio_mcp_server_config_merges_env_into_existing_entry() {
        let mut table = toml::Table::new();
        let mcp_servers = ensure_child_table(&mut table, "mcp_servers");
        mcp_servers.insert(
            "prodex-token-savior".to_string(),
            toml::Value::Table({
                let mut table = toml::Table::new();
                table.insert(
                    "command".to_string(),
                    toml::Value::String("/custom/token-savior".to_string()),
                );
                table.insert(
                    "env".to_string(),
                    toml::Value::Table({
                        let mut env = toml::Table::new();
                        env.insert(
                            "CUSTOM_ENV".to_string(),
                            toml::Value::String("preserved".to_string()),
                        );
                        env.insert(
                            "TOKEN_SAVIOR_PROFILE".to_string(),
                            toml::Value::String("legacy".to_string()),
                        );
                        env
                    }),
                );
                table
            }),
        );

        configure_stdio_mcp_server(
            &mut table,
            "prodex-token-savior",
            PathBuf::from("/tmp/token-savior"),
            &[],
            &[
                ("TOKEN_SAVIOR_PROFILE", "optimized".to_string()),
                ("TOKEN_SAVIOR_CACHE_DIR", "/tmp/prodex/cache".to_string()),
            ],
        );

        let server = table
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .and_then(|servers| servers.get("prodex-token-savior"))
            .and_then(toml::Value::as_table)
            .expect("token-savior server should exist");
        assert_eq!(
            server.get("command").and_then(toml::Value::as_str),
            Some("/custom/token-savior")
        );
        let env = server
            .get("env")
            .and_then(toml::Value::as_table)
            .expect("token-savior env should exist");
        assert_eq!(
            env.get("CUSTOM_ENV").and_then(toml::Value::as_str),
            Some("preserved")
        );
        assert_eq!(
            env.get("TOKEN_SAVIOR_PROFILE")
                .and_then(toml::Value::as_str),
            Some("optimized")
        );
        assert_eq!(
            env.get("TOKEN_SAVIOR_CACHE_DIR")
                .and_then(toml::Value::as_str),
            Some("/tmp/prodex/cache")
        );
    }

    #[test]
    fn token_savior_mcp_env_routes_state_under_prodex_home() {
        let prodex_home = PathBuf::from("/tmp/prodex-home");
        let state_dirs = token_savior_state_dirs_from_prodex_home(&prodex_home);
        let env = token_savior_mcp_env("/workspace", Some(&state_dirs));
        let value_for = |key: &str| {
            env.iter()
                .find_map(|(candidate, value)| (*candidate == key).then_some(value.as_str()))
        };

        assert_eq!(value_for("WORKSPACE_ROOTS"), Some("/workspace"));
        assert_eq!(
            value_for("TOKEN_SAVIOR_CACHE_DIR"),
            Some("/tmp/prodex-home/optimizer-state/token-savior/cache")
        );
        assert_eq!(
            value_for("TOKEN_SAVIOR_STATS_DIR"),
            Some("/tmp/prodex-home/optimizer-state/token-savior/stats")
        );
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

    #[test]
    fn managed_optimizer_discovery_finds_sqz_workspace_release_binary() {
        let root = temp_dir("sqz-root");
        let command = command_path(
            root.join("sqz")
                .join("target")
                .join("release")
                .join("sqz-mcp"),
        );
        fs::create_dir_all(command.parent().expect("command parent should exist"))
            .expect("command parent should be created");
        fs::write(&command, "").expect("fake command should be written");

        assert_eq!(
            find_managed_optimizer_command("sqz-mcp", std::slice::from_ref(&root)),
            Some(command)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn token_savior_discovery_prefers_managed_venv_over_path() {
        let path_root = temp_dir("path-root");
        let optimizer_root = temp_dir("optimizer-root");
        let path_command = command_path(path_root.join("token-savior"));
        let managed_command = command_path(
            optimizer_root
                .join("token-savior")
                .join(".venv")
                .join("bin")
                .join("token-savior"),
        );
        fs::create_dir_all(path_command.parent().expect("path parent should exist"))
            .expect("path parent should be created");
        fs::create_dir_all(
            managed_command
                .parent()
                .expect("managed parent should exist"),
        )
        .expect("managed parent should be created");
        fs::write(&path_command, "").expect("path command should be written");
        fs::write(&managed_command, "").expect("managed command should be written");

        assert_eq!(
            find_optimizer_command(
                "token-savior",
                std::slice::from_ref(&path_root),
                std::slice::from_ref(&optimizer_root),
            ),
            Some(managed_command)
        );

        let _ = fs::remove_dir_all(path_root);
        let _ = fs::remove_dir_all(optimizer_root);
    }

    #[test]
    fn managed_token_savior_discovery_prefers_venv_binary() {
        let root = temp_dir("token-savior-root");
        let checkout_command = command_path(root.join("token-savior").join("token-savior"));
        let venv_command = command_path(
            root.join("token-savior")
                .join(".venv")
                .join("bin")
                .join("token-savior"),
        );
        fs::create_dir_all(
            checkout_command
                .parent()
                .expect("checkout parent should exist"),
        )
        .expect("checkout parent should be created");
        fs::create_dir_all(venv_command.parent().expect("venv parent should exist"))
            .expect("venv parent should be created");
        fs::write(&checkout_command, "").expect("checkout command should be written");
        fs::write(&venv_command, "").expect("venv command should be written");

        assert_eq!(
            find_managed_optimizer_command("token-savior", std::slice::from_ref(&root)),
            Some(venv_command)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mcp_config_registers_managed_sqz_when_path_is_empty() {
        let codex_home = temp_dir("codex-home");
        let optimizer_root = temp_dir("optimizer-root");
        let command = command_path(
            optimizer_root
                .join("sqz")
                .join("target")
                .join("release")
                .join("sqz-mcp"),
        );
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::create_dir_all(command.parent().expect("command parent should exist"))
            .expect("command parent should be created");
        fs::write(&command, "").expect("fake command should be written");

        configure_super_optimizer_mcp_servers_with_sources(
            &codex_home,
            &[],
            std::slice::from_ref(&optimizer_root),
        )
        .expect("super optimizer MCP config should write");

        let config =
            fs::read_to_string(codex_home.join("config.toml")).expect("config.toml should exist");
        let table = toml::from_str::<toml::Value>(&config).expect("config should parse");
        let command_value = table
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .and_then(|servers| servers.get("prodex-sqz"))
            .and_then(toml::Value::as_table)
            .and_then(|server| server.get("command"))
            .and_then(toml::Value::as_str);
        let expected_command = command.display().to_string();
        assert_eq!(command_value, Some(expected_command.as_str()));

        let _ = fs::remove_dir_all(codex_home);
        let _ = fs::remove_dir_all(optimizer_root);
    }
}
