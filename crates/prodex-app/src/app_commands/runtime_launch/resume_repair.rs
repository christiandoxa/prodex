use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
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
    if !session_id_looks_full(session_id) {
        repair_resume_session_in_other_profile_homes(codex_home, session_id);
    }
    Ok(())
}

fn session_id_looks_full(session_id: &str) -> bool {
    let bytes = session_id.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
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
    let mut repaired_homes = BTreeSet::new();
    let Ok(state) = AppState::load(&paths) else {
        repair_resume_session_in_profile_root_dirs(
            &paths,
            primary_home,
            session_id,
            &mut repaired_homes,
        );
        return;
    };
    for profile in state.profiles.values() {
        repair_resume_session_in_profile_home(
            primary_home,
            &profile.codex_home,
            session_id,
            &mut repaired_homes,
        );
    }
    repair_resume_session_in_profile_root_dirs(
        &paths,
        primary_home,
        session_id,
        &mut repaired_homes,
    );
}

fn repair_resume_session_in_profile_root_dirs(
    paths: &AppPaths,
    primary_home: &Path,
    session_id: &str,
    repaired_homes: &mut BTreeSet<String>,
) {
    let Ok(entries) = fs::read_dir(&paths.managed_profiles_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        repair_resume_session_in_profile_home(primary_home, &path, session_id, repaired_homes);
    }
}

fn repair_resume_session_in_profile_home(
    primary_home: &Path,
    profile_home: &Path,
    session_id: &str,
    repaired_homes: &mut BTreeSet<String>,
) {
    if prodex_core::same_path(primary_home, profile_home) {
        return;
    }
    let key = profile_home.display().to_string();
    if !repaired_homes.insert(key) {
        return;
    }
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(profile_home, session_id);
}
