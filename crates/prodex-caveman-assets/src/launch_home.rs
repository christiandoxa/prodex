use anyhow::{Context, Result, bail};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fs_ops::{
    copy_file_streaming, read_text_file_limited, remove_existing_dir_path, write_text_file,
};
use crate::localization::localize_text_file;
use crate::marketplace::{install_caveman_marketplace, install_caveman_plugin_cache};
use crate::toml_helpers::ensure_child_table;
use crate::{
    PRODEX_CAVEMAN_DEVELOPER_INSTRUCTIONS, PRODEX_CAVEMAN_MARKETPLACE_NAME,
    PRODEX_CAVEMAN_PLUGIN_ID, PRODEX_CAVEMAN_SOURCE_REPO,
};

pub fn prepare_prodex_overlay_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(managed_profiles_root, base_codex_home, true, true, false)
}

pub fn prepare_prodex_overlay_home_from_prepared_base(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(managed_profiles_root, base_codex_home, false, true, false)
}

pub fn prepare_runtime_overlay_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(managed_profiles_root, base_codex_home, true, false, false)
}

pub fn prepare_runtime_overlay_home_from_prepared_base(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(
        managed_profiles_root,
        base_codex_home,
        false,
        false,
        false,
    )
}

pub fn prepare_desktop_overlay_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
    configure_prodex: bool,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(
        managed_profiles_root,
        base_codex_home,
        true,
        configure_prodex,
        true,
    )
}

pub fn prepare_desktop_overlay_home_from_prepared_base(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
    configure_prodex: bool,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(
        managed_profiles_root,
        base_codex_home,
        false,
        configure_prodex,
        true,
    )
}

fn prepare_prodex_overlay_home_internal(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
    maintain_session_attachments: bool,
    configure_prodex: bool,
    share_rollout_state: bool,
) -> Result<PathBuf> {
    let overlay_home = create_temporary_prodex_overlay_home(managed_profiles_root)?;
    if let Err(err) = prodex_shared_codex_fs::copy_codex_home(base_codex_home, &overlay_home)
        .and_then(|_| {
            remove_prodex_overlay_codex_apps_cache(&overlay_home)?;
            share_prodex_overlay_chat_history(
                base_codex_home,
                &overlay_home,
                maintain_session_attachments,
            )
        })
        .and_then(|_| {
            if share_rollout_state {
                share_prodex_overlay_rollout_state(base_codex_home, &overlay_home)
            } else {
                localize_prodex_overlay_rollout_state(&overlay_home)?;
                localize_prodex_overlay_rollout_state_symlinks_from_base(
                    base_codex_home,
                    &overlay_home,
                )
            }
        })
        .and_then(|_| {
            if configure_prodex {
                configure_prodex_overlay_home(&overlay_home)
            } else {
                localize_text_file(&overlay_home.join("config.toml"))
            }
        })
    {
        let _ = fs::remove_dir_all(&overlay_home);
        return Err(err);
    }
    Ok(overlay_home)
}

fn remove_prodex_overlay_codex_apps_cache(codex_home: &Path) -> Result<()> {
    for relative in [
        "cache/codex_apps_server_info",
        "cache/codex_apps_tools",
        "cache/codex_app_directory",
    ] {
        let path = codex_home.join(relative);
        remove_existing_dir_path(&path).with_context(|| {
            format!(
                "failed to remove inherited Codex app connector cache {}",
                path.display()
            )
        })?;
    }
    Ok(())
}

pub fn prepare_caveman_launch_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home(managed_profiles_root, base_codex_home)
}

pub fn configure_prodex_overlay_home(codex_home: &Path) -> Result<()> {
    localize_text_file(&codex_home.join("config.toml"))?;
    configure_caveman_config(codex_home)?;
    install_caveman_marketplace(codex_home)?;
    install_caveman_plugin_cache(codex_home)?;
    Ok(())
}

pub fn configure_caveman_launch_home(codex_home: &Path) -> Result<()> {
    configure_prodex_overlay_home(codex_home)
}

fn create_temporary_prodex_overlay_home(managed_profiles_root: &Path) -> Result<PathBuf> {
    ensure_prodex_overlay_root(managed_profiles_root)?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = managed_profiles_root.join(format!(
            ".prodex-overlay-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        if candidate.exists() {
            continue;
        }
        prodex_shared_codex_fs::create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for Prodex overlay")
}

fn ensure_prodex_overlay_root(managed_profiles_root: &Path) -> Result<()> {
    match fs::symlink_metadata(managed_profiles_root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                bail!(
                    "managed profile root {} must not be a symbolic link",
                    managed_profiles_root.display()
                );
            }
            if !metadata.is_dir() {
                bail!(
                    "managed profile root {} must be a directory",
                    managed_profiles_root.display()
                );
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(managed_profiles_root).with_context(|| {
                format!(
                    "failed to create managed profile root {}",
                    managed_profiles_root.display()
                )
            })?;
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to inspect managed profile root {}",
                    managed_profiles_root.display()
                )
            });
        }
    }
    Ok(())
}

