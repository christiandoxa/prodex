use super::config::{
    legacy_default_claude_config_dir, legacy_default_claude_config_path,
    runtime_proxy_claude_config_dir, runtime_proxy_claude_config_path,
    runtime_proxy_claude_legacy_import_marker_path, runtime_proxy_shared_claude_config_dir,
};
use super::*;

pub(crate) fn prepare_runtime_proxy_claude_config_dir(
    paths: &AppPaths,
    codex_home: &Path,
    managed: bool,
) -> Result<PathBuf> {
    let profile_dir = runtime_proxy_claude_config_dir(codex_home);
    if !managed {
        prepare_runtime_proxy_claude_import_target(&profile_dir)?;
        return Ok(profile_dir);
    }

    let shared_dir = runtime_proxy_shared_claude_config_dir(paths);
    prepare_runtime_proxy_claude_import_target(&shared_dir)?;
    migrate_runtime_proxy_claude_profile_dir_to_target(&profile_dir, &shared_dir)?;
    ensure_runtime_proxy_claude_profile_link(&profile_dir, &shared_dir)?;
    Ok(profile_dir)
}

pub(crate) fn prepare_runtime_proxy_claude_import_target(target_dir: &Path) -> Result<()> {
    create_codex_home_if_missing(target_dir)?;
    maybe_import_runtime_proxy_claude_legacy_home(target_dir)
}

pub(crate) fn maybe_import_runtime_proxy_claude_legacy_home(target_dir: &Path) -> Result<()> {
    let marker_path = runtime_proxy_claude_legacy_import_marker_path(target_dir);
    if marker_path.exists() {
        return Ok(());
    }

    let mut imported = false;
    if let Ok(legacy_dir) = legacy_default_claude_config_dir()
        && legacy_dir.is_dir()
    {
        merge_runtime_proxy_claude_directory_contents(&legacy_dir, target_dir)?;
        imported = true;
    }
    if let Ok(legacy_config_path) = legacy_default_claude_config_path()
        && legacy_config_path.is_file()
    {
        merge_runtime_proxy_claude_file(
            &legacy_config_path,
            &runtime_proxy_claude_config_path(target_dir),
        )?;
        imported = true;
    }

    if imported {
        fs::write(&marker_path, "imported\n").with_context(|| {
            format!(
                "failed to write Claude legacy import marker at {}",
                marker_path.display()
            )
        })?;
    }

    Ok(())
}

pub(crate) fn migrate_runtime_proxy_claude_profile_dir_to_target(
    profile_dir: &Path,
    target_dir: &Path,
) -> Result<()> {
    if same_path(profile_dir, target_dir) {
        return Ok(());
    }

    let metadata = match fs::symlink_metadata(profile_dir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", profile_dir.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        let source_dir = runtime_proxy_resolve_symlink_target(profile_dir)?;
        if !source_dir.exists() || same_path(&source_dir, target_dir) {
            return Ok(());
        }
        if !source_dir.is_dir() {
            bail!(
                "expected {} to point to a Claude config directory",
                profile_dir.display()
            );
        }
        merge_runtime_proxy_claude_directory_contents(&source_dir, target_dir)?;
        runtime_proxy_remove_path(profile_dir)?;
        return Ok(());
    }

    if !metadata.is_dir() {
        bail!(
            "expected {} to be a Claude config directory",
            profile_dir.display()
        );
    }

    merge_runtime_proxy_claude_directory_contents(profile_dir, target_dir)?;
    fs::remove_dir_all(profile_dir)
        .with_context(|| format!("failed to remove {}", profile_dir.display()))?;
    Ok(())
}

pub(crate) fn ensure_runtime_proxy_claude_profile_link(
    link_path: &Path,
    target_dir: &Path,
) -> Result<()> {
    if same_path(link_path, target_dir) {
        return Ok(());
    }

    match fs::symlink_metadata(link_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let existing_target = runtime_proxy_resolve_symlink_target(link_path)?;
                if same_path(&existing_target, target_dir) {
                    return Ok(());
                }
            }
            runtime_proxy_remove_path(link_path)?;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", link_path.display()));
        }
    }

    runtime_proxy_create_directory_symlink(target_dir, link_path)
}

