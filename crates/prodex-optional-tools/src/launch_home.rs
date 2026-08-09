use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fs_ops::{
    copy_file_streaming, read_text_file_limited, remove_existing_dir_path, write_text_file,
};
use crate::localization::localize_text_file;

const LEGACY_CAVEMAN_INSTRUCTIONS_SHA256: &str =
    "a07e8d5167e454f7637eb0e35230a84987c537ae98d6f795d246338b652810f1";
const LEGACY_CAVEMAN_MARKETPLACE: &str = "prodex-caveman";
const LEGACY_CAVEMAN_PLUGIN: &str = "caveman@prodex-caveman";

pub fn prepare_prodex_overlay_home(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(managed_profiles_root, base_codex_home, true, true, true)
}

pub fn prepare_prodex_overlay_home_from_prepared_base(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prepare_prodex_overlay_home_internal(managed_profiles_root, base_codex_home, false, true, true)
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

pub fn configure_prodex_overlay_home(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    localize_text_file(&config_path)?;
    remove_legacy_caveman_config(&config_path)?;
    for relative in [
        ".tmp/marketplaces/prodex-caveman",
        "plugins/cache/prodex-caveman",
    ] {
        remove_existing_dir_path(&codex_home.join(relative))?;
    }
    Ok(())
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
            // The shared helper creates and secures the root below.
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
    prodex_shared_codex_fs::create_codex_home_if_missing(managed_profiles_root)
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

fn remove_legacy_caveman_config(config_path: &Path) -> Result<()> {
    let contents = read_text_file_limited(config_path)?.unwrap_or_default();
    if contents.trim().is_empty() {
        return Ok(());
    }
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

    let mut changed = remove_table_entry(&mut table, "marketplaces", LEGACY_CAVEMAN_MARKETPLACE);
    changed |= remove_table_entry(&mut table, "plugins", LEGACY_CAVEMAN_PLUGIN);
    changed |= remove_legacy_caveman_instructions(&mut table);
    if !changed {
        return Ok(());
    }
    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Prodex overlay config")?;
    write_text_file(config_path, &rendered)
}

fn remove_table_entry(table: &mut toml::Table, parent: &str, key: &str) -> bool {
    let Some(toml::Value::Table(child)) = table.get_mut(parent) else {
        return false;
    };
    let changed = child.remove(key).is_some();
    if child.is_empty() {
        table.remove(parent);
    }
    changed
}

fn remove_legacy_caveman_instructions(table: &mut toml::Table) -> bool {
    let Some(instructions) = table
        .get("developer_instructions")
        .and_then(toml::Value::as_str)
    else {
        return false;
    };
    let retained = instructions
        .split("\n\n")
        .filter(|paragraph| {
            crate::optional_tools::hex_digest(&Sha256::digest(paragraph.as_bytes()))
                != LEGACY_CAVEMAN_INSTRUCTIONS_SHA256
        })
        .collect::<Vec<_>>();
    if retained.len() == instructions.split("\n\n").count() {
        return false;
    }
    if retained.is_empty() {
        table.remove("developer_instructions");
    } else {
        table.insert(
            "developer_instructions".to_string(),
            toml::Value::String(retained.join("\n\n")),
        );
    }
    true
}
