use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use super::{
    AppPaths, ProfileLifecycleHomeAction, ProfileLifecyclePromoteRollback, remove_home,
    validate_managed_path, validate_temporary_home_path,
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

pub(super) fn finish_home_actions(
    actions: &[ProfileLifecycleHomeAction],
    committed: bool,
) -> Result<()> {
    for action in actions {
        let (kind, source, destination, rollback_remove) = match action {
            ProfileLifecycleHomeAction::Promote {
                source,
                destination,
                rollback,
            } => (
                prodex_mojo_core::profile_export::ProfileImportHomeActionKind::Promote,
                source.as_str(),
                destination.as_str(),
                matches!(rollback, ProfileLifecyclePromoteRollback::Remove),
            ),
            ProfileLifecycleHomeAction::Create { path } => (
                prodex_mojo_core::profile_export::ProfileImportHomeActionKind::Create,
                path.as_str(),
                path.as_str(),
                false,
            ),
            ProfileLifecycleHomeAction::Cleanup { path } => (
                prodex_mojo_core::profile_export::ProfileImportHomeActionKind::Cleanup,
                path.as_str(),
                path.as_str(),
                false,
            ),
            ProfileLifecycleHomeAction::Quarantine { source, quarantine } => (
                prodex_mojo_core::profile_export::ProfileImportHomeActionKind::Quarantine,
                source.as_str(),
                quarantine.as_str(),
                false,
            ),
        };
        let source = Path::new(source);
        let destination = Path::new(destination);
        let decision = prodex_mojo_core::profile_export::profile_import_home_action(
            kind,
            committed,
            lifecycle_path_exists(source)?,
            lifecycle_path_exists(destination)?,
            rollback_remove,
        )
        .map_err(|error| anyhow::anyhow!("Mojo profile-import home decision failed: {error:?}"))?;
        match decision {
            prodex_mojo_core::profile_export::ProfileImportHomeAction::Noop => {}
            prodex_mojo_core::profile_export::ProfileImportHomeAction::Promote => {
                promote_home(source, destination)?;
            }
            prodex_mojo_core::profile_export::ProfileImportHomeAction::RestoreSource => {
                fs::rename(destination, source).with_context(|| {
                    format!(
                        "failed to restore temporary profile home {}",
                        source.display()
                    )
                })?;
            }
            prodex_mojo_core::profile_export::ProfileImportHomeAction::CleanupSource => {
                remove_home(source)?;
            }
            prodex_mojo_core::profile_export::ProfileImportHomeAction::CleanupDestination => {
                remove_home(destination)?;
            }
            prodex_mojo_core::profile_export::ProfileImportHomeAction::CleanupBoth => {
                remove_home(source)?;
                remove_home(destination)?;
            }
        }
    }
    Ok(())
}

fn promote_home(source: &Path, destination: &Path) -> Result<()> {
    if !destination.exists() && source.exists() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        match fs::rename(source, destination) {
            Ok(()) => {}
            Err(_) => {
                crate::copy_codex_home(source, destination)?;
                remove_home(source)?;
            }
        }
    }
    if source.exists() && destination.exists() {
        remove_home(source)?;
    }
    Ok(())
}
