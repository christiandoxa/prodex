use anyhow::{Context, Result};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::localization::localize_text_file;
use crate::toml_helpers::ensure_child_table;
use crate::{AGENTS_MD, PRODEX_SUPER_OPTIMIZER_AWARENESS, SUPER_OPTIMIZERS_MD};

mod claw;

pub const PRODEX_OPTIMIZERS_HOME_ENV: &str = "PRODEX_OPTIMIZERS_HOME";
const PRODEX_HOME_ENV: &str = "PRODEX_HOME";
const PRODEX_OPTIMIZERS_DIR_NAME: &str = "prodex-optimizers";
const CLAW_COMPACTOR_STATE_DIR_NAME: &str = "claw-compactor";
const SQZ_STATE_DIR_NAME: &str = "sqz";
const TOKEN_SAVIOR_STATE_DIR_NAME: &str = "token-savior";
const PRODEX_MEMORY_BACKEND_ENV: &str = "PRODEX_MEMORY_BACKEND";
const PRODEX_MEM0_API_URL_ENV: &str = "PRODEX_MEM0_API_URL";
const PRODEX_MEM0_API_KEY_ENV: &str = "PRODEX_MEM0_API_KEY";

#[derive(Debug, Clone, Copy, Default)]
pub struct SuperOptimizerMemoryConfig<'a> {
    pub enabled: bool,
    pub mem0_api_url: Option<&'a str>,
    pub mem0_api_key: Option<&'a str>,
}

impl SuperOptimizerMemoryConfig<'_> {
    fn backend_label(&self) -> &'static str {
        if !self.enabled {
            "disabled"
        } else if self.mem0_api_url.is_some() && self.mem0_api_key.is_some() {
            "managed Mem0"
        } else {
            "local sqlite"
        }
    }

    fn env_vars(&self) -> Vec<(&'static str, String)> {
        if !self.enabled {
            return Vec::new();
        }
        let (Some(api_url), Some(api_key)) = (self.mem0_api_url, self.mem0_api_key) else {
            return Vec::new();
        };
        vec![
            (PRODEX_MEMORY_BACKEND_ENV, "mem0".to_string()),
            (PRODEX_MEM0_API_URL_ENV, api_url.to_string()),
            (PRODEX_MEM0_API_KEY_ENV, api_key.to_string()),
        ]
    }
}

pub fn configure_super_optimizer_codex_home(codex_home: &Path) -> Result<()> {
    configure_super_optimizer_codex_home_with_presidio(codex_home, false)
}

pub fn configure_super_optimizer_codex_home_with_presidio(
    codex_home: &Path,
    presidio_enabled: bool,
) -> Result<()> {
    configure_super_optimizer_codex_home_with_options(
        codex_home,
        presidio_enabled,
        SuperOptimizerMemoryConfig::default(),
    )
}

pub fn configure_super_optimizer_codex_home_with_options(
    codex_home: &Path,
    presidio_enabled: bool,
    memory_config: SuperOptimizerMemoryConfig<'_>,
) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let path_dirs = path_dirs_from_env();
    let optimizer_roots = managed_optimizer_roots();
    let sqz_mcp_command = find_optimizer_command("sqz-mcp", &path_dirs, &optimizer_roots);
    let optimizers_path = codex_home.join(SUPER_OPTIMIZERS_MD);
    let awareness = render_super_optimizer_awareness(
        &path_dirs,
        &optimizer_roots,
        sqz_mcp_command.as_deref(),
        presidio_enabled,
        memory_config,
    );
    fs::write(&optimizers_path, awareness)
        .with_context(|| format!("failed to write {}", optimizers_path.display()))?;
    localize_text_file(&codex_home.join(AGENTS_MD))?;
    ensure_agents_reference(codex_home, &optimizers_path)?;
    configure_super_optimizer_mcp_servers_with_sources(
        codex_home,
        &path_dirs,
        &optimizer_roots,
        sqz_mcp_command.as_deref(),
        memory_config,
    )?;
    configure_super_optimizer_command_wrappers(
        codex_home,
        &path_dirs,
        &optimizer_roots,
        sqz_mcp_command.as_deref(),
    )?;
    claw::configure_session_hook(codex_home, &path_dirs, &optimizer_roots)
}

