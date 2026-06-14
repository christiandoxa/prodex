use std::ffi::OsString;
use std::path::Path;

use anyhow::{Result, bail};

use crate::{AppPaths, AppState, AppStateIoExt};

pub(super) fn repair_resume_session_metadata_prefix_from_codex_args(
    codex_args: &[OsString],
) -> Result<()> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        return Ok(());
    };
    let paths = AppPaths::discover()?;
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(
        &paths.shared_codex_root,
        session_id,
    )?;
    if let Some(path) = prodex_session_store::find_unrepairable_resume_session(
        &paths.shared_codex_root,
        session_id,
    )? {
        bail!(
            "session '{}' cannot be resumed because {} does not contain session metadata; the file is too incomplete to repair",
            session_id,
            path.display()
        );
    }
    Ok(())
}

pub(super) fn repair_resume_session_in_home(
    codex_home: &Path,
    codex_args: &[OsString],
) -> Result<()> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        return Ok(());
    };
    repair_resume_session_home_strict(codex_home, session_id)?;
    repair_resume_session_in_other_profile_homes(codex_home, session_id);
    Ok(())
}

fn repair_resume_session_home_strict(codex_home: &Path, session_id: &str) -> Result<()> {
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(codex_home, session_id)?;
    if let Some(path) =
        prodex_session_store::find_unrepairable_resume_session(codex_home, session_id)?
    {
        bail!(
            "session '{}' cannot be resumed because {} does not contain session metadata; the file is too incomplete to repair",
            session_id,
            path.display()
        );
    }
    Ok(())
}

fn repair_resume_session_in_other_profile_homes(primary_home: &Path, session_id: &str) {
    let Ok(paths) = AppPaths::discover() else {
        return;
    };
    let Ok(state) = AppState::load(&paths) else {
        return;
    };
    for profile in state.profiles.values() {
        if prodex_core::same_path(primary_home, &profile.codex_home) {
            continue;
        }
        let _ = prodex_session_store::repair_resume_session_metadata_prefix(
            &profile.codex_home,
            session_id,
        );
    }
}
