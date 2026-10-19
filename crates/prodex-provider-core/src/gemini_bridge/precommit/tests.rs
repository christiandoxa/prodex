use super::*;
use serde_json::{Value, json};

fn classify(
    value: Value,
    probe: &mut GeminiProviderCorePrecommitProbe,
) -> GeminiProviderCorePrecommitDecision {
    gemini_provider_core_precommit_decision_for_data_lines(&[value.to_string()], probe)
}

#[test]
fn mojo_precommit_preserves_candidate_order_flags_and_early_returns() {
    for value in [
        json!({"error": null}),
        json!({"error": false}),
        json!({"promptFeedback": {"blockReason": " blocked "}}),
    ] {
        let mut value = value;
        value["candidates"] =
            json!([{"content": {"parts": [{"text": "reason", "thought": true}]}}]);
        let mut probe = GeminiProviderCorePrecommitProbe::default();
        assert_eq!(
            classify(value, &mut probe),
            GeminiProviderCorePrecommitDecision::Commit
        );
        assert!(!probe.visible_output && !probe.reasoning_output);
    }
    for key in ["webSearchQueries", "groundingChunks"] {
        let value = json!({"candidates": [{"groundingMetadata": {key: [null]}, "content": {"parts": [{"text": "reason", "thought": true}]}}]});
        let mut probe = GeminiProviderCorePrecommitProbe::default();
        assert_eq!(
            classify(value, &mut probe),
            GeminiProviderCorePrecommitDecision::Commit
        );
        assert!(probe.visible_output && !probe.reasoning_output);
    }
    for (parts, visible_before, reasoning_after) in [
        (
            json!([{"text": "reason", "thought": true}, {"text": "visible"}]),
            false,
            true,
        ),
        (
            json!([{"text": "visible"}, {"text": "reason", "thought": true}]),
            false,
            false,
        ),
        (json!([{"text": "reason", "thought": true}]), true, true),
    ] {
        let mut probe = GeminiProviderCorePrecommitProbe {
            visible_output: visible_before,
            ..Default::default()
        };
        assert_eq!(
            classify(
                json!({"response": {"candidates": [{"content": {"parts": parts}, "finishReason": "OTHER"}]}}),
                &mut probe
            ),
            GeminiProviderCorePrecommitDecision::Commit
        );
        assert!(probe.visible_output);
        assert_eq!(probe.reasoning_output, reasoning_after);
    }
    let mut probe = GeminiProviderCorePrecommitProbe::default();
    assert_eq!(
        classify(
            json!({"candidates": [{"finishReason": "STOP"}, {"content": {"parts": [{"text": "ignored"}]}}]}),
            &mut probe
        ),
        GeminiProviderCorePrecommitDecision::RetryableInvalid("gemini_empty_response".into())
    );
    assert!(!probe.visible_output);
}

#[test]
fn mojo_precommit_reuses_media_and_instruction_classifiers() {
    for part in [
        json!({"functionCall": null}),
        json!({"functionCall": false}),
        json!({"inlineData": {"data": ""}}),
        json!({"fileData": {"fileUri": "https://example.com/a.png"}}),
        json!({"text": "data:image/png;base64,AA=="}),
        json!({"executableCode": {"code": "print(4)"}}),
        json!({"codeExecutionResult": null}),
        json!({"videoMetadata": false}),
        json!({"text": "\u{2003}\t"}),
    ] {
        let mut probe = GeminiProviderCorePrecommitProbe::default();
        assert_eq!(
            classify(
                json!({"candidates": [{"content": {"parts": [part]}, "finishReason": "OTHER"}]}),
                &mut probe
            ),
            GeminiProviderCorePrecommitDecision::Commit
        );
        assert!(probe.visible_output);
    }
    let leak = "When RTK and Prodex Smart Context auto-wrappers conflict with a specific test runner, prefer the raw command.";
    for (text, corpus, visible) in [
        (leak.to_string(), "", false),
        (format!("{leak}\n\nSafe answer."), "", true),
        (
            "alpha beta gamma delta epsilon zeta eta theta".to_string(),
            "alpha beta gamma delta epsilon zeta eta theta",
            false,
        ),
    ] {
        let mut probe = GeminiProviderCorePrecommitProbe {
            internal_instruction_corpus: corpus.to_string(),
            ..Default::default()
        };
        let decision = classify(
            json!({"candidates": [{"content": {"parts": [{"text": text}]}, "finishReason": "STOP"}]}),
            &mut probe,
        );
        assert_eq!(
            decision,
            if visible {
                GeminiProviderCorePrecommitDecision::Commit
            } else {
                GeminiProviderCorePrecommitDecision::RetryableInvalid(
                    "gemini_empty_response".into(),
                )
            }
        );
        assert_eq!(probe.visible_output, visible);
    }
}

