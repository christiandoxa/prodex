use super::transcript_events_from_session_line;
use serde_json::{Value, json};

fn show(kind: &str, mut payload: Value) -> Vec<(String, String)> {
    payload
        .as_object_mut()
        .unwrap()
        .insert("type".into(), json!(kind));
    transcript_events_from_session_line(
        &json!({"type":"response_item","payload":payload}).to_string(),
    )
    .into_iter()
    .map(|event| (event.source, event.text))
    .collect()
}

#[test]
fn tool_calls_keep_field_types_defaults_and_full_raw_arguments() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    for (kind, field, fallback) in [
        ("function_call", "arguments", "tool"),
        ("custom_tool_call", "input", "custom-tool"),
    ] {
        let mut payload = json!({"name":null});
        payload
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("a".repeat(300)));
        assert_eq!(
            show(kind, payload.clone()),
            vec![(format!("tool-call:{fallback}"), "a".repeat(300))]
        );
        payload["name"] = json!("");
        assert_eq!(
            show(kind, payload.clone()),
            vec![("tool-call:tool".into(), "a".repeat(300))]
        );
        payload["name"] = json!("with space");
        payload[field] = json!("first\nsecond\0end");
        assert_eq!(
            show(kind, payload),
            vec![("tool-call:with_space".into(), "first\nsecond end".into())]
        );
    }
    assert_eq!(
        show("function_call", json!({"arguments":null,"input":"ignored"})),
        vec![]
    );
    assert_eq!(
        show(
            "custom_tool_call",
            json!({"input":false,"arguments":"ignored"})
        ),
        vec![]
    );
    assert_eq!(
        show("function_call", json!({"arguments":"Bearer FAKE"})),
        vec![("tool-call:tool".into(), "Bearer <redacted>".into())]
    );
}

#[test]
fn tool_outputs_keep_string_only_shape_full_text_and_ansi_projection() {
    for kind in ["function_call_output", "custom_tool_call_output"] {
        assert_eq!(
            show(kind, json!({"output":"a".repeat(300)})),
            vec![("tool-output".into(), "a".repeat(300))]
        );
        assert_eq!(
            show(kind, json!({"output":"\u{1b}[31mfirst\u{1b}[0m\nsecond"})),
            vec![("tool-output".into(), "first\nsecond".into())]
        );
        for value in [
            json!(null),
            json!(false),
            json!(42),
            json!([]),
            json!({"text":"ignored"}),
        ] {
            assert_eq!(
                show(kind, json!({"output":value,"stdout":"ignored"})),
                vec![]
            );
        }
        assert_eq!(
            show(kind, json!({"output":"Bearer FAKE"})),
            vec![("tool-output".into(), "Bearer <redacted>".into())]
        );
        assert_eq!(show(kind, json!({"output":"é".repeat(300)})), vec![]);
    }
}

#[test]
fn tool_projection_retains_final_safe_text_cap_without_scalar_truncation() {
    for (kind, field) in [
        ("function_call", "arguments"),
        ("custom_tool_call", "input"),
        ("function_call_output", "output"),
    ] {
        let mut payload = json!({});
        payload
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("readable line\n".repeat(7000)));
        let events = show(kind, payload);
        assert_eq!(events.len(), 1);
        assert!(events[0].1.len() > 60_000 && events[0].1.len() <= 65_536);
        assert!(events[0].1.ends_with(" …[truncated]"));
    }
}
