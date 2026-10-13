use super::super::super::super::goal_resume;
use super::*;

#[test]
fn legacy_descendant_notify_cannot_redirect_root_capacity_recovery() {
    let root = fixture_root("native-root-vs-descendant-notify");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(&[], false)
        .unwrap()
        .unwrap();
    let write = |payload: serde_json::Value| {
        goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&payload.to_string()),
        )
        .unwrap();
    };
    write(serde_json::json!({
        "hook_event_name":"SessionStart", "session_id":SESSION_ID,
        "transcript_path":session_path, "source":"startup"
    }));
    let original_offset =
        fs::read_to_string(runtime_goal_session_offset_path(&monitor.marker_path)).unwrap();
    // Upstream SessionStart carries the root/session ID. Legacy notify carries
    // a concrete agent thread ID; descendants inherit the same notify command.
    write(serde_json::json!({
        "type":"agent-turn-complete", "thread-id":"01a1333e-0ce6-77b3-9afe-604f1d353ab6",
        "turn-id":"descendant-finished", "last-assistant-message":"done"
    }));
    assert_eq!(
        fs::read_to_string(&monitor.marker_path).unwrap().trim(),
        SESSION_ID
    );
    assert_eq!(
        fs::read_to_string(runtime_goal_session_offset_path(&monitor.marker_path)).unwrap(),
        original_offset
    );
    append_native_failure(&session_path);
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    assert_eq!(monitor.recovery_failure_class(), "overload");
    assert!(
        !monitor.has_session_goal(),
        "ordinary non-goal tasks must recover too"
    );
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn repeated_root_notify_does_not_seek_past_unobserved_terminal_capacity() {
    let root = fixture_root("native-notify-preserves-cursor");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(&[], false)
        .unwrap()
        .unwrap();
    goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&serde_json::json!({"session_id":SESSION_ID}).to_string()),
    )
    .unwrap();
    let offset =
        fs::read_to_string(runtime_goal_session_offset_path(&monitor.marker_path)).unwrap();
    append_native_failure(&session_path);
    for payload in [
        serde_json::json!({"type":"agent-turn-complete","thread-id":SESSION_ID}),
        serde_json::json!({"hook_event_name":"SessionStart","source":"compact","session_id":SESSION_ID}),
    ] {
        goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&payload.to_string()),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(runtime_goal_session_offset_path(&monitor.marker_path)).unwrap(),
            offset
        );
    }
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    monitor.acknowledge_in_place_recovery();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn canonical_session_identity_takes_precedence_over_concrete_agent_thread() {
    let root = fixture_root("native-hook-canonical-identity");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    populate_fixture(&root, &paths, &["a", "b"]);
    let monitor = prepare_runtime_usage_limit_monitor(&[], false)
        .unwrap()
        .unwrap();
    let payload = serde_json::json!({
        "session_id":SESSION_ID,
        "thread-id":"01a1333e-0ce6-77b3-9afe-604f1d353ab6"
    });
    goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&payload.to_string()),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&monitor.marker_path).unwrap().trim(),
        SESSION_ID
    );
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_bootstrap_and_explicit_new_session_start_keep_compatibility() {
    let root = fixture_root("native-marker-startup-switch");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    populate_fixture(&root, &paths, &["a", "b"]);
    let monitor = prepare_runtime_usage_limit_monitor(&[], false)
        .unwrap()
        .unwrap();
    let write = |payload: serde_json::Value| {
        goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&payload.to_string()),
        )
        .unwrap();
    };
    write(serde_json::json!({"type":"agent-turn-complete","thread-id":SESSION_ID}));
    assert_eq!(
        fs::read_to_string(&monitor.marker_path).unwrap().trim(),
        SESSION_ID
    );
    let next_root = "019c9e3d-45a0-7ad0-a6ee-b194ac2d4500";
    write(
        serde_json::json!({"hook_event_name":"SessionStart","session_id":next_root,"source":"clear"}),
    );
    assert_eq!(
        fs::read_to_string(&monitor.marker_path).unwrap().trim(),
        next_root
    );
    write(serde_json::json!({"type":"agent-turn-complete","thread-id":SESSION_ID}));
    assert_eq!(
        fs::read_to_string(&monitor.marker_path).unwrap().trim(),
        next_root
    );
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_canonical_identity_does_not_fall_back_to_descendant_notify() {
    let root = fixture_root("native-marker-invalid-identity");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    populate_fixture(&root, &paths, &["a", "b"]);
    let monitor = prepare_runtime_usage_limit_monitor(&[], false)
        .unwrap()
        .unwrap();
    goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&serde_json::json!({"session_id":SESSION_ID}).to_string()),
    )
    .unwrap();
    for bad in [
        serde_json::Value::Null,
        serde_json::json!("invalid-uuid"),
        serde_json::json!(7),
    ] {
        let payload = serde_json::json!({"session_id":bad,"thread-id":"019c9e3d-45a0-7ad0-a6ee-b194ac2d4500"});
        assert!(
            goal_resume::write_runtime_goal_session_marker(
                &monitor.marker_path,
                std::ffi::OsStr::new(&payload.to_string())
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(&monitor.marker_path).unwrap().trim(),
            SESSION_ID
        );
    }
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_descendant_notifications_do_not_replace_the_root_marker() {
    let root = fixture_root("native-concurrent-notify");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    populate_fixture(&root, &paths, &["a", "b"]);
    let monitor = prepare_runtime_usage_limit_monitor(&[], false)
        .unwrap()
        .unwrap();
    goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&serde_json::json!({"session_id":SESSION_ID}).to_string()),
    )
    .unwrap();
    let before =
        fs::read_to_string(runtime_goal_session_offset_path(&monitor.marker_path)).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let path = &monitor.marker_path;
            scope.spawn(move || {
                goal_resume::write_runtime_goal_session_marker(
                    path,
                    std::ffi::OsStr::new(r#"{"type":"agent-turn-complete","thread-id":"019c9e3d-45a0-7ad0-a6ee-b194ac2d4500"}"#),
                ).unwrap();
            });
        }
    });
    assert_eq!(
        fs::read_to_string(&monitor.marker_path).unwrap().trim(),
        SESSION_ID
    );
    assert_eq!(
        fs::read_to_string(runtime_goal_session_offset_path(&monitor.marker_path)).unwrap(),
        before
    );
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}
