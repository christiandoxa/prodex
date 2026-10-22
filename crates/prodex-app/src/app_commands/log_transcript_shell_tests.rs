use super::transcript_events_from_session_line;
use serde_json::{Value, json};

fn shell(kind: &str, mut payload: Value) -> Vec<(String, String)> {
    payload
        .as_object_mut()
        .unwrap()
        .insert("type".into(), json!(kind));
    transcript_events_from_session_line(
        &json!({"type":"response_item", "payload":payload}).to_string(),
    )
    .into_iter()
    .map(|event| (event.source, event.text))
    .collect()
}

#[test]
fn shell_projection_keeps_presence_aliases_and_typed_scalar_values() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }
    for kind in ["local_shell_call", "shell_call"] {
        assert_eq!(
            shell(
                kind,
                json!({"command":null, "arguments":"ignored", "action":"ignored"})
            ),
            vec![]
        );
        assert_eq!(
            shell(
                kind,
                json!({"command":false, "name":null, "arguments":"ignored"})
            ),
            vec![("tool-call:shell".into(), "false".into())]
        );
        assert_eq!(
            shell(
                kind,
                json!({"arguments":["echo","東京"], "name":"with space"})
            ),
            vec![("tool-call:with_space".into(), "[\"echo\",\"東京\"]".into())]
        );
        assert_eq!(
            shell(kind, json!({"action":{"z":true,"a":42}, "name":""})),
            vec![("tool-call:tool".into(), "{\"a\":42,\"z\":true}".into())]
        );
        assert_eq!(
            shell(kind, json!({"command":1e30})),
            vec![("tool-call:shell".into(), "1e+30".into())]
        );
    }
    for kind in ["local_shell_call_output", "shell_call_output"] {
        assert_eq!(
            shell(
                kind,
                json!({"output":null,"aggregated_output":"ignored","stdout":"ignored"})
            ),
            vec![]
        );
        assert_eq!(
            shell(kind, json!({"aggregated_output":false, "stdout":"ignored"})),
            vec![("tool-output".into(), "false".into())]
        );
        assert_eq!(
            shell(kind, json!({"stdout":["one",2]})),
            vec![("tool-output".into(), "[\"one\",2]".into())]
        );
    }
}

#[test]
fn shell_projection_keeps_control_rejection_scalar_bound_and_final_redaction() {
    for text in ["line\nnext", "\u{1b}[31mline\u{1b}[0m", "line\0tail"] {
        assert_eq!(shell("shell_call", json!({"command":text})), vec![]);
        assert_eq!(shell("shell_call_output", json!({"output":text})), vec![]);
    }
    let long = "é".repeat(250);
    let expected = format!("{}…", "é".repeat(192));
    assert_eq!(
        shell("shell_call", json!({"command":long})),
        vec![("tool-call:shell".into(), expected)]
    );
    assert_eq!(shell("shell_call_output", json!({"output":long})), vec![]);
    assert_eq!(
        shell("shell_call_output", json!({"output":"a".repeat(250)})),
        vec![("tool-output".into(), format!("{}…", "a".repeat(192)))]
    );
    assert_eq!(
        shell("shell_call", json!({"command":"Bearer FAKE"})),
        vec![("tool-call:shell".into(), "Bearer <redacted>".into())]
    );
    assert_eq!(
        shell(
            "shell_call_output",
            json!({"output":{"authorization":"Bearer FAKE","text":"a\nb"}})
        ),
        vec![(
            "tool-output".into(),
            "{\"authorization\":\"<redacted>\",\"text\":\"a\\nb\"}".into()
        )]
    );
}

#[test]
fn shell_projection_keeps_large_container_text_and_serde_wire_spelling() {
    let text = "é".repeat(40_000);
    let events = shell("shell_call", json!({"command":{"text":text}}));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "tool-call:shell");
    assert!(events[0].1.len() <= 65_536);
    assert!(events[0].1.starts_with("{\"text\":\""));
    assert!(events[0].1.ends_with(" …[truncated]"));
    assert!(events[0].1.len() > 60_000);
    let line = r#"{"type":"response_item","payload":{"type":"shell_call","command":null,"command":["last"]}}"#;
    let events = transcript_events_from_session_line(line);
    assert_eq!(events[0].text, "[\"last\"]");
}
