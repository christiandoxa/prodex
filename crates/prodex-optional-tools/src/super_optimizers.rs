use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};

use crate::SUPER_OPTIMIZERS_MD;
use crate::fs_ops::{read_text_file_limited, write_text_file};
use crate::localization::ensure_agents_reference;
use crate::toml_helpers::ensure_child_table;
use crate::{
    OptionalToolId, OptionalToolSet, PLAYWRIGHT_MCP_PACKAGE, ToolActivation, ToolActivationPlan,
    resolve_optional_tools,
};
use prodex_mojo_core::optional_tools_policy::{
    OptionalToolConfigShapeIssue, optional_tool_config_shape_preflight,
};

mod ponytail;

pub fn configure_super_optimizer_codex_home(codex_home: &Path) -> Result<()> {
    configure_super_optimizer_codex_home_with_presidio(codex_home, false)
}

pub fn configure_super_optimizer_codex_home_with_presidio(
    codex_home: &Path,
    presidio_enabled: bool,
) -> Result<()> {
    let selected = OptionalToolSet::super_defaults();
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
    let required = |id| {
        plan.activations
            .iter()
            .any(|activation| activation.tool.descriptor.id == id && activation.required)
    };
    let resolved_path = |id| {
        plan.activations
            .iter()
            .find(|activation| activation.tool.descriptor.id == id)
            .and_then(|activation| activation.tool.path.as_deref())
    };
    let codebase_memory_command = resolved_path(OptionalToolId::CodebaseMemoryMcp);
    let npx_command = resolved_path(OptionalToolId::PlaywrightMcp);
    let config = validate_optimizer_config_shapes(
        codex_home,
        selected(OptionalToolId::CodebaseMemoryMcp),
        selected(OptionalToolId::PlaywrightMcp),
        selected(OptionalToolId::Ponytail),
    )?;
    validate_required_mcp_servers(
        config.as_ref(),
        required(OptionalToolId::CodebaseMemoryMcp),
        required(OptionalToolId::PlaywrightMcp),
        codebase_memory_command,
        npx_command,
        false,
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
) -> Result<Option<toml::Table>> {
    if !validate_codebase_memory && !validate_playwright && !validate_ponytail {
        return Ok(None);
    }
    let config_path = codex_home.join("config.toml");
    let Some(contents) = read_text_file_limited(&config_path)? else {
        return Ok(None);
    };
    if contents.trim().is_empty() {
        return Ok(Some(toml::Table::new()));
    }
    let table = match toml::from_str::<toml::Value>(&contents)
        .with_context(|| format!("failed to parse {}", config_path.display()))?
    {
        toml::Value::Table(table) => table,
        _ => bail!("{} did not parse as a TOML table", config_path.display()),
    };

    // The host acquires TOML types only. Mojo alone decides which
    // activated subtrees must be tables and which error takes precedence.
    let shape = |value: Option<&toml::Value>| match value {
        None => 0_i64,
        Some(toml::Value::Table(_)) => 1_i64,
        Some(_) => 2_i64,
    };
    let marketplaces = table.get("marketplaces").and_then(toml::Value::as_table);
    let plugins = table.get("plugins").and_then(toml::Value::as_table);
    let mcp_servers = table.get("mcp_servers").and_then(toml::Value::as_table);
    let shapes = [
        shape(table.get("features")),
        shape(table.get("marketplaces")),
        shape(table.get("plugins")),
        shape(marketplaces.and_then(|section| section.get("ponytail"))),
        shape(plugins.and_then(|section| section.get("ponytail@ponytail"))),
        shape(table.get("mcp_servers")),
        shape(mcp_servers.and_then(|section| section.get("codebase-memory-mcp"))),
        shape(mcp_servers.and_then(|section| section.get("playwright"))),
    ];
    let selected_mask = u8::from(validate_ponytail)
        | (u8::from(validate_codebase_memory) << 1)
        | (u8::from(validate_playwright) << 2);
    let issue = optional_tool_config_shape_preflight(selected_mask, shapes).map_err(|error| {
        anyhow::anyhow!("compiled Mojo optional-tool config preflight failed: {error:?}")
    })?;
    if let Some(issue) = issue {
        let message = match issue {
            OptionalToolConfigShapeIssue::Features => {
                "configuration entry `features` must be a TOML table"
            }
            OptionalToolConfigShapeIssue::Marketplaces => {
                "configuration entry `marketplaces` must be a TOML table"
            }
            OptionalToolConfigShapeIssue::Plugins => {
                "configuration entry `plugins` must be a TOML table"
            }
            OptionalToolConfigShapeIssue::PonytailMarketplace => {
                "configuration entry `ponytail` must be a TOML table"
            }
            OptionalToolConfigShapeIssue::PonytailPlugin => {
                "configuration entry `ponytail@ponytail` must be a TOML table"
            }
            OptionalToolConfigShapeIssue::McpServers => "mcp_servers must be a TOML table",
            OptionalToolConfigShapeIssue::CodebaseMemoryMcp => {
                "mcp_servers.codebase-memory-mcp must be a TOML table"
            }
            OptionalToolConfigShapeIssue::Playwright => {
                "mcp_servers.playwright must be a TOML table"
            }
        };
        bail!("{message}");
    }
    Ok(Some(table))
}

fn validate_required_mcp_servers(
    table: Option<&toml::Table>,
    required_codebase_memory: bool,
    required_playwright: bool,
    codebase_memory_command: Option<&Path>,
    npx_command: Option<&Path>,
    require_presence: bool,
) -> Result<()> {
    if !required_codebase_memory && !required_playwright {
        return Ok(());
    }
    if required_codebase_memory && codebase_memory_command.is_none() {
        bail!(
            "required MCP server `mcp_servers.codebase-memory-mcp` cannot be safely verified: resolved command is missing"
        );
    }
    if required_playwright && npx_command.is_none() {
        bail!(
            "required MCP server `mcp_servers.playwright` cannot be safely verified: resolved command is missing"
        );
    }

    let Some(table) = table else {
        if require_presence {
            bail!("required MCP server configuration is missing");
        }
        return Ok(());
    };
    let Some(mcp_servers) = table.get("mcp_servers") else {
        if require_presence {
            bail!("required MCP server configuration `mcp_servers` is missing");
        }
        return Ok(());
    };
    let Some(mcp_servers) = mcp_servers.as_table() else {
        bail!("mcp_servers must be a TOML table");
    };

    if required_codebase_memory {
        validate_required_mcp_server(mcp_servers, "codebase-memory-mcp", require_presence, || {
            codebase_memory_command
                    .and_then(mcp_jsonl_bridge_command_args)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "required MCP server `mcp_servers.codebase-memory-mcp` cannot be safely verified: Prodex MCP bridge is unavailable"
                        )
                    })
        })?;
    }
    if required_playwright {
        validate_required_mcp_server(mcp_servers, "playwright", require_presence, || {
            npx_command
                    .map(|command| (command.to_path_buf(), playwright_mcp_args().to_vec()))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "required MCP server `mcp_servers.playwright` cannot be safely verified: resolved command is missing"
                        )
                    })
        })?;
    }
    Ok(())
}

