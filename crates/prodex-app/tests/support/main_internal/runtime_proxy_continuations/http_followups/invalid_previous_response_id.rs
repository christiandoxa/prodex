#[test]
fn runtime_proxy_http_invalid_previous_response_id_recovers_on_same_profile_once() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _trace_guard = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_RUNTIME_RESPONSE_CHAIN_TRACE",
        "true",
    );
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_invalid_previous_response_id(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );

    let turn_one = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [{"type": "input_text", "text": "turn one"}],
    });
    let turn_two = serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [{"type": "input_text", "text": "turn two"}],
    });
    let full_history = serde_json::json!([
        turn_one.clone(),
        {"type": "message", "role": "assistant", "content": "checking"},
        {"type": "compaction", "encrypted_content": "compact-context"},
        {"type": "function_call", "call_id": "call-once", "name": "lookup", "arguments": "{}"},
        {"type": "function_call_output", "call_id": "call-once", "output": "done"},
        turn_two.clone()
    ]);

    let first = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.5",
            "reasoning": {"effort": "low"},
            "input": [turn_one],
            "client_metadata": {
                "session_id": "session-second",
                "thread_id": "thread-second",
                "turn_id": "turn-one"
            },
        }),
    );
    assert_eq!(first.status().as_u16(), 200);
    assert!(
        first
            .text()
            .expect("first response body should decode")
            .contains("\"id\":\"resp-second\"")
    );

    let second = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "user-agent",
            "codex_exec/0.148.0 (Linux; x86_64)",
        )],
        serde_json::json!({
            "model": "gpt-5.6",
            "instructions": "preserve this instruction",
            "reasoning": {"effort": "max"},
            "previous_response_id": "resp-second",
            "input": full_history,
            "client_metadata": {
                "session_id": "session-second",
                "thread_id": "thread-second",
                "turn_id": "turn-two"
            },
        }),
    );
    assert_eq!(second.status().as_u16(), 200);
    assert!(
        second
            .text()
            .expect("recovered response body should decode")
            .contains("\"id\":\"resp-second\"")
    );

    let accounts = fixture.backend.responses_accounts();
    let bodies = fixture.backend.responses_bodies();
    assert_eq!(
        accounts,
        vec![
            "second-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string()
        ],
        "an invalid incremental id must recover on its owner without rotation; bodies={bodies:?}"
    );
    assert_eq!(bodies.len(), 3);
    assert!(!bodies[0].contains("previous_response_id"));
    assert!(bodies[1].contains("\"previous_response_id\":\"resp-second\""));
    let mut expected_recovery: serde_json::Value =
        serde_json::from_str(&bodies[1]).expect("incremental request should be JSON");
    expected_recovery
        .as_object_mut()
        .expect("request should be an object")
        .remove("previous_response_id");
    let actual_recovery: serde_json::Value =
        serde_json::from_str(&bodies[2]).expect("recovery request should be JSON");
    assert_eq!(actual_recovery, expected_recovery);
    assert!(
        !bodies[2].contains("previous_response_id"),
        "full-history recovery should remove only the stale incremental id: {}",
        bodies[2]
    );
    assert!(bodies[2].contains("turn one"));
    assert!(bodies[2].contains("turn two"));
    assert!(bodies[2].contains("compact-context"));
    assert!(bodies[2].contains("\"model\":\"gpt-5.6\""));
    assert!(bodies[2].contains("\"effort\":\"max\""));
    assert_eq!(bodies[2].matches("turn two").count(), 1);
    assert_eq!(bodies[2].matches("function_call_output").count(), 1);

    let log = fixture.wait_for_log(|log| {
        log.contains("responses_continuation") && log.contains("retry_attempt=1")
    });
    let trace = log
        .lines()
        .find(|line| line.contains("responses_continuation") && line.contains("retry_attempt=1"))
        .expect("recovery trace should be present");
    for field in [
        "profile_hash=",
        "logical_provider=",
        "transport_provider_hash=",
        "session_id_hash=",
        "thread_id_hash=",
        "turn_id_hash=",
        "turn_state_hash=",
        "previous_response_id_hash=",
        "response_id_hash=",
        "transport_generation=",
        "rotation_generation=",
        "compaction_generation=",
        "full_context_request=true",
        "stream_committed=",
    ] {
        assert!(trace.contains(field), "missing {field}: {trace}");
    }
    assert!(!trace.contains("resp-second"), "{trace}");
    assert!(!trace.contains("session-second"), "{trace}");
    assert!(!trace.contains("thread-second"), "{trace}");
}

#[test]
fn runtime_proxy_http_invalid_previous_response_id_stops_after_one_recovery() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_always_invalid_previous_response_id(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );
    let first = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
            "input": [{"type": "message", "role": "user", "content": "turn one"}],
            "client_metadata": {"session_id": "session-second"},
        }),
    );
    assert_eq!(first.status().as_u16(), 200);
    let _ = first.text().expect("first response body should decode");

    let second = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "user-agent",
            "codex_exec/0.148.0 (Linux; x86_64)",
        )],
        serde_json::json!({
            "model": "gpt-5.6",
            "previous_response_id": "resp-second",
            "input": [
                {"type": "message", "role": "user", "content": "turn one"},
                {"type": "message", "role": "assistant", "content": "turn one result"},
                {"type": "message", "role": "user", "content": "turn two"},
            ],
            "client_metadata": {"session_id": "session-second"},
        }),
    );
    assert_eq!(second.status().as_u16(), 400);
    assert!(second
        .text()
        .expect("invalid response body should decode")
        .contains("Invalid `previous_response_id`."));

    let accounts = fixture.backend.responses_accounts();
    let bodies = fixture.backend.responses_bodies();
    assert_eq!(accounts, vec!["second-account"; 3]);
    assert_eq!(bodies.len(), 3, "invalid ID recovery must run at most once");
    assert!(!bodies[2].contains("previous_response_id"));
}

