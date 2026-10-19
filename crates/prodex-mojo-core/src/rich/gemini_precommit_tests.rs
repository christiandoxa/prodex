use super::*;

fn response<'a>(text: &'a str, thought: bool, reason: &'a str) -> [JsonNode<'a>; 9] {
    let node = |kind, first_child, next_sibling, parent, key, text| JsonNode {
        kind,
        first_child,
        next_sibling,
        parent,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    };
    [
        node(JsonKind::Object, Some(1), None, None, "", ""),
        node(JsonKind::Array, Some(2), None, Some(0), "candidates", ""),
        node(JsonKind::Object, Some(3), None, Some(1), "", ""),
        node(JsonKind::Object, Some(4), Some(8), Some(2), "content", ""),
        node(JsonKind::Array, Some(5), None, Some(3), "parts", ""),
        node(JsonKind::Object, Some(6), None, Some(4), "", ""),
        node(JsonKind::String, None, Some(7), Some(5), "text", text),
        node(
            if thought {
                JsonKind::True
            } else {
                JsonKind::False
            },
            None,
            None,
            Some(5),
            "thought",
            "",
        ),
        node(
            JsonKind::String,
            None,
            None,
            Some(2),
            "finishReason",
            reason,
        ),
    ]
}

#[test]
fn compiled_precommit_preserves_flags_and_finish_reason_bytes() {
    for (text, thought, reason, decision, visible, reasoning) in [
        (
            "",
            false,
            "STOP",
            GeminiPrecommitDecision::RetryableInvalid("gemini_empty_response"),
            false,
            false,
        ),
        (
            "",
            false,
            "OTHER",
            GeminiPrecommitDecision::RetryableInvalid("OTHER"),
            false,
            false,
        ),
        (
            "",
            false,
            " STOP ",
            GeminiPrecommitDecision::Commit,
            false,
            false,
        ),
        (
            "",
            false,
            "\u{2003}",
            GeminiPrecommitDecision::Continue,
            false,
            false,
        ),
        (
            "\t ",
            false,
            "OTHER",
            GeminiPrecommitDecision::Commit,
            true,
            false,
        ),
        (
            "\t ",
            true,
            "STOP",
            GeminiPrecommitDecision::Commit,
            false,
            true,
        ),
        (
            "reason",
            true,
            "",
            GeminiPrecommitDecision::Continue,
            false,
            true,
        ),
    ] {
        assert_eq!(
            gemini_precommit_plan(
                &response(text, thought, reason),
                "",
                "",
                GeminiPrecommitState::default()
            ),
            Ok(GeminiPrecommitPlan {
                decision,
                state: GeminiPrecommitState {
                    visible_output: visible,
                    reasoning_output: reasoning
                },
            })
        );
    }
    let state = GeminiPrecommitState {
        visible_output: true,
        reasoning_output: false,
    };
    assert_eq!(
        gemini_precommit_plan(&response("reason", true, "OTHER"), "", "", state),
        Ok(GeminiPrecommitPlan {
            decision: GeminiPrecommitDecision::Commit,
            state: GeminiPrecommitState {
                visible_output: true,
                reasoning_output: true
            },
        })
    );
}

#[test]
fn compiled_precommit_rejects_invalid_tree_and_abi_without_output_writes() {
    let nodes = response("text", false, "STOP");
    let input = ffi_nodes(&nodes, "").unwrap();
    let mut scratch = [0_u8; 4];
    for (
        abi,
        count,
        raw,
        raw_length,
        corpus,
        corpus_length,
        visible,
        scratch_length,
        output_offset,
    ) in [
        (2, 9, 0, 0, 0, 0, 0, 4, 0),
        (1, 0, 0, 0, 0, 0, 0, 4, 0),
        (1, 9, 0, 1, 0, 0, 0, 4, 0),
        (1, 9, 0, 0, 0, 1, 0, 4, 0),
        (1, 9, 0, 0, 0, 0, 2, 4, 0),
        (1, 9, 0, 0, 0, 0, 0, 3, 0),
        (1, 9, 0, 0, 0, 0, 0, 4, 1),
    ] {
        let mut output = [71_i64; 4];
        let status = unsafe {
            prodex_mojo_gemini_precommit_v1(
                abi,
                input.as_ptr() as u64,
                count,
                raw,
                raw_length,
                corpus,
                corpus_length,
                visible,
                0,
                scratch.as_mut_ptr() as u64,
                scratch_length,
                output.as_mut_ptr() as u64 + output_offset,
            )
        };
        assert_eq!(status, if abi == 2 { 4 } else { 1 });
        assert_eq!(output, [71; 4]);
    }
    let mut invalid = nodes;
    invalid[4].first_child = Some(2);
    assert_eq!(
        gemini_precommit_plan(&invalid, "", "", GeminiPrecommitState::default()),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_precommit_frame_decisions_preserve_terminal_and_acquisition_precedence() {
    use GeminiPrecommitDecision::{Commit, Continue, RetryableInvalid};
    let done = [
        RetryableInvalid("gemini_empty_response"),
        Commit,
        Commit,
        Commit,
    ];
    let unparsed = [Commit, Commit, Commit, Commit];
    let continuing = [Continue, Continue, Continue, Continue];
    for (data, parsed_any, expected) in [
        ("[DONE]", false, &done),
        ("\u{2003}\t[DONE]\r\n\u{a0}", false, &done),
        ("[DONE]", true, &done),
        ("[done]", false, &unparsed),
        ("prefix[DONE]", false, &unparsed),
        ("broken", false, &unparsed),
        ("broken\n{}", true, &continuing),
        ("", false, &unparsed),
    ] {
        for (flags, expected) in expected.iter().enumerate() {
            let state = GeminiPrecommitState {
                visible_output: flags & 1 != 0,
                reasoning_output: flags & 2 != 0,
            };
            assert_eq!(
                gemini_precommit_frame_decision(GeminiPrecommitFrame { data, parsed_any }, state)
                    .as_ref(),
                Ok(expected)
            );
        }
    }
}

#[test]
fn compiled_precommit_frame_abi_rejects_invalid_utf8_flags_and_versions() {
    let invalid = [0xff_u8];
    for (abi, address, length, parsed_any, visible, reasoning, expected) in [
        (2, 0, 0, 0, 0, 0, -4),
        (1, 0, -1, 0, 0, 0, -1),
        (1, 0, 1, 0, 0, 0, -1),
        (1, invalid.as_ptr() as u64, 1, 0, 0, 0, -1),
        (1, 0, 0, 2, 0, 0, -1),
        (1, 0, 0, 0, -1, 0, -1),
        (1, 0, 0, 0, 0, 2, -1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_gemini_precommit_frame_v1(
                    abi, address, length, parsed_any, visible, reasoning,
                )
            },
            expected
        );
    }
}
