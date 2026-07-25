use super::*;

#[test]
fn replay_corpus_fixture_contains_inputs_only() {
    let text = include_str!("../../fixtures/smart_context_replay_corpus.json");
    let corpus = smart_context_parse_replay_corpus_json(text).expect("valid inputs-only corpus");

    assert_eq!(
        corpus.schema_version,
        SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION
    );
    assert_eq!(corpus.scenarios.len(), 1);
    assert_eq!(corpus.scenarios[0].turns.len(), 1);
}

#[test]
fn replay_corpus_rejects_prefilled_output_metrics() {
    let error = smart_context_parse_replay_corpus_json(
        r#"{
            "schema_version": 1,
            "scenarios": [{
                "id": "self-asserted",
                "transport": "http",
                "provider": "openai",
                "model": "gpt-5.1-codex",
                "context_window_tokens": 16384,
                "mode": "active",
                "turns": [{
                    "request": {"model": "gpt-5.1-codex", "input": []},
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
            "schema_version": 1,
            "scenarios": [
                {
                    "id": "duplicate",
                    "transport": "http",
                    "provider": "openai",
                    "model": "gpt-5.1-codex",
                    "context_window_tokens": 16384,
                    "mode": "exact",
                    "turns": [{"request": {"model": "gpt-5.1-codex"}}]
                },
                {
                    "id": "duplicate",
                    "transport": "websocket",
                    "provider": "openai",
                    "model": "gpt-5.1-codex",
                    "context_window_tokens": 16384,
                    "mode": "shadow",
                    "turns": [{"request": {"model": "gpt-5.1-codex"}}]
                }
            ]
        }"#,
    )
    .expect_err("duplicate ids must be rejected");

    assert!(error.contains("duplicate Smart Context replay scenario id duplicate"));
}