pub(crate) fn merge_runtime_proxy_claude_directory_contents(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    if same_path(source, destination) {
        return Ok(());
    }
    create_codex_home_if_missing(destination)?;

    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", source_path.display()))?;

        if file_type.is_dir() {
            merge_runtime_proxy_claude_directory_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            merge_runtime_proxy_claude_file(&source_path, &destination_path)?;
        } else if file_type.is_symlink() {
            merge_runtime_proxy_claude_symlink(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

pub(crate) fn merge_runtime_proxy_claude_file(source: &Path, destination: &Path) -> Result<()> {
    if same_path(source, destination) {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if !destination.exists() {
        fs::copy(source, destination).with_context(|| {
            format!(
                "failed to copy Claude state file {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        return Ok(());
    }

    if destination.is_dir() {
        bail!(
            "expected {} to be a file for Claude state",
            destination.display()
        );
    }

    let file_name = source.file_name().and_then(|name| name.to_str());
    if file_name == Some(DEFAULT_CLAUDE_CONFIG_FILE_NAME) {
        return merge_runtime_proxy_claude_json_file(source, destination);
    }
    if file_name == Some(DEFAULT_CLAUDE_SETTINGS_FILE_NAME) {
        return merge_runtime_proxy_claude_json_file(source, destination);
    }
    if source
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
    {
        return merge_runtime_proxy_claude_jsonl_file(source, destination);
    }

    Ok(())
}

pub(crate) fn merge_runtime_proxy_claude_json_file(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    let source_raw = fs::read_to_string(source)
        .with_context(|| format!("failed to read {}", source.display()))?;
    let destination_raw = fs::read_to_string(destination)
        .with_context(|| format!("failed to read {}", destination.display()))?;
    let source_value = match serde_json::from_str::<serde_json::Value>(&source_raw) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    let mut destination_value = match serde_json::from_str::<serde_json::Value>(&destination_raw) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    runtime_proxy_merge_json_defaults(&mut destination_value, &source_value);
    let rendered = serde_json::to_string_pretty(&destination_value)
        .context("failed to render merged Claude config")?;
    fs::write(destination, rendered)
        .with_context(|| format!("failed to write {}", destination.display()))
}

pub(crate) fn runtime_proxy_merge_json_defaults(
    destination: &mut serde_json::Value,
    source_defaults: &serde_json::Value,
) {
    if destination.is_null() {
        *destination = source_defaults.clone();
        return;
    }

    if let (Some(destination), Some(source_defaults)) =
        (destination.as_object_mut(), source_defaults.as_object())
    {
        for (key, source_value) in source_defaults {
            if let Some(destination_value) = destination.get_mut(key) {
                runtime_proxy_merge_json_defaults(destination_value, source_value);
            } else {
                destination.insert(key.clone(), source_value.clone());
            }
        }
    }
}

pub(crate) fn merge_runtime_proxy_claude_jsonl_file(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    fn load_jsonl_lines(
        path: &Path,
        merged: &mut Vec<String>,
        seen: &mut BTreeSet<String>,
    ) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for raw_line in content.lines() {
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() || !seen.insert(line.to_string()) {
                continue;
            }
            merged.push(line.to_string());
        }
        Ok(())
    }

    let mut merged = Vec::new();
    let mut seen = BTreeSet::new();
    load_jsonl_lines(destination, &mut merged, &mut seen)?;
    load_jsonl_lines(source, &mut merged, &mut seen)?;
    fs::write(destination, merged.join("\n"))
        .with_context(|| format!("failed to write {}", destination.display()))
}

pub(crate) fn merge_runtime_proxy_claude_symlink(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() || fs::symlink_metadata(destination).is_ok() {
        return Ok(());
    }

    let target = fs::read_link(source)
        .with_context(|| format!("failed to read symlink {}", source.display()))?;
    runtime_proxy_create_symlink(&target, destination, true)
}

pub(crate) fn runtime_proxy_resolve_symlink_target(path: &Path) -> Result<PathBuf> {
    let target = fs::read_link(path)
        .with_context(|| format!("failed to read symlink {}", path.display()))?;
    Ok(if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(target)
    })
}

pub(crate) fn runtime_proxy_remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove symbolic link {}", path.display()))?;
        return Ok(());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
    }
}

pub(crate) fn runtime_proxy_create_directory_symlink(target: &Path, link: &Path) -> Result<()> {
    runtime_proxy_create_symlink(target, link, true)
}

pub(crate) fn runtime_proxy_create_symlink(target: &Path, link: &Path, is_dir: bool) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    #[cfg(unix)]
    {
        let _ = is_dir;
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link Claude state {} -> {}",
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
                "failed to link Claude state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = is_dir;
        bail!("Claude state links are not supported on this platform");
    }

    Ok(())
}
