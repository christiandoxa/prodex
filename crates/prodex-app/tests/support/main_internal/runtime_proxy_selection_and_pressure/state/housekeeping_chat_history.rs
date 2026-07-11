use super::*;

#[test]
fn perform_prodex_cleanup_leaves_codex_managed_chat_history_untouched() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("prodex/.codex"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.shared_codex_root).expect("shared codex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let now = Local
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .expect("fixed timestamp should be valid")
        .timestamp();
    let old_ts = now - (31 * 24 * 60 * 60);
    let recent_ts = now - (2 * 24 * 60 * 60);
    fs::write(
        paths.shared_codex_root.join("history.jsonl"),
        format!(
            "{{\"session_id\":\"old\",\"ts\":{old_ts},\"text\":\"old\"}}\n\
             {{\"session_id\":\"recent\",\"ts\":{recent_ts},\"text\":\"recent\"}}\n\
             {{\"session_id\":\"unstamped\",\"text\":\"keep\"}}\n"
        ),
    )
    .expect("shared codex history should write");

    let old_session_dir = paths.shared_codex_root.join("sessions/2026/04/10");
    let recent_session_dir = paths.shared_codex_root.join("sessions/2026/05/18");
    fs::create_dir_all(&old_session_dir).expect("old codex session dir should exist");
    fs::create_dir_all(&recent_session_dir).expect("recent codex session dir should exist");
    fs::write(
        old_session_dir.join("old.jsonl"),
        format!(
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n",
            Local.timestamp_opt(old_ts, 0).single().unwrap().to_rfc3339()
        ),
    )
    .expect("old codex session should write");
    fs::write(
        recent_session_dir.join("recent.jsonl"),
        format!(
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n",
            Local
                .timestamp_opt(recent_ts, 0)
                .single()
                .unwrap()
                .to_rfc3339()
        ),
    )
    .expect("recent codex session should write");

    let shared_claude_project =
        runtime_proxy_shared_claude_config_dir(&paths).join("projects/workspace");
    fs::create_dir_all(&shared_claude_project).expect("shared claude project should exist");
    fs::write(
        shared_claude_project.join("session.jsonl"),
        format!(
            "{{\"timestamp\":\"{}\",\"message\":\"old\"}}\n\
             {{\"timestamp\":\"{}\",\"message\":\"recent\"}}\n",
            Local
                .timestamp_opt(old_ts, 0)
                .single()
                .unwrap()
                .to_rfc3339(),
            Local
                .timestamp_opt(recent_ts, 0)
                .single()
                .unwrap()
                .to_rfc3339()
        ),
    )
    .expect("shared claude history should write");

    let profile_home = paths.managed_profiles_root.join("main");
    let profile_old_session_dir = profile_home.join("sessions/2026/04/09");
    fs::create_dir_all(&profile_old_session_dir).expect("profile old session dir should exist");
    fs::write(
        profile_old_session_dir.join("profile-old.json"),
        format!(
            "{{\"timestamp\":\"{}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n",
            Local.timestamp_opt(old_ts, 0).single().unwrap().to_rfc3339()
        ),
    )
    .expect("profile old session should write");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("runtime log dir should exist");
    let pointer = runtime_log_dir.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    let simulated_now = UNIX_EPOCH
        .checked_add(Duration::from_secs(now as u64))
        .expect("simulated time should be valid");
    let summary =
        perform_prodex_cleanup_at(&paths, &state, &runtime_log_dir, &pointer, simulated_now)
            .expect("cleanup should succeed");

    assert_eq!(summary.total_removed(), 0);
    let codex_history = fs::read_to_string(paths.shared_codex_root.join("history.jsonl"))
        .expect("shared codex history should remain");
    assert!(codex_history.contains("\"session_id\":\"old\""));
    assert!(codex_history.contains("\"session_id\":\"recent\""));
    assert!(codex_history.contains("\"session_id\":\"unstamped\""));
    assert!(
        old_session_dir.join("old.jsonl").exists(),
        "old dated codex session should remain Codex-managed"
    );
    assert!(
        recent_session_dir.join("recent.jsonl").exists(),
        "recent dated codex session should remain"
    );
    assert!(
        profile_old_session_dir.join("profile-old.json").exists(),
        "old managed profile session should remain Codex-managed"
    );

    let claude_history = fs::read_to_string(shared_claude_project.join("session.jsonl"))
        .expect("shared claude history should remain");
    assert!(claude_history.contains("\"message\":\"old\""));
    assert!(claude_history.contains("\"message\":\"recent\""));
}
