use super::*;

#[test]
fn goal_resume_release_prunes_only_the_target_session_affinity() {
    let temp_dir = TestDir::isolated();
    let shared = RuntimeProxyFixtureBuilder::new().build_shared(&temp_dir);
    let target = "019f6465-a781-7ea0-95b8-83f5ed79d79c";
    let other = "019f6465-aeb1-7da2-897e-a8f6728678ff";
    let compact_target = runtime_compact_session_lineage_key(target);
    let binding = |profile_name: &str| ResponseProfileBinding {
        profile_name: profile_name.to_string(),
        bound_at: 10,
    };
    {
        let mut runtime = shared.runtime.lock().unwrap();
        runtime
            .session_id_bindings
            .insert(target.to_string(), binding("first"));
        runtime
            .session_id_bindings
            .insert(compact_target.clone(), binding("first"));
        runtime
            .session_id_bindings
            .insert(other.to_string(), binding("second"));
        runtime
            .state
            .session_profile_bindings
            .insert(target.to_string(), binding("first"));
        runtime
            .state
            .session_profile_bindings
            .insert(other.to_string(), binding("second"));
    }

    assert!(release_runtime_session_affinity(&shared, target, "goal_resume").unwrap());

    let runtime = shared.runtime.lock().unwrap();
    assert!(!runtime.session_id_bindings.contains_key(target));
    assert!(!runtime.session_id_bindings.contains_key(&compact_target));
    assert!(!runtime.state.session_profile_bindings.contains_key(target));
    assert!(runtime.session_id_bindings.contains_key(other));
    assert!(runtime.state.session_profile_bindings.contains_key(other));
    assert_eq!(
        runtime
            .continuation_statuses
            .session_id
            .get(target)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .session_id
            .get(&compact_target)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}
