use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::embedded_tree::write_caveman_plugin_tree;
use crate::{
    PRODEX_CAVEMAN_MARKETPLACE_NAME, PRODEX_CAVEMAN_PLUGIN_NAME, PRODEX_CAVEMAN_PLUGIN_VERSION,
};

pub fn install_caveman_marketplace(codex_home: &Path) -> Result<()> {
    let marketplace_root = caveman_marketplace_root(codex_home);
    let plugin_root = marketplace_root
        .join("plugins")
        .join(PRODEX_CAVEMAN_PLUGIN_NAME);
    fs::create_dir_all(marketplace_root.join(".agents/plugins")).with_context(|| {
        format!(
            "failed to create Caveman marketplace root {}",
            marketplace_root.display()
        )
    })?;
    write_caveman_plugin_tree(&plugin_root)?;
    let marketplace_manifest = serde_json::to_string_pretty(&serde_json::json!({
        "name": PRODEX_CAVEMAN_MARKETPLACE_NAME,
        "interface": {
            "displayName": "Prodex Caveman",
        },
        "plugins": [
            {
                "name": PRODEX_CAVEMAN_PLUGIN_NAME,
                "source": {
                    "source": "local",
                    "path": format!("./plugins/{PRODEX_CAVEMAN_PLUGIN_NAME}"),
                },
                "policy": {
                    "installation": "AVAILABLE",
                    "authentication": "ON_INSTALL",
                },
                "category": "Productivity",
            }
        ],
    }))
    .context("failed to serialize Caveman marketplace manifest")?;
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        marketplace_manifest,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            marketplace_root
                .join(".agents/plugins/marketplace.json")
                .display()
        )
    })?;
    Ok(())
}

pub fn install_caveman_plugin_cache(codex_home: &Path) -> Result<()> {
    let plugin_cache_base = codex_home
        .join("plugins/cache")
        .join(PRODEX_CAVEMAN_MARKETPLACE_NAME)
        .join(PRODEX_CAVEMAN_PLUGIN_NAME);
    if plugin_cache_base.exists() {
        fs::remove_dir_all(&plugin_cache_base)
            .with_context(|| format!("failed to clear {}", plugin_cache_base.display()))?;
    }
    write_caveman_plugin_tree(&plugin_cache_base.join(PRODEX_CAVEMAN_PLUGIN_VERSION))
}

pub fn caveman_marketplace_root(codex_home: &Path) -> PathBuf {
    codex_home
        .join(".tmp/marketplaces")
        .join(PRODEX_CAVEMAN_MARKETPLACE_NAME)
}
