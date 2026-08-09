use super::*;
use crate::{RTK_MD, ResolvedTool, ToolDiscoverySource, optional_tool_descriptor};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir()
        .canonicalize()
        .expect("temp dir should resolve")
        .join(format!("prodex-{name}-{}-{stamp}", std::process::id()))
}

#[test]
fn configures_only_codebase_memory_server() -> Result<()> {
    let mut table = toml::Table::new();
    configure_stdio_mcp_server(
        &mut table,
        "codebase-memory-mcp",
        PathBuf::from("/bin/prodex"),
        &[
            "__mcp-jsonl-bridge".into(),
            "/bin/codebase-memory-mcp".into(),
        ],
        &[("CBM_CACHE_DIR", "/tmp/cbm".into())],
    )?;

    let rendered = toml::to_string(&table).unwrap();
    assert!(rendered.contains("[mcp_servers.codebase-memory-mcp]"));
    assert!(rendered.contains("CBM_CACHE_DIR = \"/tmp/cbm\""));
    for removed in [
        "prodex-sqz",
        "token-savior",
        "claw-compactor",
        "prodex-memory",
        "prodex-inspect",
    ] {
        assert!(!rendered.contains(removed));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn codebase_memory_cache_ancestors_are_private() -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let home = temp_dir("super-codebase-memory-cache");
    let optimizer_state = home.join("optimizer-state");
    let codebase_memory = optimizer_state.join("codebase-memory");
    let cache_dir = codebase_memory.join("cache");
    fs::create_dir_all(&cache_dir)?;
    for path in [&optimizer_state, &codebase_memory, &cache_dir] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o775))?;
    }

    assert_eq!(prepare_codebase_memory_cache_dir(&home)?, cache_dir);
    for path in [&optimizer_state, &codebase_memory, &cache_dir] {
        assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o700);
    }

    fs::remove_dir_all(home)?;
    Ok(())
}

