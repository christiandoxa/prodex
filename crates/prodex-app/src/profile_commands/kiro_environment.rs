use anyhow::{Context, Result};
use dirs::{data_local_dir, home_dir};
use prodex_mojo_core::kiro_environment::{
    KiroMcpDisabledToolsState, KiroMcpNormalizationAction, KiroMcpServerFacts,
    plan_mcp_normalization,
};
use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const KIRO_MCP_CONFIG_MAX_BYTES: u64 = 1024 * 1024;
const KIRO_CODEBASE_MEMORY_INCOMPATIBLE_TOOL: &str = "check_index_coverage";

pub(super) fn discover_kiro_database_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("KIRO_TEST_DB_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    for variable in ["KIRO_DATA_DIR", "Q_CLI_DATA_DIR"] {
        if let Some(path) = env::var_os(variable) {
            let candidate = PathBuf::from(path).join("data.sqlite3");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    let mut candidates = Vec::new();
    if let Some(data_dir) = data_local_dir() {
        candidates.push(data_dir.join("kiro-cli").join("data.sqlite3"));
        candidates.push(data_dir.join("amazon-q").join("data.sqlite3"));
    }
    if let Some(home) = home_dir() {
        for name in ["kiro-cli", "amazon-q"] {
            candidates.push(
                home.join(".local")
                    .join("share")
                    .join(name)
                    .join("data.sqlite3"),
            );
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .context("failed to find Kiro auth database; expected ~/.local/share/kiro-cli/data.sqlite3 or ~/.local/share/amazon-q/data.sqlite3")
}

pub(crate) fn kiro_cli_data_dir_env(data_dir: &Path) -> Vec<(OsString, OsString)> {
    let value = data_dir.as_os_str().to_os_string();
    // Native Kiro uses the direct database override; keep both directory variables for older
    // Kiro and Amazon Q builds.
    vec![
        (OsString::from("KIRO_DATA_DIR"), value.clone()),
        (OsString::from("Q_CLI_DATA_DIR"), value),
        (
            OsString::from("KIRO_TEST_DB_PATH"),
            data_dir.join("data.sqlite3").into_os_string(),
        ),
    ]
}

pub(crate) fn ensure_kiro_codebase_memory_compatibility() -> Result<()> {
    let Some(kiro_home) = env::var_os("KIRO_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".kiro")))
    else {
        return Ok(());
    };
    normalize_kiro_codebase_memory_config(&kiro_home.join("settings").join("mcp.json"))?;
    Ok(())
}

fn normalize_kiro_codebase_memory_config(path: &Path) -> Result<bool> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    if metadata.len() > KIRO_MCP_CONFIG_MAX_BYTES {
        anyhow::bail!("{} exceeds the 1 MiB Kiro MCP config limit", path.display());
    }
    let mut config: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    let Some(servers) = config.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let facts = servers
        .iter()
        .map(|(name, server)| {
            let Some(server) = server.as_object() else {
                return Ok(KiroMcpServerFacts::default());
            };
            let server_name_matches = ascii_casefold_equal_exact(name, "codebase-memory-mcp")
                .map_err(|error| {
                    anyhow::anyhow!("Mojo Kiro MCP name comparison failed: {error:?}")
                })?;
            let disabled_tools_state = match server.get("disabledTools") {
                None => KiroMcpDisabledToolsState::Missing,
                Some(Value::Null) => KiroMcpDisabledToolsState::Null,
                Some(Value::Array(_)) => KiroMcpDisabledToolsState::Array,
                Some(_) => KiroMcpDisabledToolsState::Invalid,
            };
            let incompatible_tool_present = server
                .get("disabledTools")
                .and_then(Value::as_array)
                .is_some_and(|disabled| {
                    disabled
                        .iter()
                        .any(|tool| tool.as_str() == Some(KIRO_CODEBASE_MEMORY_INCOMPATIBLE_TOOL))
                });
            let command_name_matches = server
                .get("command")
                .and_then(Value::as_str)
                .and_then(|command| Path::new(command).file_name())
                .and_then(|name| name.to_str())
                .map(|name| {
                    ascii_casefold_equal_exact(name, "codebase-memory-mcp")
                        .map_err(|error| {
                            anyhow::anyhow!("Mojo Kiro MCP executable comparison failed: {error:?}")
                        })
                        .and_then(|matches| {
                            if matches {
                                Ok(true)
                            } else {
                                ascii_casefold_equal_exact(name, "codebase-memory-mcp.exe").map_err(
                                    |error| {
                                        anyhow::anyhow!(
                                            "Mojo Kiro MCP executable comparison failed: {error:?}"
                                        )
                                    },
                                )
                            }
                        })
                })
                .transpose()?
                .unwrap_or(false);
            Ok(KiroMcpServerFacts {
                server_name_matches: i64::from(server_name_matches),
                command_name_matches: i64::from(command_name_matches),
                disabled_tools_state,
                incompatible_tool_present: i64::from(incompatible_tool_present),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let actions = plan_mcp_normalization(&facts)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro MCP normalization failed: {error:?}"))?;
    let mut changed = false;
    for ((name, server), action) in servers.iter_mut().zip(actions) {
        let Some(server) = server.as_object_mut() else {
            continue;
        };
        match action {
            KiroMcpNormalizationAction::Keep => {}
            KiroMcpNormalizationAction::InitializeDisabledTools => {
                // Mojo's initialization action means the compatibility tool is the first entry.
                server.insert(
                    "disabledTools".to_owned(),
                    Value::Array(vec![Value::String(
                        KIRO_CODEBASE_MEMORY_INCOMPATIBLE_TOOL.to_string(),
                    )]),
                );
                changed = true;
            }
            KiroMcpNormalizationAction::AppendIncompatibleTool => {
                let disabled = server
                    .get_mut("disabledTools")
                    .and_then(Value::as_array_mut)
                    .with_context(|| {
                        format!(
                            "{}.mcpServers.{name}.disabledTools must be an array",
                            path.display()
                        )
                    })?;
                disabled.push(Value::String(
                    KIRO_CODEBASE_MEMORY_INCOMPATIBLE_TOOL.to_string(),
                ));
                changed = true;
            }
            KiroMcpNormalizationAction::InvalidDisabledTools => {
                server
                    .get_mut("disabledTools")
                    .and_then(Value::as_array_mut)
                    .with_context(|| {
                        format!(
                            "{}.mcpServers.{name}.disabledTools must be an array",
                            path.display()
                        )
                    })?;
            }
        }
    }
    if changed {
        let mut encoded = serde_json::to_vec_pretty(&config)
            .context("failed to serialize Kiro MCP compatibility config")?;
        encoded.push(b'\n');
        let write_path =
            if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
                fs::canonicalize(path)
                    .with_context(|| format!("failed to resolve {}", path.display()))?
            } else {
                path.to_path_buf()
            };
        crate::runtime_store::write_private_file_atomic(&write_path, &encoded)
            .with_context(|| format!("failed to update {}", path.display()))?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_env_supports_current_and_legacy_clients() {
        let env = kiro_cli_data_dir_env(Path::new("/tmp/kiro-data"));
        assert_eq!(env[0].0, "KIRO_DATA_DIR");
        assert_eq!(env[1].0, "Q_CLI_DATA_DIR");
        assert_eq!(env[2].0, "KIRO_TEST_DB_PATH");
        assert_eq!(env[0].1, env[1].1);
        assert_eq!(
            env[2].1,
            Path::new("/tmp/kiro-data")
                .join("data.sqlite3")
                .into_os_string()
        );
    }

    #[test]
    fn kiro_codebase_memory_config_disables_only_bedrock_incompatible_schema() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-mcp-compat-{}-{stamp}",
            std::process::id()
        ));
        let path = root.join("settings").join("mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            br#"{
  "mcpServers": {
    "memory": {
      "command": "/opt/tools/codebase-memory-mcp",
      "disabledTools": ["delete_project"],
      "unknown": {"keep": true}
    },
    "other": {"command": "/opt/tools/other-mcp", "unknown": [1, 2, 3]}
  }
}"#,
        )
        .unwrap();

        assert!(normalize_kiro_codebase_memory_config(&path).unwrap());
        assert!(!normalize_kiro_codebase_memory_config(&path).unwrap());
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            config["mcpServers"]["memory"]["disabledTools"],
            serde_json::json!(["delete_project", "check_index_coverage"])
        );
        assert_eq!(
            config["mcpServers"]["memory"]["unknown"],
            serde_json::json!({"keep": true})
        );
        assert!(config["mcpServers"]["other"].get("disabledTools").is_none());
        assert_eq!(
            config["mcpServers"]["other"]["unknown"],
            serde_json::json!([1, 2, 3])
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn kiro_codebase_memory_config_initializes_null_and_missing_disabled_tools() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-mcp-shapes-{}-{stamp}",
            std::process::id()
        ));
        let path = root.join("settings").join("mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            br#"{
  "mcpServers": {
    "null-entry": {"command": "/opt/tools/codebase-memory-mcp", "disabledTools": null},
    "missing-entry": {"command": "/opt/tools/codebase-memory-mcp.exe"},
    "unrelated": {"command": "/opt/tools/other-mcp", "disabledTools": "preserve"}
  }
}"#,
        )
        .unwrap();

        assert!(normalize_kiro_codebase_memory_config(&path).unwrap());
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            config["mcpServers"]["null-entry"]["disabledTools"],
            serde_json::json!(["check_index_coverage"])
        );
        assert_eq!(
            config["mcpServers"]["missing-entry"]["disabledTools"],
            serde_json::json!(["check_index_coverage"])
        );
        assert_eq!(
            config["mcpServers"]["unrelated"]["disabledTools"],
            serde_json::json!("preserve")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn kiro_codebase_memory_config_rejects_invalid_target_shape() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-mcp-invalid-{}-{stamp}",
            std::process::id()
        ));
        let path = root.join("settings").join("mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            br#"{"mcpServers":{"memory":{"command":"/opt/tools/codebase-memory-mcp","disabledTools":false}}}"#,
        )
        .unwrap();

        let error = normalize_kiro_codebase_memory_config(&path).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mcp.json.mcpServers.memory.disabledTools must be an array")
        );
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["memory"]["disabledTools"], false);

        fs::remove_dir_all(root).unwrap();
    }
}
