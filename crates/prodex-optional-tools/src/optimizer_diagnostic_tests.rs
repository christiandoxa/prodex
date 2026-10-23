use super::*;
use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestHome(PathBuf);

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn optimizer_shape_diagnostics_preserve_exact_messages_without_config_writes() -> Result<()> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    // macOS temp roots can use /var -> /private/var. Resolve the trusted
    // test root before invoking production's deliberate no-symlink guard.
    let home = TestHome(env::temp_dir().canonicalize()?.join(format!(
        "prodex-optimizer-diagnostic-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir_all(&home.0)?;
    for (config, expected) in [
        (
            "features = 7",
            "configuration entry `features` must be a TOML table",
        ),
        (
            "marketplaces = 7",
            "configuration entry `marketplaces` must be a TOML table",
        ),
        (
            "plugins = 7",
            "configuration entry `plugins` must be a TOML table",
        ),
        (
            "[marketplaces]\nponytail = 7",
            "configuration entry `ponytail` must be a TOML table",
        ),
        (
            "[plugins]\n\"ponytail@ponytail\" = 7",
            "configuration entry `ponytail@ponytail` must be a TOML table",
        ),
        ("mcp_servers = 7", "mcp_servers must be a TOML table"),
        (
            "[mcp_servers]\ncodebase-memory-mcp = 7",
            "mcp_servers.codebase-memory-mcp must be a TOML table",
        ),
        (
            "[mcp_servers]\nplaywright = 7",
            "mcp_servers.playwright must be a TOML table",
        ),
    ] {
        std::fs::write(home.0.join("config.toml"), config)?;
        let error = validate_optimizer_config_shapes(&home.0, true, true, true).unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(std::fs::read_to_string(home.0.join("config.toml"))?, config);
        assert_eq!(std::fs::read_dir(&home.0)?.count(), 1);
    }
    Ok(())
}

#[test]
fn required_mcp_diagnostics_preserve_exact_names_and_enabled_preflight_priority() {
    for name in ["codebase-memory-mcp", "playwright", "fixture-東京"] {
        for (enabled, suffix, expected_resolutions) in [
            (
                toml::Value::Boolean(false),
                "is disabled (`enabled = false`); refusing to override user configuration",
                0,
            ),
            (
                toml::Value::String("invalid".into()),
                "cannot be safely verified: `enabled` must be a boolean",
                0,
            ),
            (
                toml::Value::Boolean(true),
                "cannot be safely verified: inherited configuration is custom or incomplete; refusing to replace user configuration",
                1,
            ),
        ] {
            let servers = toml::Table::from_iter([(
                name.to_string(),
                toml::Value::Table(toml::Table::from_iter([
                    ("enabled".into(), enabled),
                    ("command".into(), toml::Value::String("different".into())),
                ])),
            )]);
            let resolutions = Cell::new(0);
            let error = validate_required_mcp_server(&servers, name, true, || {
                resolutions.set(resolutions.get() + 1);
                Ok((PathBuf::from("fixture-command"), vec!["serve".into()]))
            })
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("required MCP server `mcp_servers.{name}` {suffix}")
            );
            assert_eq!(resolutions.get(), expected_resolutions);
        }
    }
}