#[test]
fn configures_playwright_default() {
    let mut table = toml::Table::new();
    configure_default_playwright_mcp_server(&mut table, Path::new("/bin/npx")).unwrap();

    let servers = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .unwrap();
    let playwright = servers
        .get("playwright")
        .and_then(toml::Value::as_table)
        .unwrap();
    assert_eq!(
        playwright.get("command").and_then(toml::Value::as_str),
        Some("/bin/npx")
    );
    assert_eq!(
        playwright
            .get("args")
            .and_then(toml::Value::as_array)
            .unwrap()
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>(),
        [
            "--no-install",
            PLAYWRIGHT_MCP_PACKAGE,
            "--headless",
            "--isolated"
        ]
    );
    assert_eq!(
        playwright
            .get("default_tools_approval_mode")
            .and_then(toml::Value::as_str),
        Some("writes")
    );
    assert_eq!(
        playwright
            .get("startup_timeout_sec")
            .and_then(toml::Value::as_integer),
        Some(60)
    );
    assert_eq!(
        playwright.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(!playwright.contains_key("required"));
}

#[test]
fn activation_pins_the_validated_rtk_path_in_the_overlay() -> Result<()> {
    let root = temp_dir("super-managed-rtk");
    let home = root.join("overlay");
    let managed = root.join("managed tools");
    fs::create_dir_all(&managed)?;
    let rtk = managed.join(if cfg!(windows) { "rtk.exe" } else { "rtk" });
    fs::write(&rtk, "validated fixture")?;
    let plan = ToolActivationPlan {
        activations: vec![ToolActivation {
            tool: crate::ResolvedTool {
                descriptor: crate::optional_tool_descriptor(OptionalToolId::Rtk),
                source: crate::ToolDiscoverySource::ManagedRoot,
                path: Some(rtk.clone()),
                version: Some("test".to_string()),
                digest: Some("sha256:test".to_string()),
            },
            required: false,
        }],
        unavailable: Vec::new(),
    };

    activate_optional_tools_for_codex(&home, &plan, false)?;

    let wrapper = home
        .join("bin")
        .join(if cfg!(windows) { "rtk.cmd" } else { "rtk" });
    let script = fs::read_to_string(wrapper)?;
    assert!(script.contains(&rtk.display().to_string()));
    assert!(!script.contains("not installed"));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn preserves_user_playwright_server() {
    let mut table = toml::from_str::<toml::Table>(
        r#"
[mcp_servers.playwright]
command = "custom-playwright"
args = ["--headed"]
"#,
    )
    .unwrap();

    configure_default_playwright_mcp_server(&mut table, Path::new("/bin/npx")).unwrap();

    let rendered = toml::to_string(&table).unwrap();
    assert!(rendered.contains("command = \"custom-playwright\""));
    assert!(rendered.contains("args = [\"--headed\"]"));
    assert!(!rendered.contains(PLAYWRIGHT_MCP_PACKAGE));
}

#[test]
fn rejects_non_table_mcp_servers_and_nested_entries() {
    let mut table = toml::Table::from_iter([(
        "mcp_servers".to_string(),
        toml::Value::String("invalid".to_string()),
    )]);
    let error = configure_stdio_mcp_server(
        &mut table,
        "codebase-memory-mcp",
        PathBuf::from("/bin/prodex"),
        &[],
        &[],
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("mcp_servers must be a TOML table")
    );

    let mut table = toml::Table::new();
    table.insert(
        "mcp_servers".to_string(),
        toml::Value::Table(toml::Table::from_iter([(
            "playwright".to_string(),
            toml::Value::String("invalid".to_string()),
        )])),
    );
    let error =
        configure_default_playwright_mcp_server(&mut table, Path::new("/bin/npx")).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("mcp_servers.playwright must be a TOML table")
    );
}

#[test]
fn optimizer_preflight_rejects_wrong_shapes_before_mutation() -> Result<()> {
    let cases = [
        ("features = \"invalid\"\n", "features"),
        ("marketplaces = []\n", "marketplaces"),
        ("plugins = false\n", "plugins"),
        ("mcp_servers = \"invalid\"\n", "mcp_servers"),
        ("[marketplaces]\nponytail = \"invalid\"\n", "ponytail"),
        (
            "[plugins]\n\"ponytail@ponytail\" = \"invalid\"\n",
            "ponytail@ponytail",
        ),
        (
            "[mcp_servers]\nplaywright = \"invalid\"\n",
            "mcp_servers.playwright",
        ),
    ];
    let activations: Vec<ToolActivation> = [
        OptionalToolId::Rtk,
        OptionalToolId::CodebaseMemoryMcp,
        OptionalToolId::PlaywrightMcp,
        OptionalToolId::Ponytail,
    ]
    .into_iter()
    .map(|id| ToolActivation {
        tool: ResolvedTool {
            descriptor: optional_tool_descriptor(id),
            source: ToolDiscoverySource::ManagedRoot,
            path: None,
            version: None,
            digest: None,
        },
        required: false,
    })
    .collect();

    for (index, (contents, expected)) in cases.into_iter().enumerate() {
        let home = temp_dir(&format!("optimizer-preflight-{index}"));
        fs::create_dir_all(&home)?;
        fs::write(home.join("config.toml"), contents)?;
        let error = activate_optional_tools_for_codex(
            &home,
            &ToolActivationPlan {
                activations: activations.clone(),
                unavailable: Vec::new(),
            },
            false,
        )
        .unwrap_err();

        assert!(error.to_string().contains(expected));
        assert_eq!(fs::read_to_string(home.join("config.toml"))?, contents);
        for relative in [
            "AGENTS.md",
            RTK_MD,
            SUPER_OPTIMIZERS_MD,
            "bin",
            ".tmp/marketplaces/ponytail",
            "plugins/cache/ponytail",
        ] {
            assert!(
                !home.join(relative).exists(),
                "unexpected mutation: {relative}"
            );
        }
        fs::remove_dir_all(home)?;
    }
    Ok(())
}

#[test]
fn optimizer_preflight_ignores_unselected_config_sections() -> Result<()> {
    let home = temp_dir("optimizer-preflight-unselected");
    fs::create_dir_all(&home)?;
    fs::write(
        home.join("config.toml"),
        "features = \"unused\"\nmcp_servers = \"unused\"\n",
    )?;

    validate_optimizer_config_shapes(&home, false, false, false)?;

    fs::remove_dir_all(home)?;
    Ok(())
}

#[test]
fn optimizer_preflight_scopes_mcp_children_independently() -> Result<()> {
    let home = temp_dir("optimizer-preflight-mcp-scope");
    fs::create_dir_all(&home)?;
    fs::write(
        home.join("config.toml"),
        "[mcp_servers]\ncodebase-memory-mcp = \"invalid\"\nplaywright = {}\n",
    )?;
    validate_optimizer_config_shapes(&home, false, true, false)?;
    assert!(validate_optimizer_config_shapes(&home, true, false, false).is_err());

    fs::write(
        home.join("config.toml"),
        "[mcp_servers]\ncodebase-memory-mcp = {}\nplaywright = \"invalid\"\n",
    )?;
    validate_optimizer_config_shapes(&home, true, false, false)?;
    assert!(validate_optimizer_config_shapes(&home, false, true, false).is_err());

    fs::remove_dir_all(home)?;
    Ok(())
}

#[test]
fn optimizer_preflight_validates_only_activated_tools() -> Result<()> {
    let home = temp_dir("optimizer-preflight-activation-only");
    fs::create_dir_all(&home)?;
    let config = "mcp_servers = \"invalid\"\n";
    fs::write(home.join("config.toml"), config)?;

    activate_optional_tools_for_codex(
        &home,
        &ToolActivationPlan {
            activations: Vec::new(),
            unavailable: vec![crate::ToolHealth {
                id: OptionalToolId::CodebaseMemoryMcp,
                status: crate::ToolHealthStatus::Missing,
                source: None,
                path: None,
                version: None,
                digest: None,
                can_activate: false,
                detail: "optional fixture unavailable".to_string(),
            }],
        },
        false,
    )?;
    assert_eq!(fs::read_to_string(home.join("config.toml"))?, config);

    let error = activate_optional_tools_for_codex(
        &home,
        &ToolActivationPlan {
            activations: vec![ToolActivation {
                tool: ResolvedTool {
                    descriptor: optional_tool_descriptor(OptionalToolId::CodebaseMemoryMcp),
                    source: ToolDiscoverySource::ManagedRoot,
                    path: None,
                    version: None,
                    digest: None,
                },
                required: false,
            }],
            unavailable: Vec::new(),
        },
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("mcp_servers"));

    fs::remove_dir_all(home)?;
    Ok(())
}

#[test]
fn agents_reference_is_idempotent() -> Result<()> {
    let home = temp_dir("super-agents-reference");
    fs::create_dir_all(&home)?;
    let reference = home.join(SUPER_OPTIMIZERS_MD);
    ensure_agents_reference(&home, &reference)?;
    ensure_agents_reference(&home, &reference)?;

    let agents = fs::read_to_string(home.join("AGENTS.md"))?;
    assert_eq!(agents.lines().count(), 1);
    assert_eq!(agents, format!("@{}\n", reference.display()));
    fs::remove_dir_all(home)?;
    Ok(())
}

#[test]
fn awareness_contains_only_the_minimal_stack() {
    let awareness = render_super_optimizer_awareness(
        Some(Path::new("/bin/rtk")),
        Some(Path::new("/bin/codebase-memory-mcp")),
        Some(Path::new("/bin/npx")),
        Some(Path::new("/tmp/ponytail")),
        true,
    );
    for kept in [
        "rtk",
        "codebase-memory-mcp",
        "playwright-mcp",
        "ponytail",
        "presidio",
    ] {
        assert!(awareness.contains(kept));
    }
    assert!(awareness.contains("index_repository"));
    for removed in [
        "prodex-sqz",
        "token-savior",
        "claw-compactor",
        "prodex-memory",
        "prodex-inspect",
        "Mem0",
    ] {
        assert!(!awareness.contains(removed));
    }
}