#[test]
fn runtime_proxy_http_stale_overlay_resume_after_restart_recovers_chain_once() {
    let session_id = "01a01824-29f7-7332-96c7-5d09044ee2d0";
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_invalid_previous_response_id(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );
    let first = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
            "input": [{"type": "message", "role": "user", "content": "turn one"}],
            "client_metadata": {"session_id": session_id},
        }),
    );
    assert!(first.text().unwrap().contains("resp-second"));

    let relative = format!("sessions/2026/08/19/rollout-{session_id}.jsonl");
    let rollout_path = fixture.paths.shared_codex_root.join(&relative);
    std::fs::create_dir_all(rollout_path.parent().unwrap()).unwrap();
    std::fs::write(
        &rollout_path,
        format!(
            "{{\"timestamp\":\"2026-08-19T10:50:18Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"session_id\":\"{session_id}\",\"timestamp\":\"2026-08-19T10:50:18Z\",\"cwd\":\"/home/test-user/project\",\"originator\":\"codex-cli\",\"cli_version\":\"0.148.0\",\"model_provider\":\"openai\"}}}}\n"
        ),
    )
    .unwrap();
    let stale_path = fixture
        .paths
        .shared_codex_root
        .with_file_name(".prodex-overlay-old")
        .join(&relative);
    let database_path = fixture.paths.shared_codex_root.join("state_5.sqlite");
    let database = rusqlite::Connection::open(&database_path).unwrap();
    database
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, stale_path.display().to_string()],
        )
        .unwrap();
    drop(database);
    crate::app_commands::runtime_launch::resume_repair::repair_resume_session_in_shared_home(
        &fixture.paths.shared_codex_root,
        &["resume".into(), session_id.into()],
    )
    .unwrap();
    let database = rusqlite::Connection::open(database_path).unwrap();
    let repaired_path: String = database
        .query_row(
            "SELECT rollout_path FROM threads WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(repaired_path, rollout_path.display().to_string());

    let fixture = fixture.restart();
    let resumed = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "user-agent",
            "codex_exec/0.148.0 (Linux; x86_64)",
        )],
        serde_json::json!({
            "model": "gpt-5.6",
            "previous_response_id": "resp-second",
            "input": [
                {"type": "message", "role": "user", "content": "turn one"},
                {"type": "message", "role": "assistant", "content": "answer one"},
                {"type": "message", "role": "user", "content": "turn two"}
            ],
            "client_metadata": {"session_id": session_id},
        }),
    );

    assert_eq!(resumed.status().as_u16(), 200);
    assert!(resumed.text().unwrap().contains("resp-second"));
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["second-account".to_string(); 3]
    );
    let bodies = fixture.backend.responses_bodies();
    assert!(bodies[1].contains("previous_response_id"));
    assert!(!bodies[2].contains("previous_response_id"));
    assert_eq!(bodies[2].matches("turn two").count(), 1);
}

#[test]
fn runtime_proxy_http_invalid_previous_response_id_workaround_is_off_for_0_147() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_invalid_previous_response_id(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );
    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "user-agent",
            "codex-cli/0.147.0",
        )],
        serde_json::json!({
            "model": "gpt-5.6",
            "previous_response_id": "resp-second",
            "input": [
                {"type": "message", "role": "assistant", "content": "turn one result"},
                {"type": "message", "role": "user", "content": "turn two"}
            ],
            "client_metadata": {"session_id": "session-second"},
        }),
    );

    assert_eq!(response.status().as_u16(), 400);
    assert_eq!(fixture.backend.responses_bodies().len(), 1);
}

#[test]
fn runtime_proxy_http_sse_invalid_previous_response_id_recovers_once_without_rotation() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_sse_invalid_previous_response_id(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );

    let first = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
            "input": [{"type": "message", "role": "user", "content": "turn one"}],
            "client_metadata": {"session_id": "session-second"},
        }),
    );
    assert_eq!(first.status().as_u16(), 200);
    assert!(first.text().expect("first response body should decode").contains("resp-second"));

    let second = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "user-agent",
            "codex_exec/0.148.0 (Linux; x86_64)",
        )],
        serde_json::json!({
            "model": "gpt-5.6",
            "previous_response_id": "resp-second",
            "input": [
                {"type": "message", "role": "user", "content": "turn one"},
                {"type": "message", "role": "assistant", "content": "answer one"},
                {"type": "message", "role": "user", "content": "turn two"},
            ],
            "client_metadata": {"session_id": "session-second"},
        }),
    );
    assert_eq!(second.status().as_u16(), 200);
    let second_body = second
        .text()
        .expect("recovered SSE response body should decode");
    assert!(second_body.contains("resp-second"), "{second_body}");

    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["second-account".to_string(); 3],
        "an SSE invalid-id failure must recover once on the chain owner"
    );
    let bodies = fixture.backend.responses_bodies();
    assert!(bodies[1].contains("previous_response_id"));
    assert!(!bodies[2].contains("previous_response_id"));
    assert_eq!(bodies[2].matches("turn one").count(), 1);
    assert_eq!(bodies[2].matches("answer one").count(), 1);
    assert_eq!(bodies[2].matches("turn two").count(), 1);
}
