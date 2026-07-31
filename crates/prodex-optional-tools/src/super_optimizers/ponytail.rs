use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::Path;

use crate::fs_ops::{read_text_file_limited, remove_existing_dir_path, write_text_file};
use crate::toml_helpers::ensure_child_table;
use crate::{OptionalToolId, ResolvedTool, ToolHealthStatus, optional_tool_status};

const MARKETPLACE_NAME: &str = "ponytail";
const PLUGIN_NAME: &str = "ponytail";
const PLUGIN_ID: &str = "ponytail@ponytail";

pub(super) fn install_ponytail_plugin(codex_home: &Path, tool: &ResolvedTool) -> Result<()> {
    ensure!(
        tool.descriptor.id == OptionalToolId::Ponytail,
        "refusing to activate a non-Ponytail tool as Ponytail"
    );
    let health = optional_tool_status(OptionalToolId::Ponytail);
    ensure!(
        health.status == ToolHealthStatus::Installed
            && health.path == tool.path
            && health.version == tool.version
            && health.digest == tool.digest,
        "Ponytail installation changed after resolution"
    );
    let checkout = tool
        .path
        .as_deref()
        .context("validated Ponytail installation has no path")?;
    let plugin_json = checkout.join(".codex-plugin").join("plugin.json");
    let plugin_version = ponytail_plugin_version(&plugin_json)?;
    let marketplace_root = codex_home.join(".tmp/marketplaces").join(MARKETPLACE_NAME);
    copy_ponytail_checkout(checkout, &marketplace_root)?;

    let plugin_cache_base = codex_home
        .join("plugins/cache")
        .join(MARKETPLACE_NAME)
        .join(PLUGIN_NAME);
    remove_existing_dir_path(&plugin_cache_base)
        .with_context(|| format!("failed to clear {}", plugin_cache_base.display()))?;
    copy_ponytail_checkout(checkout, &plugin_cache_base.join(&plugin_version))?;
    configure_ponytail_plugin_config(codex_home, checkout, &plugin_version)?;
    Ok(())
}

fn configure_ponytail_plugin_config(
    codex_home: &Path,
    checkout: &Path,
    plugin_version: &str,
) -> Result<()> {
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

    let features = ensure_child_table(&mut table, "features")?;
    features.insert("plugins".to_string(), toml::Value::Boolean(true));
    features.insert("remote_plugin".to_string(), toml::Value::Boolean(false));

    let marketplaces = ensure_child_table(&mut table, "marketplaces")?;
    let ponytail_marketplace = ensure_child_table(marketplaces, MARKETPLACE_NAME)?;
    ponytail_marketplace.insert(
        "source_type".to_string(),
        toml::Value::String("local".to_string()),
    );
    ponytail_marketplace.insert(
        "source".to_string(),
        toml::Value::String(checkout.display().to_string()),
    );
    ponytail_marketplace.insert(
        "version".to_string(),
        toml::Value::String(plugin_version.to_string()),
    );

    let plugins = ensure_child_table(&mut table, "plugins")?;
    let ponytail_plugin = ensure_child_table(plugins, PLUGIN_ID)?;
    ponytail_plugin.insert("enabled".to_string(), toml::Value::Boolean(true));

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Ponytail plugin config overlay")?;
    write_text_file(&config_path, &rendered)?;
    Ok(())
}

fn ponytail_plugin_version(plugin_json: &Path) -> Result<String> {
    let contents = fs::read_to_string(plugin_json)
        .with_context(|| format!("failed to read {}", plugin_json.display()))?;
    let value: serde_json::Value = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", plugin_json.display()))?;
    Ok(value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.trim().is_empty())
        .unwrap_or("local")
        .to_string())
}

fn copy_ponytail_checkout(source: &Path, destination: &Path) -> Result<()> {
    remove_existing_dir_path(destination)
        .with_context(|| format!("failed to clear {}", destination.display()))?;
    copy_ponytail_dir(source, destination)
}

fn copy_ponytail_dir(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let name = entry.file_name();
        if name.to_string_lossy() == ".git" {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(&name);
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", source_path.display()))?;
        if file_type.is_dir() {
            copy_ponytail_dir(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        } else {
            anyhow::bail!("unsupported file type in {}", source_path.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ponytail_marketplace_migrates_legacy_directory_source_type() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let codex_home = std::env::temp_dir()
            .canonicalize()
            .expect("temp dir should resolve")
            .join(format!(
                "prodex-ponytail-config-{}-{stamp}",
                std::process::id()
            ));
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            "[marketplaces.ponytail]\nsource_type = \"directory\"\n",
        )
        .unwrap();
        let checkout = codex_home.join("ponytail");

        configure_ponytail_plugin_config(&codex_home, &checkout, "1.2.3").unwrap();

        let config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        let config: toml::Value = toml::from_str(&config).unwrap();
        assert_eq!(
            config["marketplaces"][MARKETPLACE_NAME]["source_type"].as_str(),
            Some("local")
        );
        let _ = fs::remove_dir_all(codex_home);
    }
}
