use super::*;

#[test]
fn codex_delete_cleanup_prunes_session_and_compact_bindings() {
    let root = temp_dir("delete-prune-bindings");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let compact_key = prodex_runtime_store::runtime_compact_session_lineage_key(session_id);
    let now = chrono::Local::now().timestamp();
    let binding = ResponseProfileBinding {
        profile_name: "main".to_string(),
        bound_at: now,
    };
    write_state(
        &root,
        AppState {
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: root.join("main-home"),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            session_profile_bindings: BTreeMap::from([
                (session_id.to_string(), binding.clone()),
                (compact_key.clone(), binding.clone()),
            ]),
            ..AppState::default()
        },
    );
    let state = AppState::load(&paths).unwrap();
    let continuations = RuntimeContinuationStore {
        session_profile_bindings: BTreeMap::from([(session_id.to_string(), binding.clone())]),
        session_id_bindings: BTreeMap::from([
            (session_id.to_string(), binding.clone()),
            (compact_key.clone(), binding),
        ]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &continuations, &state.profiles).unwrap();
    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &state.profiles, now)
        .unwrap();

    cleanup_codex_deleted_session_binding(Some(session_id)).unwrap();

    let state = AppState::load(&paths).unwrap();
    assert!(!state.session_profile_bindings.contains_key(session_id));
    assert!(!state.session_profile_bindings.contains_key(&compact_key));
    for persisted in [
        load_runtime_continuations_with_recovery(&paths, &state.profiles)
            .unwrap()
            .value,
        load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
            .unwrap()
            .value
            .continuations,
    ] {
        assert!(!persisted.session_profile_bindings.contains_key(session_id));
        assert!(!persisted.session_id_bindings.contains_key(session_id));
        assert!(!persisted.session_id_bindings.contains_key(&compact_key));
        assert_eq!(
            persisted
                .statuses
                .session_id
                .get(session_id)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
    }
}
