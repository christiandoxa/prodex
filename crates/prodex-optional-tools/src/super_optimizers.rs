use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};

use crate::discovery;
use crate::fs_ops::{read_text_file_limited, write_text_file};
use crate::localization::ensure_agents_reference;
use crate::toml_helpers::ensure_child_table;
use crate::{
    OptionalToolId, OptionalToolSet, ToolActivation, ToolActivationPlan, resolve_optional_tools,
};
use crate::{PRODEX_SUPER_OPTIMIZER_AWARENESS, SUPER_OPTIMIZERS_MD};

mod ponytail;

const PRODEX_HOME_ENV: &str = "PRODEX_HOME";
const PLAYWRIGHT_MCP_PACKAGE: &str = "@playwright/mcp@0.0.78";

pub fn configure_super_optimizer_codex_home(codex_home: &Path) -> Result<()> {
    configure_super_optimizer_codex_home_with_presidio(codex_home, false)
}

pub fn configure_super_optimizer_codex_home_with_presidio(
    codex_home: &Path,
    presidio_enabled: bool,
) -> Result<()> {
    let selected = [
        OptionalToolId::Rtk,
        OptionalToolId::CodebaseMemoryMcp,
        OptionalToolId::Ponytail,
    ]
    .into_iter()
    .collect::<OptionalToolSet>();
    let plan = resolve_optional_tools(&selected, &OptionalToolSet::default());
    activate_optional_tools_for_codex(codex_home, &plan, presidio_enabled)
}

pub fn activate_optional_tools_for_codex(
    codex_home: &Path,
    plan: &ToolActivationPlan,
    presidio_enabled: bool,
) -> Result<()> {
    let selected = |id| {
        plan.activations
            .iter()
            .any(|activation| activation.tool.descriptor.id == id)
    };
    validate_optimizer_config_shapes(
        codex_home,
        selected(OptionalToolId::CodebaseMemoryMcp),
        selected(OptionalToolId::PlaywrightMcp),
        selected(OptionalToolId::Ponytail),
    )?;
    for activation in &plan.activations {
        match activation.tool.descriptor.id {
            OptionalToolId::Caveman => {
                crate::activate_caveman_for_codex(codex_home, &activation.tool)?;
            }
            OptionalToolId::Rtk => crate::rtk::configure_rtk_codex_home_with_command(
                codex_home,
                activation.tool.path.as_deref(),
            )?,
            _ => {}
        }
    }
    configure_selected_optimizer_codex_home(codex_home, &plan.activations, presidio_enabled)
}

fn validate_optimizer_config_shapes(
    codex_home: &Path,
    validate_codebase_memory: bool,
    validate_playwright: bool,
    validate_ponytail: bool,
) -> Result<()> {
    if !validate_codebase_memory && !validate_playwright && !validate_ponytail {
        return Ok(());
    }
    let config_path = codex_home.join("config.toml");
    let Some(contents) = read_text_file_limited(&config_path)? else {
        return Ok(());
    };
    if contents.trim().is_empty() {
        return Ok(());
    }
    let table = match toml::from_str::<toml::Value>(&contents)
        .with_context(|| format!("failed to parse {}", config_path.display()))?
    {
        toml::Value::Table(table) => table,
        _ => bail!("{} did not parse as a TOML table", config_path.display()),
    };

    if validate_ponytail {
        validate_ponytail_config_shapes(&table)?;
    }
    if !validate_codebase_memory && !validate_playwright {
        return Ok(());
    }
    let Some(mcp_servers) = table.get("mcp_servers") else {
        return Ok(());
    };
    let mcp_servers = match mcp_servers {
        toml::Value::Table(table) => table,
        _ => bail!("mcp_servers must be a TOML table"),
    };
    validate_selected_mcp_server_shapes(mcp_servers, validate_codebase_memory, validate_playwright)
}

fn validate_ponytail_config_shapes(table: &toml::Table) -> Result<()> {
    for key in ["features", "marketplaces", "plugins"] {
        if let Some(value) = table.get(key)
            && !matches!(value, toml::Value::Table(_))
        {
            bail!("configuration entry `{key}` must be a TOML table");
        }
    }
    if let Some(marketplaces) = table.get("marketplaces").and_then(toml::Value::as_table)
        && let Some(value) = marketplaces.get("ponytail")
        && !matches!(value, toml::Value::Table(_))
    {
        bail!("configuration entry `ponytail` must be a TOML table");
    }
    if let Some(plugins) = table.get("plugins").and_then(toml::Value::as_table)
        && let Some(value) = plugins.get("ponytail@ponytail")
        && !matches!(value, toml::Value::Table(_))
    {
        bail!("configuration entry `ponytail@ponytail` must be a TOML table");
    }
    Ok(())
}

