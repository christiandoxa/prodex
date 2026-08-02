use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

use super::super::import_export::{
    ProfileLifecycleHomeAction, ProfileLifecyclePlan, lifecycle_profile_state,
    write_profile_lifecycle_plan,
};
use crate::{
    AddProfileArgs, AppPaths, AppState, ProfileEntry, ProfileProvider, absolutize,
    activate_profile as mark_profile_active, copy_codex_home, create_codex_home_if_missing,
    ensure_path_is_unique, managed_profile_home_path, prepare_managed_codex_home,
};

pub(super) fn add_new_profile_to_state(
    paths: &AppPaths,
    state: &mut AppState,
    args: &AddProfileArgs,
    source_home: Option<&Path>,
    source_email: Option<String>,
    managed: bool,
    activate_profile: bool,
) -> Result<(PathBuf, PathBuf)> {
    let codex_home = match args.codex_home.as_ref() {
        Some(path) => absolutize(path.clone())?,
        None => managed_profile_home_path(paths, &args.name)?,
    };
    ensure_path_is_unique(state, &codex_home)?;
    if managed && codex_home.exists() {
        bail!(
            "managed profile home {} already exists",
            codex_home.display()
        );
    }
    let desired_profile = ProfileEntry {
        codex_home: codex_home.clone(),
        managed,
        email: source_email,
        provider: ProfileProvider::Openai,
    };
    let lifecycle_path = write_profile_lifecycle_plan(
        paths,
        "manage",
        &ProfileLifecyclePlan {
            profile_states: vec![lifecycle_profile_state(
                &args.name,
                None,
                Some(&desired_profile),
            )?],
            previous_active_profile: state.active_profile.clone(),
            next_active_profile: if activate_profile {
                Some(args.name.clone())
            } else {
                state.active_profile.clone()
            },
            home_actions: managed
                .then(|| ProfileLifecycleHomeAction::Create {
                    path: codex_home.display().to_string(),
                })
                .into_iter()
                .collect(),
            auth_journal_paths: Vec::new(),
        },
    )?;
    if let Some(source) = source_home {
        if managed {
            copy_codex_home(source, &codex_home)?;
        }
    } else {
        create_codex_home_if_missing(&codex_home)?;
    }
    if managed {
        prepare_managed_codex_home(paths, &codex_home)?;
    }
    state.profiles.insert(args.name.clone(), desired_profile);
    if activate_profile {
        mark_profile_active(state, &args.name);
    }
    Ok((codex_home, lifecycle_path))
}
