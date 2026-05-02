use super::*;

pub(crate) fn record_run_selection(state: &mut AppState, profile_name: &str) {
    prodex_runtime_launch::record_run_selection_at(state, profile_name, Local::now().timestamp());
}

pub(crate) fn resolve_profile_name(state: &AppState, requested: Option<&str>) -> Result<String> {
    prodex_runtime_launch::resolve_profile_name(state, requested)
}

pub(crate) fn ensure_path_is_unique(state: &AppState, candidate: &Path) -> Result<()> {
    prodex_runtime_launch::ensure_profile_path_is_unique(state, candidate)
}

pub(crate) fn should_enable_runtime_rotation_proxy(
    state: &AppState,
    selected_profile_name: &str,
    allow_auto_rotate: bool,
) -> bool {
    if !allow_auto_rotate || state.profiles.len() <= 1 {
        return false;
    }

    let Some(selected_profile) = state.profiles.get(selected_profile_name) else {
        return false;
    };
    if !selected_profile.codex_home.exists() {
        return false;
    }

    state.profiles.values().any(|profile| {
        profile
            .provider
            .auth_summary(&profile.codex_home)
            .quota_compatible
    })
}