fn validate_required_mcp_server(
    mcp_servers: &toml::Table,
    name: &str,
    require_presence: bool,
    expected_command: impl FnOnce() -> Result<(PathBuf, Vec<String>)>,
) -> Result<()> {
    let Some(value) = mcp_servers.get(name) else {
        if require_presence {
            bail!("required MCP server `mcp_servers.{name}` is not configured");
        }
        return Ok(());
    };
    let Some(server) = value.as_table() else {
        bail!("mcp_servers.{name} must be a TOML table");
    };
    validate_required_mcp_server_enabled(name, server)?;
    let (command, args) = expected_command()?;
    validate_required_mcp_server_command(name, server, &command, &args)
}

fn validate_required_mcp_server_enabled(name: &str, server: &toml::Table) -> Result<()> {
    match server.get("enabled") {
        Some(toml::Value::Boolean(false)) => bail!(
            "required MCP server `mcp_servers.{name}` is disabled (`enabled = false`); refusing to override user configuration"
        ),
        Some(toml::Value::Boolean(true)) | None => Ok(()),
        Some(_) => bail!(
            "required MCP server `mcp_servers.{name}` cannot be safely verified: `enabled` must be a boolean"
        ),
    }
}

fn validate_required_mcp_server_command(
    name: &str,
    server: &toml::Table,
    command: &Path,
    args: &[String],
) -> Result<()> {
    let expected_command = command.display().to_string();
    let expected_args = toml::Value::Array(args.iter().cloned().map(toml::Value::String).collect());
    if server.get("command").and_then(toml::Value::as_str) != Some(expected_command.as_str())
        || server.get("args") != Some(&expected_args)
    {
        bail!(
            "required MCP server `mcp_servers.{name}` cannot be safely verified: inherited configuration is custom or incomplete; refusing to replace user configuration"
        );
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
    let required = |id| {
        activations
            .iter()
            .any(|activation| activation.tool.descriptor.id == id && activation.required)
    };

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
    configure_super_mcp_servers(
        codex_home,
        codebase_memory_command,
        npx_command,
        required(OptionalToolId::CodebaseMemoryMcp),
        required(OptionalToolId::PlaywrightMcp),
    )?;
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
    let paths = [
        rtk_command,
        codebase_memory_command,
        npx_command,
        ponytail_checkout,
    ]
    .map(|path| path.map(|path| path.display().to_string()));
    prodex_mojo_core::optional_tools_policy::render_optional_tool_awareness(
        [
            paths[0].as_deref(),
            paths[1].as_deref(),
            paths[2].as_deref(),
            paths[3].as_deref(),
        ],
        presidio_enabled,
    )
    .expect("Mojo optional-tool awareness renderer failed")
}

fn configure_super_mcp_servers(
    codex_home: &Path,
    codebase_memory_command: Option<&Path>,
    npx_command: Option<&Path>,
    required_codebase_memory: bool,
    required_playwright: bool,
) -> Result<()> {
    if required_codebase_memory && codebase_memory_command.is_none() {
        bail!(
            "required MCP server `mcp_servers.codebase-memory-mcp` cannot be safely verified: resolved command is missing"
        );
    }
    if required_playwright && npx_command.is_none() {
        bail!(
            "required MCP server `mcp_servers.playwright` cannot be safely verified: resolved command is missing"
        );
    }
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
    let codebase_memory = codebase_memory_command.and_then(mcp_jsonl_bridge_command_args);
    if required_codebase_memory && codebase_memory.is_none() {
        bail!(
            "required MCP server `mcp_servers.codebase-memory-mcp` cannot be safely verified: Prodex MCP bridge is unavailable"
        );
    }
    validate_required_mcp_servers(
        Some(&table),
        required_codebase_memory,
        required_playwright,
        codebase_memory_command,
        npx_command,
        false,
    )?;
    if let Some((bridge, bridge_args)) = codebase_memory {
        configure_stdio_mcp_server(&mut table, "codebase-memory-mcp", bridge, &bridge_args, &[])?;
    }
    if let Some(command) = npx_command {
        configure_default_playwright_mcp_server(&mut table, command)?;
    }
    validate_required_mcp_servers(
        Some(&table),
        required_codebase_memory,
        required_playwright,
        codebase_memory_command,
        npx_command,
        true,
    )?;
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
    let args = playwright_mcp_args();
    configure_stdio_mcp_server_fields(server, command.to_path_buf(), &args, &[]);
    server.insert("enabled".to_string(), toml::Value::Boolean(true));
    server.insert("startup_timeout_sec".to_string(), toml::Value::Integer(60));
    server.insert(
        "default_tools_approval_mode".to_string(),
        toml::Value::String("writes".to_string()),
    );
    Ok(())
}

fn playwright_mcp_args() -> [String; 4] {
    [
        "--no-install".to_string(),
        PLAYWRIGHT_MCP_PACKAGE.to_string(),
        "--headless".to_string(),
        "--isolated".to_string(),
    ]
}

fn configure_stdio_mcp_server(
    table: &mut toml::Table,
    name: &str,
    command: PathBuf,
    args: &[String],
    env_vars: &[(&str, String)],
) -> Result<()> {
    let mcp_servers = mcp_servers_table(table)?;
    match mcp_servers.get(name) {
        Some(toml::Value::Table(_)) => return Ok(()),
        Some(_) => bail!("mcp_servers.{name} must be a TOML table"),
        None => {}
    }
    let server = ensure_child_table(mcp_servers, name)
        .with_context(|| format!("mcp_servers.{name} must be a TOML table"))?;
    configure_stdio_mcp_server_fields(server, command, args, env_vars);
    server.insert("enabled".to_string(), toml::Value::Boolean(true));
    server.insert("startup_timeout_sec".to_string(), toml::Value::Integer(60));
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