#[test]
fn mojo_precommit_keeps_sse_acquisition_and_terminal_behavior() {
    for (lines, expected) in [
        (vec!["broken".to_string()], GeminiProviderCorePrecommitDecision::Commit),
        (vec!["broken".to_string(), "{}".to_string()], GeminiProviderCorePrecommitDecision::Continue),
        (vec!["[DONE]".to_string()], GeminiProviderCorePrecommitDecision::RetryableInvalid("gemini_empty_response".into())),
        (vec![json!({"candidates": [{"content": {"parts": [{"text": "reason", "thought": true}]}}]}).to_string(), json!({"candidates": [{"finishReason": "STOP"}]}).to_string()], GeminiProviderCorePrecommitDecision::Commit),
    ] {
        assert_eq!(gemini_provider_core_precommit_decision_for_data_lines(&lines, &mut GeminiProviderCorePrecommitProbe::default()), expected);
    }
}

#[test]
#[should_panic(expected = "Mojo Gemini pre-commit policy returned invalid output")]
fn mojo_precommit_preserves_rejection_of_nonobject_parts() {
    classify(
        json!({"candidates": [{"content": {"parts": [null]}}]}),
        &mut GeminiProviderCorePrecommitProbe::default(),
    );
}

#[test]
fn mojo_precommit_terminal_frames_preserve_probe_and_case_sensitive_markers() {
    use GeminiProviderCorePrecommitDecision::{Commit, Continue, RetryableInvalid};
    let done = [
        RetryableInvalid("gemini_empty_response".to_string()),
        Commit,
        Commit,
        Commit,
    ];
    let unparsed = [Commit, Commit, Commit, Commit];
    for (lines, expected) in [
        (vec!["\u{2003}[DONE]\r\n\u{a0}".to_string()], &done),
        (
            vec!["\t".to_string(), "[DONE]".to_string(), " ".to_string()],
            &done,
        ),
        (vec!["[done]".to_string()], &unparsed),
        (vec!["broken".to_string()], &unparsed),
        (vec![], &unparsed),
    ] {
        for (flags, expected) in expected.iter().enumerate() {
            let mut probe = GeminiProviderCorePrecommitProbe {
                visible_output: flags & 1 != 0,
                reasoning_output: flags & 2 != 0,
                internal_instruction_corpus: "synthetic corpus".to_string(),
            };
            assert_eq!(
                &gemini_provider_core_precommit_decision_for_data_lines(&lines, &mut probe),
                expected
            );
            assert_eq!(
                (
                    probe.visible_output,
                    probe.reasoning_output,
                    probe.internal_instruction_corpus.as_str()
                ),
                (flags & 1 != 0, flags & 2 != 0, "synthetic corpus")
            );
        }
    }
    let mut probe = GeminiProviderCorePrecommitProbe::default();
    assert_eq!(
        gemini_provider_core_precommit_decision_for_data_lines(
            &["{\"response\": null}".to_string(), "[DONE]".to_string()],
            &mut probe
        ),
        Continue
    );
}
