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
        let mut output = prefix.to_string();
        tool_text::gemini_provider_core_collect_payload_text(value.as_ref(), &mut output);
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