fn share_prodex_overlay_chat_history(
    base_codex_home: &Path,
    overlay_home: &Path,
    maintain_session_attachments: bool,
) -> Result<()> {
    link_prodex_overlay_shared_chat_file(
        &base_codex_home.join("history.jsonl"),
        &overlay_home.join("history.jsonl"),
    )?;
    link_prodex_overlay_shared_chat_dir(
        &base_codex_home.join("sessions"),
        &overlay_home.join("sessions"),
    )?;
    link_prodex_overlay_shared_chat_dir(
        &base_codex_home.join("archived_sessions"),
        &overlay_home.join("archived_sessions"),
    )?;
    link_prodex_overlay_shared_chat_dir(
        &base_codex_home.join("attachments"),
        &overlay_home.join("attachments"),
    )?;
    if maintain_session_attachments {
        prodex_shared_codex_fs::persist_codex_session_image_attachments(base_codex_home)?;
    }
    Ok(())
}

fn link_prodex_overlay_shared_chat_file(source: &Path, link: &Path) -> Result<()> {
    if let Some(parent) = source.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if fs::symlink_metadata(source).is_err() {
        write_text_file(source, "")?;
    }
    replace_prodex_overlay_path_with_symlink(source, link, false)
}

fn link_prodex_overlay_shared_chat_dir(source: &Path, link: &Path) -> Result<()> {
    fs::create_dir_all(source).with_context(|| format!("failed to create {}", source.display()))?;
    replace_prodex_overlay_path_with_symlink(source, link, true)
}

fn share_prodex_overlay_rollout_state(base_codex_home: &Path, overlay_home: &Path) -> Result<()> {
    if !base_codex_home.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(base_codex_home)
        .with_context(|| format!("failed to read {}", base_codex_home.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", base_codex_home.display()))?;
        let file_name = entry.file_name();
        if is_prodex_overlay_rollout_state_file_name(&file_name.to_string_lossy()) {
            replace_prodex_overlay_path_with_symlink(
                &entry.path(),
                &overlay_home.join(file_name),
                false,
            )?;
        }
    }
    Ok(())
}

fn localize_prodex_overlay_rollout_state(codex_home: &Path) -> Result<()> {
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
        if is_prodex_overlay_rollout_state_file_name(&file_name) {
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
            let path = entry.path();
            if file_type.is_symlink() {
                localize_prodex_overlay_rollout_state_symlink(&path)?;
                continue;
            }
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn localize_prodex_overlay_rollout_state_symlinks_from_base(
    base_codex_home: &Path,
    overlay_home: &Path,
) -> Result<()> {
    if !base_codex_home.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(base_codex_home)
        .with_context(|| format!("failed to read {}", base_codex_home.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", base_codex_home.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !is_prodex_overlay_rollout_state_file_name(&file_name) {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if file_type.is_symlink() {
            copy_prodex_overlay_rollout_state_symlink(
                &entry.path(),
                &overlay_home.join(file_name.as_ref()),
            )?;
        }
    }
    Ok(())
}

fn localize_prodex_overlay_rollout_state_symlink(path: &Path) -> Result<()> {
    let target =
        fs::read_link(path).with_context(|| format!("failed to read {}", path.display()))?;
    let source = if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(target)
    };
    if !copy_file_streaming(&source, path)? {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

fn copy_prodex_overlay_rollout_state_symlink(source_link: &Path, destination: &Path) -> Result<()> {
    let target = fs::read_link(source_link)
        .with_context(|| format!("failed to read {}", source_link.display()))?;
    let source = if target.is_absolute() {
        target
    } else {
        source_link
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target)
    };
    let _ = copy_file_streaming(&source, destination)?;
    Ok(())
}

fn is_prodex_overlay_rollout_state_file_name(file_name: &str) -> bool {
    file_name.starts_with("state_")
        && [".sqlite", ".sqlite-shm", ".sqlite-wal"]
            .iter()
            .any(|suffix| file_name.ends_with(suffix))
}

fn replace_prodex_overlay_path_with_symlink(
    target: &Path,
    link: &Path,
    is_dir: bool,
) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if fs::symlink_metadata(link).is_ok() {
        remove_prodex_overlay_path(link)?;
    }
    create_prodex_overlay_symlink(target, link, is_dir)
}

fn remove_prodex_overlay_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
    }
}

fn create_prodex_overlay_symlink(target: &Path, link: &Path, is_dir: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = is_dir;
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link Prodex overlay chat history {} -> {}",
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
                "failed to link Prodex overlay chat history {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link, is_dir);
        bail!("Prodex overlay chat history links are not supported on this platform");
    }

    Ok(())
}

fn configure_caveman_config(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let contents = read_text_file_limited(&config_path)?.unwrap_or_default();
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
    features.insert("plugins".to_string(), toml::Value::Boolean(false));
    features.insert("remote_plugin".to_string(), toml::Value::Boolean(false));

    let developer_instructions = table
        .entry("developer_instructions".to_string())
        .or_insert_with(|| toml::Value::String(String::new()));
    let current_instructions = developer_instructions.as_str().unwrap_or_default();
    if !current_instructions.contains(PRODEX_CAVEMAN_DEVELOPER_INSTRUCTIONS) {
        *developer_instructions = toml::Value::String(if current_instructions.trim().is_empty() {
            PRODEX_CAVEMAN_DEVELOPER_INSTRUCTIONS.to_string()
        } else {
            format!("{current_instructions}\n\n{PRODEX_CAVEMAN_DEVELOPER_INSTRUCTIONS}")
        });
    }

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
        .context("failed to render Prodex overlay config")?;
    write_text_file(&config_path, &rendered)?;
    Ok(())
}
