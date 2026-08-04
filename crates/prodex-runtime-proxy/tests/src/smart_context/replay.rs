use super::*;

#[test]
fn replay_corpus_fixture_contains_inputs_only() {
    let text = include_str!("../../fixtures/smart_context_replay_corpus.json");
    let corpus = smart_context_parse_replay_corpus_json(text).expect("valid inputs-only corpus");

    assert_eq!(
        corpus.schema_version,
        SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION
    );
    assert!(corpus.scenarios.len() >= 10);
    assert!(
        corpus
            .scenarios
            .iter()
            .any(|scenario| scenario.turns.len() >= 30)
    );
    assert!(
        [16_384, 32_768, 131_072, 200_000]
            .into_iter()
            .all(|window| corpus
                .scenarios
                .iter()
                .any(|scenario| scenario.context_window_tokens == window))
    );
}

#[test]
fn replay_corpus_rejects_prefilled_output_metrics() {
    let error = smart_context_parse_replay_corpus_json(
        r#"{
            "schema_version": 2,
            "scenarios": [{
                "id": "self-asserted",
                "transport": "http",
                "route": "responses",
                "provider": "openai",
                "model": "gpt-5.1-codex",
                "context_window_tokens": 16384,
                "mode": "active",
                "turns": [{
                    "request": {"model": "gpt-5.1-codex", "input": []},
                    "expected_outcome": "pass_through",
                    "input_tokens": 1000
                }]
            }]
        }"#,
    )
    .expect_err("output metrics must not be accepted as corpus input");

    assert!(error.contains("unknown field `input_tokens`"), "{error}");
}

#[test]
fn replay_corpus_rejects_duplicate_scenario_ids() {
    let error = smart_context_parse_replay_corpus_json(
        r#"{
            "schema_version": 2,
            "scenarios": [
                {
                    "id": "duplicate",
                    "transport": "http",
                    "route": "responses",
                    "provider": "openai",
                    "model": "gpt-5.1-codex",
                    "context_window_tokens": 16384,
                    "mode": "exact",
                    "turns": [{
                        "request": {"model": "gpt-5.1-codex"},
                        "expected_outcome": "pass_through"
                    }]
                },
                {
                    "id": "duplicate",
                    "transport": "websocket",
                    "route": "websocket",
                    "provider": "openai",
                    "model": "gpt-5.1-codex",
                    "context_window_tokens": 16384,
                    "mode": "shadow",
                    "turns": [{
                        "request": {"model": "gpt-5.1-codex"},
                        "expected_outcome": "pass_through"
                    }]
                }
            ]
        }"#,
    )
    .expect_err("duplicate ids must be rejected");

    assert!(error.contains("duplicate Smart Context replay scenario id duplicate"));
}

fn replay_test_scenario_with_request(
    id: String,
    group: Option<String>,
    request: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "transport": "http",
        "route": "responses",
        "provider": "openai",
        "model": "gpt-5.1-codex",
        "context_window_tokens": 16384,
        "mode": "exact",
        "concurrent_group": group,
        "turns": [{"request": request, "expected_outcome": "pass_through"}]
    })
}

fn replay_test_scenario(id: String, group: String) -> serde_json::Value {
    replay_test_scenario_with_request(
        id,
        Some(group),
        serde_json::json!({"model": "gpt-5.1-codex"}),
    )
}

fn replay_test_request_with_input_length(length: usize) -> serde_json::Value {
    serde_json::json!({
        "model": "gpt-5.1-codex",
        "input": "x".repeat(length)
    })
}

fn replay_test_scenario_with_serialized_bytes(
    id: &str,
    group: Option<&str>,
    bytes: usize,
) -> serde_json::Value {
    let base = replay_test_scenario_with_request(
        id.to_string(),
        group.map(str::to_string),
        serde_json::json!({"model": "gpt-5.1-codex", "input": ""}),
    );
    let fixed_bytes = serde_json::to_vec(
        &serde_json::from_value::<SmartContextReplayScenarioInput>(base).unwrap(),
    )
    .unwrap()
    .len();
    assert!(bytes >= fixed_bytes);
    let scenario = replay_test_scenario_with_request(
        id.to_string(),
        group.map(str::to_string),
        replay_test_request_with_input_length(bytes - fixed_bytes),
    );
    let serialized_bytes = serde_json::to_vec(
        &serde_json::from_value::<SmartContextReplayScenarioInput>(scenario.clone()).unwrap(),
    )
    .unwrap()
    .len();
    assert_eq!(serialized_bytes, bytes);
    scenario
}

