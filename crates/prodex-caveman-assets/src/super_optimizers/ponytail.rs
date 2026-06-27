use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::toml_helpers::ensure_child_table;

const MARKETPLACE_NAME: &str = "ponytail";
const PLUGIN_NAME: &str = "ponytail";
const PLUGIN_ID: &str = "ponytail@ponytail";
const SOURCE_REPO: &str = "https://github.com/DietrichGebert/ponytail.git";

pub(super) fn install_ponytail_plugin(codex_home: &Path, checkout: &Path) -> Result<()> {
    let plugin_json = checkout.join(".codex-plugin").join("plugin.json");
    let plugin_version = ponytail_plugin_version(&plugin_json)?;
    let marketplace_root = codex_home.join(".tmp/marketplaces").join(MARKETPLACE_NAME);
    copy_ponytail_checkout(checkout, &marketplace_root)?;

    let plugin_cache_base = codex_home
        .join("plugins/cache")
        .join(MARKETPLACE_NAME)
        .join(PLUGIN_NAME);
    if plugin_cache_base.exists() {
        fs::remove_dir_all(&plugin_cache_base)
            .with_context(|| format!("failed to clear {}", plugin_cache_base.display()))?;
    }
    copy_ponytail_checkout(checkout, &plugin_cache_base.join(&plugin_version))?;
    configure_ponytail_plugin_config(codex_home)?;
    Ok(())
}

pub(super) fn find_ponytail_checkout(optimizer_roots: &[PathBuf]) -> Option<PathBuf> {
    optimizer_roots
        .iter()
        .map(|root| root.join(PLUGIN_NAME))
        .find(|checkout| {
            checkout.join(".codex-plugin").join("plugin.json").is_file()
                && checkout
                    .join("hooks")
                    .join("claude-codex-hooks.json")
                    .is_file()
                && checkout.join("skills").is_dir()
        })
}

fn configure_ponytail_plugin_config(codex_home: &Path) -> Result<()> {
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

    let features = ensure_child_table(&mut table, "features");
    features.insert("plugins".to_string(), toml::Value::Boolean(true));

    let marketplaces = ensure_child_table(&mut table, "marketplaces");
    let ponytail_marketplace = ensure_child_table(marketplaces, MARKETPLACE_NAME);
    ponytail_marketplace.insert(
        "last_updated".to_string(),
        toml::Value::String(chrono::Local::now().to_rfc3339()),
    );
    ponytail_marketplace.insert(
        "source_type".to_string(),
        toml::Value::String("git".to_string()),
    );
    ponytail_marketplace.insert(
        "source".to_string(),
        toml::Value::String(SOURCE_REPO.to_string()),
    );

    let plugins = ensure_child_table(&mut table, "plugins");
    let ponytail_plugin = ensure_child_table(plugins, PLUGIN_ID);
    ponytail_plugin.insert("enabled".to_string(), toml::Value::Boolean(true));

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Ponytail plugin config overlay")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
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
    if destination.exists() {
        fs::remove_dir_all(destination)
            .with_context(|| format!("failed to clear {}", destination.display()))?;
    }
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
        }
    }
    Ok(())
}
