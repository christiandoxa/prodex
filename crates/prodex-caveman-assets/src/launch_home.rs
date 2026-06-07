use anyhow::{Context, Result, bail};
use chrono::Local;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hook_trust::{configure_caveman_hook_trust_state, configure_caveman_session_start_hook};
use crate::localization::localize_text_file;
use crate::marketplace::{install_caveman_marketplace, install_caveman_plugin_cache};
use crate::toml_helpers::ensure_child_table;
use crate::{
    PRODEX_CAVEMAN_HOOK_MARKER, PRODEX_CAVEMAN_HOOK_SCRIPT, PRODEX_CAVEMAN_MARKETPLACE_NAME,
    PRODEX_CAVEMAN_PLUGIN_ID, PRODEX_CAVEMAN_SOURCE_REPO,
};

pub fn prepare_caveman_launch_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    let caveman_home = create_temporary_caveman_home(managed_profiles_root)?;
    if let Err(err) = prodex_shared_codex_fs::copy_codex_home(base_codex_home, &caveman_home)
        .and_then(|_| share_caveman_chat_history(base_codex_home, &caveman_home))
        .and_then(|_| localize_caveman_rollout_state(&caveman_home))
        .and_then(|_| configure_caveman_launch_home(&caveman_home))
    {
        let _ = fs::remove_dir_all(&caveman_home);
        return Err(err);
    }
    Ok(caveman_home)
}

pub fn configure_caveman_launch_home(codex_home: &Path) -> Result<()> {
    localize_text_file(&codex_home.join("config.toml"))?;
    configure_caveman_session_start_script(codex_home)?;
    configure_caveman_config(codex_home)?;
    install_caveman_marketplace(codex_home)?;
    install_caveman_plugin_cache(codex_home)?;
    Ok(())
}

fn configure_caveman_session_start_script(codex_home: &Path) -> Result<()> {
    let script_path = codex_home.join("bin").join(PRODEX_CAVEMAN_HOOK_SCRIPT);
    let script = format!(
        r#"#!/usr/bin/env sh
codex_home="${{CODEX_HOME:-${{HOME:-}}/.codex}}"
marker="$codex_home/{PRODEX_CAVEMAN_HOOK_MARKER}"
marker_dir=$(dirname "$marker")
mkdir -p "$marker_dir" 2>/dev/null || true
if [ -e "$marker" ]; then
  exit 0
fi
: > "$marker" 2>/dev/null || exit 0
printf '%s\n' 'CAVEMAN MODE ACTIVE. $caveman full: terse, no filler, exact tech. Code/commits/security normal. Stop: stop caveman/normal mode.' 'RTK ACTIVE WHEN CONFIGURED. In prodex rtk/s/super, noisy shell commands must visibly start with rtk <cmd>; do not wait for the user to remind you.'
"#
    );
    write_executable_script(&script_path, &script)
}

