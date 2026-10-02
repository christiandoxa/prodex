use super::*;

fn completed_tool_call_input<'a>(
    call_id: &'a str,
    name: &'a str,
    arguments: &'a str,
) -> GeminiResponseKernelInput<'a> {
    let mut input =
        GeminiResponseKernelInput::new(GeminiResponseKernelOperation::StreamCompletedToolCallItem);
    input.call_id = Some(call_id);
    input.name = Some(name);
    input.arguments = Some(arguments);
    input
}

#[test]
fn stream_completed_tool_call_item_emits_exact_mojo_bytes_for_each_branch() {
    let mut ordinary = completed_tool_call_input("call_1", "plain_tool", r#"{"text":"雪"}"#);
    ordinary.signature = Some("signature-🌱");
    let expected_ordinary = r#"{"type":"function_call","call_id":"call_1","name":"plain_tool","arguments":"{\"text\":\"雪\"}","gemini_thought_signature":"signature-🌱"}"#;
    assert_eq!(
        gemini_response_kernel(ordinary).unwrap(),
        expected_ordinary.as_bytes(),
    );

    let mut tool_search =
        completed_tool_call_input("call_search", "tool_search", r#"{"query":"sqz tools"}"#);
    tool_search.created_at_present = true;
    assert_eq!(
        gemini_response_kernel(tool_search).unwrap(),
        br#"{"type":"tool_search_call","call_id":"call_search","execution":"client","arguments":{"query":"sqz tools"}}"#,
    );

    let mut malformed_search = completed_tool_call_input("call_raw", "tool_search", "not-json");
    malformed_search.signature = Some("sig");
    assert_eq!(
        gemini_response_kernel(malformed_search).unwrap(),
        br#"{"type":"function_call","call_id":"call_raw","name":"tool_search","arguments":"not-json","gemini_thought_signature":"sig"}"#,
    );

    let mut apply_patch = completed_tool_call_input("call_patch", "apply_patch", "{}");
    apply_patch.created_at_present = true;
    apply_patch.response = Some("*** Begin Patch\n*** End Patch");
    assert_eq!(
        gemini_response_kernel(apply_patch).unwrap(),
        br#"{"type":"custom_tool_call","call_id":"call_patch","name":"apply_patch","input":"*** Begin Patch\n*** End Patch"}"#,
    );

    let mut blocked = completed_tool_call_input("call_blocked", "tool_search", r#"{"query":"x"}"#);
    blocked.created_at_present = true;
    blocked.reason_present = true;
    blocked.message = Some("blocked \"quote\"\n");
    assert_eq!(
        gemini_response_kernel(blocked).unwrap(),
        br#"{"type":"message","role":"assistant","content":[{"type":"output_text","text":"blocked \"quote\"\n"}]}"#,
    );
}