fn replay_test_scenario_with_total_serialized_bytes(bytes: usize) -> serde_json::Value {
    let base = replay_test_scenario_with_request(
        "total".to_string(),
        None,
        serde_json::json!({"model": "gpt-5.1-codex", "input": ""}),
    );
    let base_corpus =
        serde_json::from_str::<SmartContextReplayCorpus>(&replay_test_corpus(vec![base.clone()]))
            .unwrap();
    let fixed_bytes = serde_json::to_vec(&base_corpus).unwrap().len();
    assert!(bytes >= fixed_bytes);
    let scenario = replay_test_scenario_with_request(
        "total".to_string(),
        None,
        replay_test_request_with_input_length(bytes - fixed_bytes),
    );
    let corpus = serde_json::from_str::<SmartContextReplayCorpus>(&replay_test_corpus(vec![
        scenario.clone(),
    ]))
    .unwrap();
    assert_eq!(serde_json::to_vec(&corpus).unwrap().len(), bytes);
    scenario
}

fn replay_test_artifact_request(reference_count: usize) -> serde_json::Value {
    let references = (0..reference_count)
        .map(|index| format!("psc2:{index:064x}"))
        .collect::<Vec<_>>()
        .join(" ");
    serde_json::json!({
        "model": "gpt-5.1-codex",
        "input": [{
            "type": "message",
            "role": "user",
            "content": references
        }]
    })
}

fn replay_test_corpus(scenarios: Vec<serde_json::Value>) -> String {
    serde_json::json!({
        "schema_version": SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION,
        "scenarios": scenarios,
    })
    .to_string()
}

#[test]
fn replay_corpus_accepts_exact_concurrent_group_limits() {
    let scenarios = (0..SMART_CONTEXT_REPLAY_MAX_SCENARIOS_PER_CONCURRENT_GROUP)
        .map(|index| replay_test_scenario(format!("member-{index}"), "group".to_string()))
        .collect();

    smart_context_parse_replay_corpus_json(&replay_test_corpus(scenarios))
        .expect("exact concurrent-group membership bound should be accepted");
}

#[test]
fn replay_corpus_rejects_concurrent_group_over_bound() {
    let scenarios = (0..=SMART_CONTEXT_REPLAY_MAX_SCENARIOS_PER_CONCURRENT_GROUP)
        .map(|index| replay_test_scenario(format!("member-{index}"), "group".to_string()))
        .collect();

    let error = smart_context_parse_replay_corpus_json(&replay_test_corpus(scenarios))
        .expect_err("concurrent-group membership over bound should fail");

    assert!(error.contains("exceeds 64 scenarios"), "{error}");
}

#[test]
fn replay_corpus_rejects_concurrent_group_count_over_bound() {
    let scenarios = (0..=SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUPS)
        .flat_map(|group| {
            [
                replay_test_scenario(format!("group-{group}-a"), format!("group-{group}")),
                replay_test_scenario(format!("group-{group}-b"), format!("group-{group}")),
            ]
        })
        .collect();

    let error = smart_context_parse_replay_corpus_json(&replay_test_corpus(scenarios))
        .expect_err("concurrent-group count over bound should fail");

    assert!(error.contains("exceeds 64 concurrent groups"), "{error}");
}

#[test]
fn replay_corpus_accepts_exact_concurrent_group_count_limit() {
    let scenarios = (0..SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUPS)
        .flat_map(|group| {
            [
                replay_test_scenario(format!("group-{group}-a"), format!("group-{group}")),
                replay_test_scenario(format!("group-{group}-b"), format!("group-{group}")),
            ]
        })
        .collect();

    smart_context_parse_replay_corpus_json(&replay_test_corpus(scenarios))
        .expect("exact concurrent-group count bound should be accepted");
}

