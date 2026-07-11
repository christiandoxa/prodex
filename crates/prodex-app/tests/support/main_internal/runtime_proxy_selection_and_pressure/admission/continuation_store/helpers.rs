use super::*;

pub(super) fn single_profile_test_profiles(temp_dir: &TestDir) -> BTreeMap<String, ProfileEntry> {
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )])
}

pub(super) fn insert_continuation_binding(
    store: &mut RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    bound_at: i64,
) {
    let binding = ResponseProfileBinding {
        profile_name: "main".to_string(),
        bound_at,
    };
    match kind {
        RuntimeContinuationBindingKind::Response => {
            store
                .response_profile_bindings
                .insert(key.to_string(), binding);
        }
        RuntimeContinuationBindingKind::TurnState => {
            store.turn_state_bindings.insert(key.to_string(), binding);
        }
        RuntimeContinuationBindingKind::SessionId => {
            store
                .session_profile_bindings
                .insert(key.to_string(), binding.clone());
            store.session_id_bindings.insert(key.to_string(), binding);
        }
    }
}

pub(super) fn insert_dead_continuation_status(
    store: &mut RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    dead_at: i64,
) {
    let status = dead_continuation_status(dead_at);
    match kind {
        RuntimeContinuationBindingKind::Response => {
            store.statuses.response.insert(key.to_string(), status);
        }
        RuntimeContinuationBindingKind::TurnState => {
            store.statuses.turn_state.insert(key.to_string(), status);
        }
        RuntimeContinuationBindingKind::SessionId => {
            store.statuses.session_id.insert(key.to_string(), status);
        }
    }
}

pub(super) fn continuation_binding_present(
    store: &RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
) -> bool {
    match kind {
        RuntimeContinuationBindingKind::Response => {
            store.response_profile_bindings.contains_key(key)
        }
        RuntimeContinuationBindingKind::TurnState => store.turn_state_bindings.contains_key(key),
        RuntimeContinuationBindingKind::SessionId => {
            store.session_profile_bindings.contains_key(key)
                && store.session_id_bindings.contains_key(key)
        }
    }
}

pub(super) fn continuation_dead_status_present(
    store: &RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
) -> bool {
    let status = match kind {
        RuntimeContinuationBindingKind::Response => store.statuses.response.get(key),
        RuntimeContinuationBindingKind::TurnState => store.statuses.turn_state.get(key),
        RuntimeContinuationBindingKind::SessionId => store.statuses.session_id.get(key),
    };
    status.is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
}