fn create_temporary_caveman_home(managed_profiles_root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(managed_profiles_root).with_context(|| {
        format!(
            "failed to create managed profile root {}",
            managed_profiles_root.display()
        )
    })?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = managed_profiles_root
            .join(format!(".caveman-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        prodex_shared_codex_fs::create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for Caveman")
}

fn share_caveman_chat_history(base_codex_home: &Path, caveman_home: &Path) -> Result<()> {
    link_caveman_shared_chat_file(
        &base_codex_home.join("history.jsonl"),
        &caveman_home.join("history.jsonl"),
    )?;
    link_caveman_shared_chat_dir(
        &base_codex_home.join("sessions"),
        &caveman_home.join("sessions"),
    )?;
    link_caveman_shared_chat_dir(
        &base_codex_home.join("archived_sessions"),
        &caveman_home.join("archived_sessions"),
    )
}

fn link_caveman_shared_chat_file(source: &Path, link: &Path) -> Result<()> {
    if let Some(parent) = source.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if fs::symlink_metadata(source).is_err() {
        fs::write(source, "").with_context(|| format!("failed to write {}", source.display()))?;
    }
    replace_caveman_path_with_symlink(source, link, false)
}

fn link_caveman_shared_chat_dir(source: &Path, link: &Path) -> Result<()> {
    fs::create_dir_all(source).with_context(|| format!("failed to create {}", source.display()))?;
    replace_caveman_path_with_symlink(source, link, true)
}

fn localize_caveman_rollout_state(codex_home: &Path) -> Result<()> {
    if !codex_home.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(codex_home)
        .with_context(|| format!("failed to read {}", codex_home.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", codex_home.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_caveman_rollout_state_file_name(&file_name) {
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
            let path = entry.path();
            if file_type.is_symlink() {
                localize_caveman_rollout_state_symlink(&path)?;
                continue;
            }
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn localize_caveman_rollout_state_symlink(path: &Path) -> Result<()> {
    let target =
        fs::read_link(path).with_context(|| format!("failed to read {}", path.display()))?;
    let source = if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(target)
    };
    match fs::read(&source) {
        Ok(bytes) => {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            fs::write(path, bytes)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", source.display()));
        }
    }
    Ok(())
}

fn is_caveman_rollout_state_file_name(file_name: &str) -> bool {
    file_name.starts_with("state_")
        && [".sqlite", ".sqlite-shm", ".sqlite-wal"]
            .iter()
            .any(|suffix| file_name.ends_with(suffix))
}

fn replace_caveman_path_with_symlink(target: &Path, link: &Path, is_dir: bool) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if fs::symlink_metadata(link).is_ok() {
        remove_caveman_path(link)?;
    }
    create_caveman_symlink(target, link, is_dir)
}

fn remove_caveman_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
    }
}

fn create_caveman_symlink(target: &Path, link: &Path, is_dir: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = is_dir;
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link Caveman chat history {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        if is_dir {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
        .with_context(|| {
            format!(
                "failed to link Caveman chat history {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link, is_dir);
        bail!("Caveman chat history links are not supported on this platform");
    }

    Ok(())
}

fn configure_caveman_config(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let contents = fs::read_to_string(&config_path).unwrap_or_default();
    let mut table = if contents.trim().is_empty() {
        toml::Table::new()
    } else {
        match toml::from_str::<toml::Value>(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
        {
            toml::Value::Table(table) => table,
            _ => bail!("{} did not parse as a TOML table", config_path.display()),
        }
    };

    let features = ensure_child_table(&mut table, "features");
    features.insert("plugins".to_string(), toml::Value::Boolean(true));

    let caveman_hook_group_index = configure_caveman_session_start_hook(&mut table);
    configure_caveman_hook_trust_state(&mut table, &config_path, caveman_hook_group_index)?;

    let marketplaces = ensure_child_table(&mut table, "marketplaces");
    let caveman_marketplace = ensure_child_table(marketplaces, PRODEX_CAVEMAN_MARKETPLACE_NAME);
    caveman_marketplace.insert(
        "last_updated".to_string(),
        toml::Value::String(Local::now().to_rfc3339()),
    );
    caveman_marketplace.insert(
        "source_type".to_string(),
        toml::Value::String("git".to_string()),
    );
    caveman_marketplace.insert(
        "source".to_string(),
        toml::Value::String(PRODEX_CAVEMAN_SOURCE_REPO.to_string()),
    );
    caveman_marketplace.insert("ref".to_string(), toml::Value::String("main".to_string()));

    let plugins = ensure_child_table(&mut table, "plugins");
    let caveman_plugin = ensure_child_table(plugins, PRODEX_CAVEMAN_PLUGIN_ID);
    caveman_plugin.insert("enabled".to_string(), toml::Value::Boolean(true));

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Caveman config overlay")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

fn write_executable_script(path: &Path, script: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, script).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}
