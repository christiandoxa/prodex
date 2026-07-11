use super::{
    RuntimeGeminiPrecommitDecision, RuntimeGeminiPrecommitProbe,
    runtime_gemini_precommit_decision_for_data_lines,
};

#[test]
fn gemini_precommit_retries_malformed_stream_before_visible_output() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_bad",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "MALFORMED_FUNCTION_CALL"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("MALFORMED_FUNCTION_CALL".to_string())
    );
}

#[test]
fn gemini_precommit_commits_once_visible_output_exists() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_text",
            "candidates": [{
                "content": {"parts": [{"text": "hi"}]}
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_commits_native_code_execution_output() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_code",
            "candidates": [{
                "content": {"parts": [
                    {"executableCode": {"language": "PYTHON", "code": "print(4)"}},
                    {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "4"}}
                ]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_commits_thought_only_stop_as_reasoning_output() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_thought",
            "candidates": [{
                "content": {"parts": [{"text": "reasoning", "thought": true}]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_commits_max_tokens_for_codex_incomplete_event() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_truncated",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "MAX_TOKENS"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(decision, RuntimeGeminiPrecommitDecision::Commit);
}

#[test]
fn gemini_precommit_retries_empty_stop_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_empty",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}

#[test]
fn gemini_precommit_retries_internal_instruction_leak_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_leak",
            "candidates": [{
                "content": {"parts": [{
                    "text": "When RTK and Prodex Smart Context auto-wrappers conflict with a specific test runner, prefer the raw command."
                }]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}

#[test]
fn gemini_precommit_retries_optimizer_fallback_instruction_leak_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_optimizer_fallback_leak",
            "candidates": [{
                "content": {"parts": [{
                    "text": "breaks task execution, immediately drop the optimizer tool and use normal file reads/commands to complete the task. updates or basic file reads unless the user explicitly asks for optimizer diagnostics. Use optimizers for their intended job: reducing token usage of large files and deep graphs. Do not overcomplicate targeted reads or basic config debugging."
                }]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}

#[test]
fn gemini_precommit_retries_super_capabilities_instruction_leak_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_super_capabilities_leak",
            "candidates": [{
                "content": {"parts": [{
                    "text": "Never commit AST summary artifacts into source trees or merge diffs containing proxy markers.\n\nFor diagnostics, the runtime provides `prodex super check-optimizers` to verify installed paths and current optimizer integration status.\n\nThe Prodex bridge handles auto-compression of the main runtime system prompt and capabilities during launch; do not manipulate the system prompt yourself.\n\nUse these capabilities only as directed."
                }]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}

#[test]
fn gemini_precommit_retries_truncated_internal_prompt_fragment_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_truncated_internal_fragment",
            "candidates": [{
                "content": {"parts": [{
                    "text": "If reads or basic config debugging. reads or basic config debugging."
                }]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe::default();

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}

#[test]
fn gemini_precommit_retries_verbatim_system_instruction_echo_before_commit() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_system_echo",
            "candidates": [{
                "content": {"parts": [{
                    "text": "Prodex Super Mode also runs a quick background probe at startup to check rtk, claw, prodex-token-savior, and prodex-sqz versions."
                }]},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = RuntimeGeminiPrecommitProbe {
        internal_instruction_corpus: "prodex super mode also runs a quick background probe at startup to check rtk claw prodex-token-savior and prodex-sqz versions".to_string(),
        ..RuntimeGeminiPrecommitProbe::default()
    };

    let decision = runtime_gemini_precommit_decision_for_data_lines(&data, &mut probe);

    assert_eq!(
        decision,
        RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}
