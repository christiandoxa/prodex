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
