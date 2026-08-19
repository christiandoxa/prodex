#[test]
fn runtime_proxy_http_invalid_previous_response_id_recovers_on_same_profile_once() {
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
    let full_history = serde_json::json!([turn_one.clone(), turn_two.clone()]);

    let first = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
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

    let second = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
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
    assert!(
        !bodies[2].contains("previous_response_id"),
        "full-history recovery should remove only the stale incremental id: {}",
        bodies[2]
    );
    assert!(bodies[2].contains("turn one"));
    assert!(bodies[2].contains("turn two"));
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

    let second = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
            "previous_response_id": "resp-second",
            "input": [
                {"type": "message", "role": "user", "content": "turn one"},
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
fn runtime_proxy_http_sse_invalid_previous_response_id_does_not_rotate() {
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

    let second = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.6",
            "previous_response_id": "resp-second",
            "input": [
                {"type": "message", "role": "user", "content": "turn one"},
                {"type": "message", "role": "user", "content": "turn two"},
            ],
            "client_metadata": {"session_id": "session-second"},
        }),
    );
    assert_eq!(second.status().as_u16(), 200);
    assert!(second
        .text()
        .expect("SSE error body should decode")
        .contains("Invalid `previous_response_id`."));

    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["second-account".to_string(), "second-account".to_string()],
        "an SSE invalid-id failure must not re-enter generic rotation"
    );
}
