use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const PRODEX_CAVEMAN_MARKETPLACE_NAME: &str = "prodex-caveman";
pub const PRODEX_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_CAVEMAN_PLUGIN_VERSION: &str = "0.1.0";
pub const PRODEX_CAVEMAN_PLUGIN_ID: &str = "caveman@prodex-caveman";
pub const PRODEX_CAVEMAN_SOURCE_REPO: &str = "https://github.com/JuliusBrussee/caveman.git";
pub const PRODEX_CAVEMAN_FULL_ASSETS_ENV: &str = "PRODEX_CAVEMAN_FULL_ASSETS";

struct EmbeddedCavemanFile {
    relative_path: &'static str,
    contents: &'static str,
}

const CAVEMAN_CORE_PLUGIN_FILES: &[EmbeddedCavemanFile] = &[
    EmbeddedCavemanFile {
        relative_path: ".codex-plugin/plugin.json",
        contents: include_str!("../caveman_assets/.codex-plugin/plugin.json"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman-small.svg",
        contents: include_str!("../caveman_assets/assets/caveman-small.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman.svg",
        contents: include_str!("../caveman_assets/assets/caveman.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/SKILL.md",
        contents: include_str!("../caveman_assets/skills/caveman/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/agents/openai.yaml",
        contents: include_str!("../caveman_assets/skills/caveman/agents/openai.yaml"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/assets/caveman-small.svg",
        contents: include_str!("../caveman_assets/skills/caveman/assets/caveman-small.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/assets/caveman.svg",
        contents: include_str!("../caveman_assets/skills/caveman/assets/caveman.svg"),
    },
];

const CAVEMAN_COMPRESS_PLUGIN_FILES: &[EmbeddedCavemanFile] = &[
    EmbeddedCavemanFile {
        relative_path: "skills/compress/SKILL.md",
        contents: include_str!("../caveman_assets/skills/compress/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/__init__.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/__init__.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/__main__.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/__main__.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/benchmark.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/benchmark.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/cli.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/cli.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/compress.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/compress.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/detect.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/detect.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/validate.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/validate.py"),
    },
];

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

fn write_caveman_plugin_tree(root: &Path) -> Result<()> {
    write_caveman_plugin_files(root, CAVEMAN_CORE_PLUGIN_FILES)?;
    if caveman_full_assets_enabled() {
        write_caveman_plugin_files(root, CAVEMAN_COMPRESS_PLUGIN_FILES)?;
    }
    Ok(())
}

fn write_caveman_plugin_files(root: &Path, files: &[EmbeddedCavemanFile]) -> Result<()> {
    for file in files {
        let path = root.join(file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, file.contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn caveman_full_assets_enabled() -> bool {
    env::var(PRODEX_CAVEMAN_FULL_ASSETS_ENV)
        .ok()
        .is_some_and(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no" | "off"
            )
        })
}
