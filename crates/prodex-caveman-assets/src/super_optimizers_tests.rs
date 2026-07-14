use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("prodex-{name}-{}-{stamp}", std::process::id()))
}

#[test]
fn configures_only_codebase_memory_server() {
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
    );

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
}

#[test]
fn configures_playwright_default() {
    let mut table = toml::Table::new();
    configure_default_playwright_mcp_server(&mut table, Path::new("/bin/npx"));

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
        ["-y", PLAYWRIGHT_MCP_PACKAGE, "--headless", "--isolated"]
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

    configure_default_playwright_mcp_server(&mut table, Path::new("/bin/npx"));

    let rendered = toml::to_string(&table).unwrap();
    assert!(rendered.contains("command = \"custom-playwright\""));
    assert!(rendered.contains("args = [\"--headed\"]"));
    assert!(!rendered.contains(PLAYWRIGHT_MCP_PACKAGE));
}

#[test]
fn playwright_requires_node_18_or_newer() {
    assert_eq!(node_major_version(b"v18.0.0\n"), Some(18));
    assert_eq!(node_major_version(b"22.14.0\n"), Some(22));
    assert_eq!(node_major_version(b"not-a-version\n"), None);
}

#[test]
fn agents_reference_is_idempotent() -> Result<()> {
    let home = temp_dir("super-agents-reference");
    fs::create_dir_all(&home)?;
    let reference = home.join(SUPER_OPTIMIZERS_MD);
    ensure_agents_reference(&home, &reference)?;
    ensure_agents_reference(&home, &reference)?;

    let agents = fs::read_to_string(home.join(AGENTS_MD))?;
    assert_eq!(agents.lines().count(), 1);
    assert_eq!(agents, format!("@{}\n", reference.display()));
    fs::remove_dir_all(home)?;
    Ok(())
}

#[test]
fn awareness_contains_only_the_minimal_stack() {
    let awareness = render_super_optimizer_awareness(
        &[],
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
