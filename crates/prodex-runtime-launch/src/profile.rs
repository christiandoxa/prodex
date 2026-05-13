use anyhow::bail;
use std::path::{Path, PathBuf};

pub fn allow_profileless_local_home(
    requested_profile: Option<&str>,
    model_provider_override: Option<&str>,
    local_provider_id: &str,
) -> bool {
    requested_profile.is_none()
        && model_provider_override
            .is_some_and(|provider| provider.eq_ignore_ascii_case(local_provider_id))
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
    state
        .last_run_selected_at
        .insert(profile_name.to_string(), selected_at);
}

pub fn resolve_profile_name(
    state: &prodex_state::AppState,
    requested: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(name) = requested {
        if state.profiles.contains_key(name) {
            return Ok(name.to_string());
        }
        bail!("profile '{}' does not exist", name);
    }

    if let Some(active) = state.active_profile.as_deref() {
        if state.profiles.contains_key(active) {
            return Ok(active.to_string());
        }
        bail!("active profile '{}' no longer exists", active);
    }

    if state.profiles.len() == 1 {
        let (name, _) = state
            .profiles
            .iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("single profile lookup failed unexpectedly"))?;
        return Ok(name.clone());
    }

    bail!("no active profile selected; use `prodex use --profile <name>` or pass --profile")
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

pub fn dry_run_caveman_home_placeholder(
    managed_profiles_root: &Path,
    base_codex_home: &Path,
) -> PathBuf {
    managed_profiles_root.join(format!(
        ".caveman-dry-run-from-{}",
        base_codex_home
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ))
}