fn validate_selected_mcp_server_shapes(
    mcp_servers: &toml::Table,
    validate_codebase_memory: bool,
    validate_playwright: bool,
) -> Result<()> {
    for (name, selected) in [
        ("codebase-memory-mcp", validate_codebase_memory),
        ("playwright", validate_playwright),
    ] {
        if selected
            && let Some(value) = mcp_servers.get(name)
            && !matches!(value, toml::Value::Table(_))
        {
            bail!("mcp_servers.{name} must be a TOML table");
        }
    }
    Ok(())
}

fn configure_selected_optimizer_codex_home(
    codex_home: &Path,
    activations: &[ToolActivation],
    presidio_enabled: bool,
) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let resolved_path = |id| {
        activations
            .iter()
            .find(|activation| activation.tool.descriptor.id == id)
            .and_then(|activation| activation.tool.path.as_deref())
    };
    let rtk_command = resolved_path(OptionalToolId::Rtk);
    let codebase_memory_command = resolved_path(OptionalToolId::CodebaseMemoryMcp);
    let npx_command = resolved_path(OptionalToolId::PlaywrightMcp);
    let ponytail = activations
        .iter()
        .find(|activation| activation.tool.descriptor.id == OptionalToolId::Ponytail);
    let ponytail_checkout = ponytail.and_then(|activation| activation.tool.path.as_deref());

    if rtk_command.is_none()
        && codebase_memory_command.is_none()
        && npx_command.is_none()
        && ponytail_checkout.is_none()
    {
        return Ok(());
    }

    let optimizers_path = codex_home.join(SUPER_OPTIMIZERS_MD);
    let awareness = render_super_optimizer_awareness(
        rtk_command,
        codebase_memory_command,
        npx_command,
        ponytail_checkout,
        presidio_enabled,
    );
    write_text_file(&optimizers_path, &awareness)?;
    ensure_agents_reference(codex_home, &optimizers_path)?;
    configure_super_mcp_servers(codex_home, codebase_memory_command, npx_command)?;
    if let Some(activation) = ponytail {
        ponytail::install_ponytail_plugin(codex_home, &activation.tool)?;
    }
    Ok(())
}

fn render_super_optimizer_awareness(
    rtk_command: Option<&Path>,
    codebase_memory_command: Option<&Path>,
    npx_command: Option<&Path>,
    ponytail_checkout: Option<&Path>,
    presidio_enabled: bool,
) -> String {
    let mut awareness = PRODEX_SUPER_OPTIMIZER_AWARENESS.to_string();
    awareness.push_str("\n## Available Now\n\n");
    awareness.push_str(&format!("- rtk: {}\n", availability_label(rtk_command)));
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
        )?;
    }
    if let Some(command) = npx_command {
        configure_default_playwright_mcp_server(&mut table, command)?;
    }
    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Super optimizer config overlay")?;
    write_text_file(&config_path, &rendered)
}

fn configure_default_playwright_mcp_server(table: &mut toml::Table, command: &Path) -> Result<()> {
    let mcp_servers = mcp_servers_table(table)?;
    match mcp_servers.get("playwright") {
        Some(toml::Value::Table(_)) => return Ok(()),
        Some(_) => bail!("mcp_servers.playwright must be a TOML table"),
        None => {}
    }
    let server = ensure_child_table(mcp_servers, "playwright")
        .with_context(|| "mcp_servers.playwright must be a TOML table")?;
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
    Ok(())
}

fn configure_stdio_mcp_server(
    table: &mut toml::Table,
    name: &str,
    command: PathBuf,
    args: &[String],
    env_vars: &[(&str, String)],
) -> Result<()> {
    let mcp_servers = mcp_servers_table(table)?;
    let server = ensure_child_table(mcp_servers, name)
        .with_context(|| format!("mcp_servers.{name} must be a TOML table"))?;
    configure_stdio_mcp_server_fields(server, command, args, env_vars);
    Ok(())
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

fn mcp_servers_table(table: &mut toml::Table) -> Result<&mut toml::Table> {
    if !table.contains_key("mcp_servers") {
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
    }
    match table.get_mut("mcp_servers") {
        Some(toml::Value::Table(table)) => Ok(table),
        _ => bail!("mcp_servers must be a TOML table"),
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

#[cfg(test)]
#[path = "super_optimizers_tests.rs"]
mod tests;
