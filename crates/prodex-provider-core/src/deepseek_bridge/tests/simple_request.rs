use super::deepseek_provider_core_simple_request;

fn simple(body: &[u8]) -> bool {
    deepseek_provider_core_simple_request(body, |_| false)
}

#[test]
fn accepts_supported_input_item_shapes() {
    for body in [
        r#"{"input":"hello 世界"}"#,
        r#"{"input":[]}"#,
        r#"{"input":[{"role":"user","content":"hello"}]}"#,
        r#"{"input":[{"type":"message","role":"assistant","content":[{"type":"input_text","text":"hi"},{"type":"output_text","text":"there"},{"type":"text","text":"!"}]}]}"#,
        r#"{"input":[{"type":"function_call","name":"lookup","call_id":"call_1"}]}"#,
        r#"{"input":[{"type":"function_call_output","call_id":"call_1","output":"done"}]}"#,
        r#"{"input":[{"type":"mcp_call","tool_name":"lookup","tool_call_id":"call_1"}]}"#,
        r#"{"input":[{"type":"mcp_tool_result","id":"call_1","result":"done"}]}"#,
        r#"{"input":[{"type":"mcp_call_output","tool_call_id":"call_1","error":"failed"}]}"#,
        r#"{"input":[{"type":"custom_tool_call","name":"lookup","id":"call_1"}]}"#,
        r#"{"input":[{"type":"custom_tool_call_output","call_id":"call_1","content":"done"}]}"#,
        r#"{"input":[{"type":"local_shell_call","command":"pwd"}]}"#,
        r#"{"input":[{"type":"local_shell_call","action":{"command":["git","status"]}}]}"#,
    ] {
        assert!(simple(body.as_bytes()), "{body}");
    }
}

#[test]
fn accepts_supported_tools_choices_and_response_formats() {
    for body in [
        r#"{"input":"x","tools":[]}"#,
        r#"{"input":"x","tools":[{"type":"function","function":{"name":"lookup"}}]}"#,
        r#"{"input":"x","tools":[{"type":"function","function":{"name":"工具"}}]}"#,
        r#"{"input":"x","tool_choice":null}"#,
        r#"{"input":"x","tool_choice":"auto"}"#,
        r#"{"input":"x","tool_choice":"none"}"#,
        r#"{"input":"x","tool_choice":"required"}"#,
        r#"{"input":"x","tool_choice":{"type":"function","name":"lookup"}}"#,
        r#"{"input":"x","response_format":{"type":"text"}}"#,
        r#"{"input":"x","response_format":{"type":"json_object"}}"#,
        r#"{"input":"x","response_format":{"type":"json_schema"}}"#,
        r#"{"input":"x","response_format":{"type":"json"}}"#,
        r#"{"input":"x","response_format":{"type":"structured_output"}}"#,
    ] {
        assert!(simple(body.as_bytes()), "{body}");
    }

    for body in [
        r#"{"input":"x","tools":[{"type":"web_search_preview"}]}"#,
        r#"{"input":"x","tools":[{"type":"function","function":{"name":"　"}}]}"#,
        r#"{"input":"x","tool_choice":"unsupported"}"#,
        r#"{"input":"x","tool_choice":{"type":"function"}}"#,
        r#"{"input":"x","response_format":null}"#,
        r#"{"input":"x","response_format":{"type":"unsupported"}}"#,
        r#"{"input":"x","response_format":{"type":4}}"#,
        r#"{"input":"x","web_search_options":null}"#,
        r#"{"input":"x","safety_identifier":"user_1"}"#,
    ] {
        assert!(!simple(body.as_bytes()), "{body}");
    }
}

#[test]
fn rejects_invalid_items_and_preserves_field_precedence() {
    for body in [
        "null",
        "[]",
        r#"{"input":4}"#,
        r#"{"input":[4]}"#,
        r#"{"input":[{"type":"unknown"}]}"#,
        r#"{"input":[{"type":"message","role":"developer"}]}"#,
        r#"{"input":[{"type":"function_call","name":4,"tool_name":"lookup","call_id":"call_1"}]}"#,
        r#"{"input":[{"type":"message","content":[{"text":null,"input_text":"hello"}]}]}"#,
        r#"{"input":[{"type":"function_call","name":"lookup","call_id":null,"id":"call_1"}]}"#,
        r#"{"input":"x","tool_choice":false}"#,
    ] {
        assert!(!simple(body.as_bytes()), "{body}");
    }
}

#[test]
fn accepts_escaped_json_names_and_enum_values_after_parsing() {
    for (body, expected) in [
        (
            r#"{"in\u0070ut":[{"t\u0079pe":"m\u0065ssage","c\u006fntent":"你好"}]}"#,
            true,
        ),
        (
            r#"{"input":"hello","safety_ident\u0069fier":"user_1"}"#,
            false,
        ),
        (
            r#"{"input":[{"type":"m\u0063p_call","name":"lookup","call_id":"call_1"}]}"#,
            true,
        ),
    ] {
        assert_eq!(simple(body.as_bytes()), expected, "{body}");
    }
}

#[test]
fn malformed_json_and_invalid_utf8_fail_closed() {
    for body in [
        b"{".as_slice(),
        b"{\"input\":\"x\",}".as_slice(),
        &[b'{', 0xff, b'}'],
    ] {
        assert!(!simple(body));
    }
}

#[test]
fn previous_response_lookup_uses_decoded_nonblank_ids_once() {
    let body = br#"{"input":"hello","previous_response_id":"resp\u005f1"}"#;
    let mut seen = Vec::new();
    assert!(deepseek_provider_core_simple_request(body, |id| {
        seen.push(id.to_owned());
        false
    }));
    assert_eq!(seen, ["resp_1"]);

    let mut seen = Vec::new();
    assert!(!deepseek_provider_core_simple_request(body, |id| {
        seen.push(id.to_owned());
        id == "resp_1"
    }));
    assert_eq!(seen, ["resp_1"]);

    let blank_id = br#"{"input":"hello","previous_response_id":"\u2003"}"#;
    let mut lookups = 0;
    assert!(deepseek_provider_core_simple_request(blank_id, |_| {
        lookups += 1;
        false
    }));
    assert_eq!(lookups, 0);

    assert!(!deepseek_provider_core_simple_request(b"{", |_| {
        lookups += 1;
        false
    }));
    assert_eq!(lookups, 0);
}

#[test]
fn eligible_inputs_above_four_mib_reach_mojo_policy() {
    let body = format!(r#"{{"input":"{}"}}"#, "x".repeat(4 * 1024 * 1024));
    assert!(body.len() > 4 * 1024 * 1024);
    assert!(simple(body.as_bytes()));
}

#[test]
fn simple_request_policy_keeps_sixteen_mib_input_bound() {
    const MAX_BYTES: usize = 16 * 1024 * 1024;
    let overhead = r#"{"input":""}"#.len();
    let mut body = format!(r#"{{"input":"{}"}}"#, "x".repeat(MAX_BYTES - overhead));
    assert_eq!(body.len(), MAX_BYTES);
    assert!(simple(body.as_bytes()));

    body.insert(body.len() - 2, 'x');
    assert!(!simple(body.as_bytes()));
}
