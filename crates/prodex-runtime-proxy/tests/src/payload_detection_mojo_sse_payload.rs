use super::*;

#[test]
fn mojo_sse_payload_join_preserves_json_bom_and_multiline_boundaries() {
    let payload = serde_json::json!({
        "type": "response.created",
        "response": {"id": "resp-🧭"},
    });
    let lines = vec![
        "\u{feff}\u{feff}{".to_owned(),
        format!("\"type\": {},", payload["type"]),
        format!("\"response\": {}", payload["response"]),
        "}".to_owned(),
    ];
    assert_eq!(parse_runtime_sse_payload(&lines), Some(payload.clone()));
    assert_eq!(parse_runtime_sse_event(&lines).response_ids, ["resp-🧭"]);

    let prefixed = vec!["".to_owned(), format!("\u{feff}{}", payload)];
    assert_eq!(parse_runtime_sse_payload(&prefixed), None);
    assert_eq!(parse_runtime_sse_payload(&[]), None);
    let lines = ["\u{feff}".to_owned(), payload.to_string()];
    assert_eq!(parse_runtime_sse_payload(&lines), Some(payload.clone()));

    // A real wire stream dispatches only the completed event; the
    // assembled JSON must survive every split and retain Unicode IDs.
    let response_id = "resp-🧭";
    let body = format!(
        "data: \u{feff}\u{feff}{{\r\ndata: \"type\": \"response.created\",\r\n\
         data: \"response\": {{\"id\":\"{response_id}\"}}\r\ndata: }}\r\n\r\n"
    );
    let expected = collect_runtime_sse_events(&[body.as_bytes()]);
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].response_ids, ["resp-🧭"]);
    for cuts in [vec![0], vec![1; 300], (0..512).collect()] {
        let actual = collect_runtime_sse_events_for_chunk_sizes(body.as_bytes(), &cuts);
        assert_eq!(
            runtime_sse_event_signatures(&actual),
            runtime_sse_event_signatures(&expected)
        );
    }
}