#[test]
fn replay_corpus_rejects_concurrent_group_name_over_bound() {
    let exact_group = "g".repeat(SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUP_NAME_BYTES);
    let over_group = "g".repeat(SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUP_NAME_BYTES + 1);
    smart_context_parse_replay_corpus_json(&replay_test_corpus(vec![
        replay_test_scenario("exact-a".to_string(), exact_group.clone()),
        replay_test_scenario("exact-b".to_string(), exact_group),
    ]))
    .expect("exact concurrent-group name bound should be accepted");

    let error = smart_context_parse_replay_corpus_json(&replay_test_corpus(vec![
        replay_test_scenario("over-a".to_string(), over_group.clone()),
        replay_test_scenario("over-b".to_string(), over_group),
    ]))
    .expect_err("concurrent-group name over bound should fail");

    assert!(error.contains("exceeds 128 bytes"), "{error}");
}

#[test]
fn replay_corpus_accepts_exact_concurrent_group_serialized_bytes() {
    let first = SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP / 2;
    let scenarios = vec![
        replay_test_scenario_with_serialized_bytes("group-a", Some("group"), first),
        replay_test_scenario_with_serialized_bytes(
            "group-b",
            Some("group"),
            SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP - first,
        ),
    ];

    smart_context_parse_replay_corpus_json(&replay_test_corpus(scenarios))
        .expect("exact concurrent-group serialized-byte bound should be accepted");
}

#[test]
fn replay_corpus_rejects_concurrent_group_serialized_bytes_over_bound() {
    let first = SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP / 2;
    let scenarios = vec![
        replay_test_scenario_with_serialized_bytes("group-a", Some("group"), first),
        replay_test_scenario_with_serialized_bytes(
            "group-b",
            Some("group"),
            SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP - first + 1,
        ),
    ];

    let error = smart_context_parse_replay_corpus_json(&replay_test_corpus(scenarios))
        .expect_err("concurrent-group serialized-byte over bound should fail");

    let expected = format!(
        "exceeds {} serialized bytes",
        SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP
    );
    assert!(error.contains(&expected), "{error}");
}

#[test]
fn replay_corpus_accepts_exact_total_serialized_bytes() {
    let scenario = replay_test_scenario_with_total_serialized_bytes(
        SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES,
    );

    smart_context_parse_replay_corpus_json(&replay_test_corpus(vec![scenario]))
        .expect("exact total serialized-byte bound should be accepted");
}

#[test]
fn replay_corpus_rejects_total_serialized_bytes_over_bound() {
    let scenario = replay_test_scenario_with_total_serialized_bytes(
        SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES + 1,
    );

    let error = smart_context_parse_replay_corpus_json(&replay_test_corpus(vec![scenario]))
        .expect_err("total serialized-byte over bound should fail");

    let expected = format!(
        "exceeds {} total serialized bytes",
        SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES
    );
    assert!(error.contains(&expected), "{error}");
}

#[test]
fn replay_corpus_accepts_exact_artifact_reference_count() {
    let scenario = replay_test_scenario_with_request(
        "references".to_string(),
        None,
        replay_test_artifact_request(SMART_CONTEXT_REPLAY_MAX_ARTIFACT_REFERENCES),
    );

    smart_context_parse_replay_corpus_json(&replay_test_corpus(vec![scenario]))
        .expect("exact artifact/reference bound should be accepted");
}

#[test]
fn replay_corpus_rejects_artifact_reference_count_over_bound() {
    let scenario = replay_test_scenario_with_request(
        "references".to_string(),
        None,
        replay_test_artifact_request(SMART_CONTEXT_REPLAY_MAX_ARTIFACT_REFERENCES + 1),
    );

    let error = smart_context_parse_replay_corpus_json(&replay_test_corpus(vec![scenario]))
        .expect_err("artifact/reference over bound should fail");

    let expected = format!(
        "exceeds {} artifact/reference count",
        SMART_CONTEXT_REPLAY_MAX_ARTIFACT_REFERENCES
    );
    assert!(error.contains(&expected), "{error}");
}
