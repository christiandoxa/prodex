#[test]
fn compiled_previous_response_json_search_preserves_error_message_alias_priority() {
    let payload = serde_json::json!({
        "unrelated": {"message":"ordinary content"},
        "nested": [
            {"code":"other", "message":"not an error"},
            {
                "code":"previous_response_not_found",
                "message":null,
                "detail":"detail-selected-雪",
                "error":"must-not-win",
            },
            {
                "code":"previous_response_not_found",
                "message":"must-not-win-second",
            }
        ],
    });
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        Some("detail-selected-雪".to_string()),
    );
    let no_message = serde_json::json!({
        "metadata": {},
        "error": {"code":"previous_response_not_found"}
    });
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&no_message),
        Some("Previous response could not be found on the selected Codex account.".to_string()),
    );
    let wrong_type = serde_json::json!({
        "code":"previous_response_not_found",
        "message":45,
        "detail":false,
        "error":"last string alias",
    });
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&wrong_type),
        Some("last string alias".to_string()),
    );
    let empty_message = serde_json::json!({
        "code":"previous_response_not_found",
        "message":"",
        "detail":"nonempty fallback",
    });
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&empty_message),
        Some(String::new()),
    );
}

#[test]
fn compiled_previous_response_json_search_detects_nested_invalid_id_and_ignores_plain_text() {
    let marker = format!(
        "invalid {}previous_response_id{}.",
        char::from(96),
        char::from(96)
    );
    let correct = serde_json::json!({
        "message":marker,
        "type":"invalid_request_error",
    });
    let wrong_case = serde_json::json!({
        "message":marker,
        "type":"Invalid_request_error",
    });
    assert!(runtime_proxy_value_is_invalid_previous_response_id(
        &correct
    ));
    assert!(!runtime_proxy_value_is_invalid_previous_response_id(
        &wrong_case
    ));
    assert!(runtime_proxy_value_is_invalid_previous_response_id(
        &serde_json::json!([
            {"note":"ignore"},
            {"response": {"error":correct}},
        ])
    ));
    assert!(!runtime_proxy_value_is_invalid_previous_response_id(
        &serde_json::json!({
            "type": "invalid_request_error",
            "message": "ordinary response text",
            "detail": marker,
        })
    ));
}

#[test]
fn compiled_previous_response_json_search_honors_exact_2048_node_budget() {
    // The root array itself consumes one of the 2048 inspected nodes.
    let candidate = serde_json::json!({
        "code":"previous_response_not_found",
        "message":"exact-boundary",
    });
    let mut reachable = vec![serde_json::Value::Null; 2046];
    reachable.push(candidate.clone());
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&serde_json::Value::Array(
            reachable
        ),),
        Some("exact-boundary".to_string()),
    );
    let mut out_of_budget = vec![serde_json::Value::Null; 2047];
    out_of_budget.push(candidate);
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&serde_json::Value::Array(
            out_of_budget
        ),),
        None,
    );
}

#[test]
fn compiled_previous_response_json_search_stops_after_first_preorder_match() {
    let payload = serde_json::json!({
        "a": {"code":"previous_response_not_found","message":"first match"},
        "b": {"code":"previous_response_not_found","message":"later match"},
    });
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        Some("first match".to_string()),
    );
    let payload = serde_json::json!({
        "ignore":{
            "payload":"x".repeat(300_000),
            "response":{"code":"previous_response_not_found","message":"accepted"},
        }
    });
    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        Some("accepted".to_string()),
    );
}

#[test]
fn compiled_previous_response_text_selection_trims_unicode_and_preserves_original_message() {
    for (text, expected) in [
        ("", None),
        (" \t\n\u{3000}", None),
        (
            " \u{2003}previous_response_not_found \u{3000} ",
            Some("previous_response_not_found"),
        ),
        (
            " \tPrevious response with id 'resp-雪' not found.\u{2003}",
            Some("Previous response with id 'resp-雪' not found."),
        ),
        ("output mentioning previous_response_not_found", None),
        ("no ordinary failure here", None),
    ] {
        assert_eq!(
            extract_runtime_proxy_previous_response_message_from_text(text).as_deref(),
            expected,
            "{text:?}",
        );
    }
    assert_eq!(
        extract_runtime_proxy_previous_response_message(b" \tprevious_response_not_found\n",),
        Some("previous_response_not_found".to_string()),
    );
}

