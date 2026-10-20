use super::*;
use serde_json::{Value, json};

#[test]
fn mojo_payload_collection_preserves_prefix_empty_strings_and_first_present_field() {
    for (value, prefix, expected) in [
        (None, "seed", "seed"),
        (Some(json!("")), "seed", "seed\n"),
        (Some(json!(["", "", "A", ""])), "", "A\n"),
        (
            Some(json!({"output": null, "content": "ignored", "text": "ignored"})),
            "seed",
            "seed",
        ),
        (
            Some(json!({"content": [{"text": "a"}, 2, [null, {"text": "b"}]], "text": "ignored"})),
            "",
            "a\nb",
        ),
        (Some(json!("\u{2003}東京\0")), "", "\u{2003}東京\0"),
    ] {
        let output: String = tool_text::gemini_provider_core_guardrail_projection(
            &json!({"value": value, "prefix": prefix}),
            prodex_mojo_core::gemini_guardrails::GeminiGuardrailJsonOperation::PayloadText,
        );
        assert_eq!(output, expected);
    }
}

#[test]
fn mojo_verification_history_keeps_user_boundary_nonblank_window_and_exit_precedence() {
    let user = json!({"role":"user","content":"Check the result."});
    let failed = json!({"role":"tool","content":"error: unavailable"});
    let verified = json!({"role":"tool","content":"verification: ok"});
    let working = json!({"role":"tool","content":"working"});
    let zero_failure =
        json!({"role":"tool","content":"error: old message\nProcess exited with code 0"});
    for (messages, expected) in [
        (vec![], true),
        (vec![verified.clone()], true),
        (vec![json!({"role":"assistant"}), verified.clone()], false),
        (vec![user.clone(), failed.clone(), verified.clone()], true),
        (
            vec![user.clone(), failed.clone(), working, verified.clone()],
            false,
        ),
        (
            vec![
                user.clone(),
                failed.clone(),
                json!({"role":"tool","content":"\u{2003}\n"}),
                verified.clone(),
            ],
            true,
        ),
        (
            vec![user.clone(), failed, user.clone(), verified.clone()],
            false,
        ),
        (
            vec![
                user.clone(),
                json!({"role":"tool","output":null,"content":"verification: ok"}),
            ],
            true,
        ),
        (vec![user.clone(), zero_failure.clone(), verified], false),
        (vec![user.clone(), zero_failure], true),
        (
            vec![
                user.clone(),
                json!({"role":"tool","output":"verification: ok\nerror: prior\nProcess exited with code 0"}),
            ],
            true,
        ),
        (
            vec![
                user,
                json!({"role":"tool","output":[{"text":"verification: ok"}, Value::Null]}),
            ],
            false,
        ),
    ] {
        assert_eq!(
            gemini_provider_core_unverified_success_claim("Everything is complete", &messages),
            expected
        );
        assert!(!gemini_provider_core_unverified_success_claim(
            "The result is available",
            &messages
        ));
    }
}

#[test]
fn mojo_payload_selection_reaches_exact_command_output_callers() {
    let messages = [
        json!({"role":"user","content":[{"text":"Return only the command output."}]}),
        json!({"role":"tool","output":{"output":null,"content":"ignored"},"content":"ignored"}),
    ];
    assert!(gemini_provider_core_conversation_requests_command_output_only(&messages));
    assert_eq!(gemini_provider_core_forced_command_output(&messages), None);
}

#[test]
fn compiled_exact_output_selects_latest_user_and_subsequent_tool_via_mojo() {
    let request = json!({"role":"user","content":"Return only the command output."});
    assert!(!gemini_provider_core_conversation_requests_command_output_only(&[]));
    assert_eq!(gemini_provider_core_forced_command_output(&[]), None);
    assert!(gemini_provider_core_conversation_requests_command_output_only(&[request.clone()]));
    assert_eq!(
        gemini_provider_core_forced_command_output(&[request.clone()]),
        None
    );

    let messages = vec![
        json!({"role":"tool","output":"stale before user"}),
        request.clone(),
        json!({"role":"tool","content":{"text":" Output:\n  first 🚀\n\n"}}),
        json!({"role":"assistant","content":"ignore"}),
        json!({"role":"tool","output":[" Output:\n", {"text":"second 東京"}]}),
    ];
    assert!(gemini_provider_core_conversation_requests_command_output_only(&messages));
    assert_eq!(
        gemini_provider_core_forced_command_output(&messages),
        Some("second 東京".to_string())
    );

    let mut stale = messages;
    stale.push(json!({"role":"user","content":"Summarize the results."}));
    assert!(!gemini_provider_core_conversation_requests_command_output_only(&stale));
    assert_eq!(gemini_provider_core_forced_command_output(&stale), None);
    stale
        .push(json!({"role":"user","content":[{"content":"  Return only the command output.  "}]}));
    assert!(gemini_provider_core_conversation_requests_command_output_only(&stale));
    assert_eq!(gemini_provider_core_forced_command_output(&stale), None);
    stale.push(json!({"role":"tool","output":null,"content":"ignored fallback output"}));
    assert_eq!(gemini_provider_core_forced_command_output(&stale), None);
}

#[test]
fn compiled_exact_output_required_command_preserves_assistant_and_marker_precedence() {
    let user = json!({
        "role":"user",
        "content":[{"text":"Return only the command output. Run exactly:\n  echo hello  \n"}],
    });
    let tool = json!({"role":"tool","output":"Output:\n  hello  "});
    let matches = vec![
        user.clone(),
        json!({
            "role":"assistant",
            "tool_calls":[{"function":{"arguments":"{\"command\":\" echo hello \"}"}}],
        }),
        tool.clone(),
    ];
    assert_eq!(
        gemini_provider_core_forced_command_output(&matches),
        Some("hello".into())
    );

    let misses = vec![
        user.clone(),
        json!({
            "role":"assistant",
            "tool_calls":[{"function":{"arguments":"{\"cmd\":\"echo other\"}"}}],
        }),
        tool.clone(),
    ];
    assert_eq!(gemini_provider_core_forced_command_output(&misses), None);

    let another = vec![
        user.clone(),
        json!({"role":"assistant","args":{"shell_command":"echo hello"}}),
        tool.clone(),
    ];
    assert_eq!(
        gemini_provider_core_forced_command_output(&another),
        Some("hello".into())
    );

    let marker_user = json!({
        "role":"user",
        "content":"Return only the command output. Run exactly:\necho PRODEX_VERIFY_12345"
    });
    assert_eq!(
        gemini_provider_core_forced_command_output(&[
            marker_user.clone(),
            json!({"role":"tool","output":"PRODEX_VERIFY_12345 is complete"}),
        ]),
        Some("PRODEX_VERIFY_12345 is complete".into())
    );
    assert_eq!(
        gemini_provider_core_forced_command_output(&[
            marker_user,
            json!({"role":"tool","output":"prodex_verify_12345 is complete"}),
        ]),
        None
    );
    assert_eq!(
        gemini_provider_core_forced_command_output(&[
            user,
            json!({"role":"tool","output":"Success. No files changed."}),
        ]),
        None
    );
}