fn render_super_optimizer_awareness(
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
    sqz_mcp_command: Option<&Path>,
    presidio_enabled: bool,
    memory_config: SuperOptimizerMemoryConfig<'_>,
) -> String {
    let mut awareness = PRODEX_SUPER_OPTIMIZER_AWARENESS.to_string();
    awareness.push_str("\n## Available Now\n\n");
    awareness.push_str(&format!(
        "- rtk: {}\n",
        availability_label(find_path_command("rtk", path_dirs).as_deref())
    ));
    awareness.push_str(&format!(
        "- prodex-sqz MCP: {}\n",
        availability_label(sqz_mcp_command)
    ));
    awareness.push_str(&format!(
        "- prodex-token-savior MCP: {}\n",
        availability_label(
            find_optimizer_command("token-savior", path_dirs, optimizer_roots).as_deref()
        )
    ));
    awareness.push_str(&format!(
        "- prodex-claw-compactor: {}\n",
        availability_label(
            find_optimizer_command("claw-compactor", path_dirs, optimizer_roots).as_deref()
        )
    ));
    let memory_availability = if memory_config.enabled {
        availability_label(find_prodex_memory_command().as_deref())
    } else {
        "disabled".to_string()
    };
    awareness.push_str(&format!("- prodex-memory MCP: {memory_availability}\n"));
    awareness.push_str(&format!(
        "- prodex-inspect MCP: {}\n",
        availability_label(find_prodex_builtin_command().as_deref())
    ));
    awareness.push_str(&format!(
        "- prodex-memory backend: {}\n",
        memory_config.backend_label()
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

fn configure_super_optimizer_mcp_servers_with_sources(
    codex_home: &Path,
    path_dirs: &[PathBuf],
    optimizer_roots: &[PathBuf],
    sqz_mcp_command: Option<&Path>,
    memory_config: SuperOptimizerMemoryConfig<'_>,
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
    if let Some(command) = sqz_mcp_command {
        let sqz_state = optimizer_state_dirs_from_env(SQZ_STATE_DIR_NAME);
        let sqz_env = optimizer_state_env(sqz_state.as_ref());
        configure_stdio_mcp_server(
            &mut table,
            "prodex-sqz",
            command.to_path_buf(),
            &["--transport", "stdio"],
            &sqz_env,
        );
    }

    if let Some(command) = find_optimizer_command("token-savior", path_dirs, optimizer_roots) {
        let workspace_roots = env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let token_savior_state = token_savior_state_dirs_from_env();
        if let Some(state_dirs) = token_savior_state.as_ref() {
            write_token_savior_sitecustomize(state_dirs)?;
        }
        let token_savior_env = token_savior_mcp_env(&workspace_roots, token_savior_state.as_ref());
        configure_stdio_mcp_server(
            &mut table,
            "prodex-token-savior",
            command,
            &[],
            &token_savior_env,
        );
    }
    if memory_config.enabled
        && let Some(command) = find_prodex_memory_command()
    {
        let memory_env = memory_config.env_vars();
        configure_stdio_mcp_server(
            &mut table,
            "prodex-memory",
            command,
            &["__memory-mcp"],
            &memory_env,
        );
    }
    if let Some(command) = find_prodex_builtin_command() {
        configure_stdio_mcp_server(
            &mut table,
            "prodex-inspect",
            command,
            &["__inspect-mcp"],
            &[],
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
    sqz_mcp_command: Option<&Path>,
) -> Result<()> {
    let bin_dir = codex_home.join("bin");
    if let Some(command) = find_optimizer_command("sqz", path_dirs, optimizer_roots) {
        let sqz_state = optimizer_state_dirs_from_env(SQZ_STATE_DIR_NAME);
        let sqz_env = optimizer_state_env(sqz_state.as_ref());
        write_shell_wrapper_with_env(&bin_dir.join("sqz"), &command, &[], &sqz_env)?;
        write_shell_wrapper_with_env(&bin_dir.join("prodex-sqz-cli"), &command, &[], &sqz_env)?;
    }
    if let Some(command) = sqz_mcp_command {
        let sqz_state = optimizer_state_dirs_from_env(SQZ_STATE_DIR_NAME);
        let sqz_env = optimizer_state_env(sqz_state.as_ref());
        write_shell_wrapper_with_env(&bin_dir.join("sqz-mcp"), command, &[], &sqz_env)?;
    }
    claw::configure_command_wrappers(&bin_dir, path_dirs, optimizer_roots)
}

pub(super) fn write_shell_wrapper_with_env(
    path: &Path,
    command: &Path,
    args: &[&str],
    env_vars: &[(&str, String)],
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let args = args
        .iter()
        .map(|arg| format!(" '{}'", arg.replace('\'', "'\\''")))
        .collect::<String>();
    let exports = shell_exports(env_vars);
    let script = format!(
        "#!/usr/bin/env sh\n{}exec '{}'{} \"$@\"\n",
        exports,
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

pub(super) fn shell_exports(env_vars: &[(&str, String)]) -> String {
    env_vars
        .iter()
        .map(|(key, value)| format!("export {}='{}'\n", key, value.replace('\'', "'\\''")))
        .collect()
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
    python_path_dir: PathBuf,
    stats_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OptimizerStateDirs {
    home_dir: PathBuf,
    data_dir: PathBuf,
    config_dir: PathBuf,
    state_dir: PathBuf,
    cache_dir: PathBuf,
}

fn token_savior_mcp_env(
    workspace_roots: &str,
    state_dirs: Option<&TokenSaviorStateDirs>,
) -> Vec<(&'static str, String)> {
    let mut env_vars = vec![
        ("TOKEN_SAVIOR_CLIENT", "codex".to_string()),
        ("TOKEN_SAVIOR_PROFILE", "optimized".to_string()),
        ("TOKEN_SAVIOR_NO_WARMUP", "1".to_string()),
        ("TS_CAPTURE_DISABLED", "1".to_string()),
        ("TS_MEMORY_DISABLE", "1".to_string()),
        ("TS_AUTO_EXTRACT", "0".to_string()),
        ("WORKSPACE_ROOTS", workspace_roots.to_string()),
    ];
    if let Some(state_dirs) = state_dirs {
        env_vars.extend(optimizer_state_env(Some(&state_dirs.optimizer_dirs())));
        env_vars.push((
            "TOKEN_SAVIOR_CACHE_DIR",
            state_dirs.cache_dir.display().to_string(),
        ));
        env_vars.push((
            "PYTHONPATH",
            pythonpath_with_prepend(&state_dirs.python_path_dir),
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

pub(super) fn claw_compactor_state_env_from_env() -> Vec<(&'static str, String)> {
    optimizer_state_env(optimizer_state_dirs_from_env(CLAW_COMPACTOR_STATE_DIR_NAME).as_ref())
}

fn prodex_home_from_env() -> Option<PathBuf> {
    env::var_os(PRODEX_HOME_ENV)
        .map(PathBuf::from)
        .map(absolutize_path_lossy)
        .or_else(|| home_dir_from_env().map(|home| home.join(".prodex")))
}

fn optimizer_state_dirs_from_env(name: &str) -> Option<OptimizerStateDirs> {
    let prodex_home = prodex_home_from_env()?;
    Some(optimizer_state_dirs_from_prodex_home(&prodex_home, name))
}

fn optimizer_state_dirs_from_prodex_home(prodex_home: &Path, name: &str) -> OptimizerStateDirs {
    let root = prodex_home.join("optimizer-state").join(name);
    OptimizerStateDirs {
        home_dir: root.join("home"),
        data_dir: root.join("data"),
        config_dir: root.join("config"),
        state_dir: root.join("state"),
        cache_dir: root.join("cache"),
    }
}

fn optimizer_state_env(state_dirs: Option<&OptimizerStateDirs>) -> Vec<(&'static str, String)> {
    state_dirs
        .map(|state_dirs| {
            vec![
                ("HOME", state_dirs.home_dir.display().to_string()),
                ("XDG_DATA_HOME", state_dirs.data_dir.display().to_string()),
                (
                    "XDG_CONFIG_HOME",
                    state_dirs.config_dir.display().to_string(),
                ),
                ("XDG_STATE_HOME", state_dirs.state_dir.display().to_string()),
                ("XDG_CACHE_HOME", state_dirs.cache_dir.display().to_string()),
            ]
        })
        .unwrap_or_default()
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
    let optimizer_dirs =
        optimizer_state_dirs_from_prodex_home(prodex_home, TOKEN_SAVIOR_STATE_DIR_NAME);
    let cache_dir = optimizer_dirs.cache_dir.clone();
    TokenSaviorStateDirs {
        cache_dir,
        python_path_dir: optimizer_dirs.state_dir.join("python"),
        stats_dir: optimizer_dirs.state_dir.join("stats"),
    }
}

impl TokenSaviorStateDirs {
    fn optimizer_dirs(&self) -> OptimizerStateDirs {
        let root = self
            .cache_dir
            .parent()
            .expect("token-savior cache dir should have state root")
            .to_path_buf();
        OptimizerStateDirs {
            home_dir: root.join("home"),
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            state_dir: root.join("state"),
            cache_dir: self.cache_dir.clone(),
        }
    }
}

fn pythonpath_with_prepend(path: &Path) -> String {
    let mut paths = vec![path.to_path_buf()];
    if let Some(existing) = env::var_os("PYTHONPATH")
        && !existing.is_empty()
    {
        paths.extend(env::split_paths(&existing));
    }
    env::join_paths(paths)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.display().to_string())
}

fn write_token_savior_sitecustomize(state_dirs: &TokenSaviorStateDirs) -> Result<()> {
    fs::create_dir_all(&state_dirs.python_path_dir)
        .with_context(|| format!("failed to create {}", state_dirs.python_path_dir.display()))?;
    let sitecustomize_path = state_dirs.python_path_dir.join("sitecustomize.py");
    fs::write(&sitecustomize_path, TOKEN_SAVIOR_SITECUSTOMIZE)
        .with_context(|| format!("failed to write {}", sitecustomize_path.display()))?;
    Ok(())
}

const TOKEN_SAVIOR_SITECUSTOMIZE: &str = r#"""Prodex token-savior compatibility shims."""

import hashlib
import os
import re


def _patch_cache_manager():
    cache_dir = os.environ.get("TOKEN_SAVIOR_CACHE_DIR")
    if not cache_dir:
        return
    try:
        from token_savior.cache_ops import CacheManager
    except Exception:
        return

    def _workspace_cache_dir(root_path):
        absolute = os.path.abspath(os.path.expanduser(root_path))
        digest = hashlib.sha256(absolute.encode("utf-8", "surrogatepass")).hexdigest()[:16]
        name = os.path.basename(absolute.rstrip(os.sep)) or "workspace"
        slug = re.sub(r"[^A-Za-z0-9._-]+", "-", name).strip(".-") or "workspace"
        return os.path.join(os.path.expanduser(cache_dir), "projects", f"{slug}-{digest}")

    def path(self):
        project_cache_dir = _workspace_cache_dir(self.root_path)
        new_path = os.path.join(project_cache_dir, CacheManager.FILENAME)
        legacy = os.path.join(project_cache_dir, CacheManager.LEGACY_FILENAME)
        if not os.path.exists(new_path) and os.path.exists(legacy):
            try:
                os.makedirs(project_cache_dir, exist_ok=True)
                os.rename(legacy, new_path)
            except OSError:
                return legacy
        os.makedirs(project_cache_dir, exist_ok=True)
        return new_path

    CacheManager.path = path


_patch_cache_manager()
"#;

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
    matches!(
        command,
        "sqz" | "sqz-mcp" | "token-savior" | "claw-compactor"
    )
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
    for root in optimizer_roots {
        for candidate in managed_optimizer_command_candidates(root, command) {
            if optimizer_command_ready(command, &candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_prodex_memory_command() -> Option<PathBuf> {
    find_prodex_builtin_command()
}

fn find_prodex_builtin_command() -> Option<PathBuf> {
    env::current_exe().ok().filter(|path| executable_file(path))
}

fn optimizer_command_ready(command: &str, path: &Path) -> bool {
    executable_file(path)
        && match command {
            "sqz-mcp" => command_probe_success(path, &["--transport", "stdio", "--help"]),
            "token-savior" => token_savior_command_ready(path),
            _ => true,
        }
}

fn command_probe_success(path: &Path, args: &[&str]) -> bool {
    Command::new(path)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn token_savior_command_ready(path: &Path) -> bool {
    let Some(venv_root) = python_venv_root_for_command(path) else {
        return true;
    };
    python_venv_has_module(venv_root, "mcp")
}

fn python_venv_root_for_command(path: &Path) -> Option<&Path> {
    let bin_dir = path.parent()?;
    let venv_root = bin_dir.parent()?;
    let bin_name = bin_dir.file_name()?.to_str()?;
    let venv_name = venv_root.file_name()?.to_str()?;
    if matches!(bin_name, "bin" | "Scripts") && matches!(venv_name, ".venv" | "venv") {
        Some(venv_root)
    } else {
        None
    }
}

fn python_venv_has_module(venv_root: &Path, module_name: &str) -> bool {
    let windows_site_packages = venv_root.join("Lib").join("site-packages");
    if windows_site_packages.join(module_name).is_dir() {
        return true;
    }

    let lib_dir = venv_root.join("lib");
    let Ok(entries) = fs::read_dir(lib_dir) else {
        return false;
    };
    entries.filter_map(|entry| entry.ok()).any(|entry| {
        entry
            .path()
            .join("site-packages")
            .join(module_name)
            .is_dir()
    })
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
#[path = "super_optimizers_tests.rs"]
mod super_optimizers_tests;
