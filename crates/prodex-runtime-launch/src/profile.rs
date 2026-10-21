use anyhow::bail;
use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use prodex_mojo_core::runtime::{
    ProfileNameResolution, profile_name_resolution_plan, profile_next_selection_at,
};
use std::path::{Path, PathBuf};

pub fn allow_profileless_local_home(
    requested_profile: Option<&str>,
    model_provider_override: Option<&str>,
    local_provider_id: &str,
) -> bool {
    requested_profile.is_none()
        && model_provider_override.is_some_and(|provider| {
            ascii_casefold_equal_exact(provider, local_provider_id)
                .expect("Mojo local-provider identity comparison failed")
        })
}

pub fn fixed_runtime_proxy_state(
    state: &prodex_state::AppState,
    profile_name: &str,
) -> anyhow::Result<prodex_state::AppState> {
    let mut fixed = state.clone();
    if !fixed.profiles.contains_key(profile_name) {
        bail!("profile '{}' is missing", profile_name);
    }
    fixed
        .profiles
        .retain(|candidate_name, _| candidate_name == profile_name);
    fixed.active_profile = Some(profile_name.to_string());
    fixed
        .last_run_selected_at
        .retain(|candidate_name, _| candidate_name == profile_name);
    fixed
        .response_profile_bindings
        .retain(|_, binding| binding.profile_name == profile_name);
    fixed
        .session_profile_bindings
        .retain(|_, binding| binding.profile_name == profile_name);
    Ok(fixed)
}

pub fn record_run_selection_at(
    state: &mut prodex_state::AppState,
    profile_name: &str,
    selected_at: i64,
) {
    state
        .last_run_selected_at
        .retain(|name, _| state.profiles.contains_key(name));
    let timestamps = state
        .last_run_selected_at
        .values()
        .copied()
        .collect::<Vec<_>>();
    let selected_at = profile_next_selection_at(&timestamps, selected_at)
        .expect("compiled Mojo run-selection timestamp planner failed");
    state
        .last_run_selected_at
        .insert(profile_name.to_string(), selected_at);
}

pub fn resolve_profile_name(
    state: &prodex_state::AppState,
    requested: Option<&str>,
) -> anyhow::Result<String> {
    let active = state.active_profile.as_deref();
    let action = profile_name_resolution_plan(
        requested.map(|name| state.profiles.contains_key(name)),
        active.map(|name| state.profiles.contains_key(name)),
        state.profiles.len(),
    )
    .expect("compiled Mojo profile-name resolution planner failed");
    match action {
        ProfileNameResolution::Requested => Ok(requested
            .expect("Mojo chose absent requested profile")
            .to_string()),
        ProfileNameResolution::Active => Ok(active
            .expect("Mojo chose absent active profile")
            .to_string()),
        ProfileNameResolution::Single => Ok(state
            .profiles
            .keys()
            .next()
            .expect("Mojo chose single profile from empty state")
            .clone()),
        ProfileNameResolution::MissingRequested => {
            bail!("profile '{}' does not exist", requested.unwrap_or_default())
        }
        ProfileNameResolution::MissingActive => {
            bail!(
                "active profile '{}' no longer exists",
                active.unwrap_or_default()
            )
        }
        ProfileNameResolution::NoSelection => {
            bail!("no active profile selected; use `prodex use --profile <name>` or pass --profile")
        }
    }
}

pub fn ensure_profile_path_is_unique(
    state: &prodex_state::AppState,
    candidate: &Path,
) -> anyhow::Result<()> {
    for (name, profile) in &state.profiles {
        if prodex_core::same_path(&profile.codex_home, candidate) {
            bail!(
                "path {} is already used by profile '{}'",
                candidate.display(),
                name
            );
        }
    }
    Ok(())
}

pub fn dry_run_prodex_overlay_home_placeholder(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> PathBuf {
    managed_profiles_root.join(format!(
        ".prodex-overlay-dry-run-from-{}",
        base_codex_home
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ))
}
