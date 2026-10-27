use super::{
    LoginMethod, acquire_profile_lifecycle_lock, auto_login_transition, find_profile_by_identity,
    finish_auto_login_for_existing_profile, finish_auto_login_for_new_profile,
    load_profile_state_with_profile_recovery_locked,
};
use crate::{AppPaths, AppState, ProfileIdentity, read_profile_identity_from_auth};
use anyhow::{Context, Result, bail};
use std::path::Path;

fn local_identities(state: &AppState) -> Vec<(String, ProfileIdentity)> {
    state
        .profiles
        .iter()
        .filter(|(_, profile)| profile.provider.supports_codex_runtime())
        .map(|(name, profile)| {
            (
                name.clone(),
                read_profile_identity_from_auth(&profile.codex_home).unwrap_or_default(),
            )
        })
        .collect()
}

pub(super) fn finish_identity_login(
    paths: &AppPaths,
    method: LoginMethod,
    login_home: &Path,
    auth_label: &str,
    auth_json: &str,
    identity: &ProfileIdentity,
) -> Result<()> {
    let email = identity
        .email
        .as_deref()
        .context("logged-in account identity did not include an email")?;
    // A changing profile set must never cause an unbounded retry or overwrite
    // an account selected before another lifecycle operation committed.
    for _ in 0..3 {
        let (mut snapshot, original_profiles, original_identities) = {
            let _lock = acquire_profile_lifecycle_lock(paths)?;
            let (state, _) = load_profile_state_with_profile_recovery_locked(paths, true)?;
            let profiles = state.profiles.clone();
            let identities = local_identities(&state);
            (state, profiles, identities)
        };
        // The legacy quota fallback may block. Other commands can still recover
        // and read their profile state while this independent lookup is pending.
        let profile_name = find_profile_by_identity(&mut snapshot, identity)?;
        let _lock = acquire_profile_lifecycle_lock(paths)?;
        let (mut current, _) = load_profile_state_with_profile_recovery_locked(paths, true)?;
        if current.profiles != original_profiles
            || local_identities(&current) != original_identities
        {
            continue;
        }
        // Preserve concurrent runtime/affinity updates from the fresh state;
        // merge only identity metadata obtained from the unchanged profile set.
        for (name, profile) in &mut current.profiles {
            profile.email = snapshot.profiles[name].email.clone();
        }
        match auto_login_transition(method, Some(auth_label), profile_name.is_some()) {
            prodex_mojo_core::profile_login_policy::AutoLoginTransition::IdentityExisting => {
                let name = profile_name
                    .as_deref()
                    .context("Mojo selected an existing login target without a profile")?;
                finish_auto_login_for_existing_profile(
                    paths,
                    &mut current,
                    login_home,
                    name,
                    email,
                    auth_json,
                )?;
            }
            prodex_mojo_core::profile_login_policy::AutoLoginTransition::IdentityNew => {
                if profile_name.is_some() {
                    bail!("Mojo selected a new login target for an existing profile");
                }
                finish_auto_login_for_new_profile(paths, &mut current, login_home, email)?;
            }
            route => unreachable!("Mojo identity transition returned unexpected route {route:?}"),
        }
        return Ok(());
    }
    bail!("profile state changed repeatedly while login was being finalized; retry login")
}
