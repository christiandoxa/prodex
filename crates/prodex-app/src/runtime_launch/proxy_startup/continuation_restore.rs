use super::*;

pub(super) fn restore_session_continuations(
    launch_state: &AppState,
    persisted_state: &AppState,
    persisted: &RuntimeContinuationStore,
    journal: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    // Merge sidecar release proof first, never two legacy owner-only state
    // projections that could poison an otherwise valid rotation with Conflict.
    let authoritative = merge_runtime_continuation_store(persisted, journal, profiles);
    merge_runtime_continuation_store(
        &merge_runtime_continuation_store(
            &authoritative,
            &runtime_continuation_store_from_app_state(launch_state),
            profiles,
        ),
        &runtime_continuation_store_from_app_state(persisted_state),
        profiles,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_start_restores_released_session_before_merging_disagreeing_legacy_state() {
        let now = Local::now().timestamp();
        let binding = |profile: &str, bound_at| ResponseProfileBinding {
            profile_name: profile.to_owned(),
            bound_at,
            binding_identity: None,
        };
        let mut launch = AppState::default();
        launch
            .session_profile_bindings
            .insert("resume-session".into(), binding("alpha", now - 20));
        let mut disk = AppState::default();
        disk.session_profile_bindings
            .insert("resume-session".into(), binding("beta", now));
        let mut sidecar = RuntimeContinuationStore::default();
        sidecar
            .statuses
            .session_release_epochs
            .insert("resume-session".into(), now - 10);
        sidecar.session_id_bindings = disk.session_profile_bindings.clone();
        sidecar.session_profile_bindings = disk.session_profile_bindings.clone();
        let restored = restore_session_continuations(
            &launch,
            &disk,
            &sidecar,
            &RuntimeContinuationStore::default(),
            &BTreeMap::new(),
        );
        assert_eq!(
            restored.session_id_bindings["resume-session"].profile_name,
            "beta"
        );
        assert_eq!(
            restored.session_profile_bindings["resume-session"].profile_name,
            "beta"
        );
        assert_eq!(
            restored.statuses.session_release_epochs["resume-session"],
            now - 10
        );
    }

    #[test]
    fn cold_start_keeps_disagreeing_owner_conflict_without_release_evidence() {
        let now = Local::now().timestamp();
        let binding = |profile: &str| ResponseProfileBinding {
            profile_name: profile.to_owned(),
            bound_at: now,
            binding_identity: None,
        };
        let mut launch = AppState::default();
        let mut disk = AppState::default();
        launch
            .session_profile_bindings
            .insert("shared-session".into(), binding("alpha"));
        disk.session_profile_bindings
            .insert("shared-session".into(), binding("beta"));
        let restored = restore_session_continuations(
            &launch,
            &disk,
            &RuntimeContinuationStore::default(),
            &RuntimeContinuationStore::default(),
            &BTreeMap::new(),
        );
        assert_eq!(
            restored.session_profile_bindings["shared-session"].profile_name,
            prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE
        );
    }
}
