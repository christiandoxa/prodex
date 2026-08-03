use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use super::{
    AppPaths, ProfileLifecycleHomeAction, validate_managed_path, validate_temporary_home_path,
};

pub(super) fn lifecycle_path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

pub(super) fn validate_home_actions(
    paths: &AppPaths,
    actions: &[ProfileLifecycleHomeAction],
) -> Result<()> {
    for action in actions {
        match action {
            ProfileLifecycleHomeAction::Promote {
                source,
                destination,
                ..
            } => {
                validate_temporary_home_path(paths, Path::new(source), "promote source")?;
                validate_managed_path(paths, Path::new(destination), "promote destination")?;
            }
            ProfileLifecycleHomeAction::Create { path } => {
                validate_managed_path(paths, Path::new(path), "create path")?;
            }
            ProfileLifecycleHomeAction::Cleanup { path } => {
                validate_temporary_home_path(paths, Path::new(path), "cleanup path")?;
            }
            ProfileLifecycleHomeAction::Quarantine { source, quarantine } => {
                validate_managed_path(paths, Path::new(source), "quarantine source")?;
                validate_managed_path(paths, Path::new(quarantine), "quarantine path")?;
                if !Path::new(quarantine)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".remove-"))
                {
                    bail!(
                        "profile lifecycle quarantine path {} is invalid",
                        quarantine
                    );
                }
            }
        }
    }
    Ok(())
}
