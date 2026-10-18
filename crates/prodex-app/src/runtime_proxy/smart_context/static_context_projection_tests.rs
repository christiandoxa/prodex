use super::*;

#[test]
fn compiled_static_prompt_projection_preserves_nested_priority_and_unicode_exactly() {
    let value = serde_json::json!({
        "developer": " dev\0\"🧭 ", " developer": "ignored sibling",
        "instructions": " \u{2003} ", "system": 3,
        "input": [
            {"role":"user", "content":"ignored"},
            {"role":"system", "content":" ", "input_text":"ignored fallback"},
            {"role":"developer", "content":[{"text":"first", "input_text":"second",
                "content":["third", {"text":"fourth", "ignored":"hidden"}]}]},
            {"role":"system", "content":["ignored content"], "input_text":""},
            {"role":"system", "input_text":"exact 🧭\n"},
            {"role":"developer", "content":null},
            {"role":1, "content":"ignored role"},
        ]
    });
    let (items, legacy) = runtime_smart_context_static_request_projection(&value);
    assert_eq!(
        items
            .into_iter()
            .map(|item| (item.id, item.text))
            .collect::<Vec<_>>(),
        vec![
            ("developer".to_string(), " dev\0\"🧭 ".to_string()),
            (
                "input[2].developer".to_string(),
                "first\nsecond\nthird\nfourth".to_string()
            ),
            ("input[4].system".to_string(), "exact 🧭\n".to_string()),
        ]
    );
    assert_eq!(legacy, None);
}

#[test]
fn compiled_static_prompt_projection_keeps_lexical_input_ids_and_legacy_consensus() {
    let value = serde_json::json!({"input": (0..12).map(|index| {
        serde_json::json!({"role":"system", "content":format!("rules {index}")})
    }).collect::<Vec<_>>()});
    let ids = runtime_smart_context_static_context_items(&value)
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [0, 10, 11, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            .into_iter()
            .map(|index| format!("input[{index}].system"))
            .collect::<Vec<_>>()
    );
    for (value, expected) in [
        (
            serde_json::json!({"instructions":"psc static scpc:legacy"}),
            Some("scpc:legacy"),
        ),
        (
            serde_json::json!({"instructions":"\u{2003}psc static scpc2:digest\u{3000}",
            "input":[{"role":"developer", "content":[{"text":"prodex static context unchanged scpc2:digest"}]}]}),
            Some("scpc2:digest"),
        ),
        (
            serde_json::json!({"instructions":"psc static scpc:a", "system":"psc static scpc:b"}),
            None,
        ),
        (
            serde_json::json!({"instructions":"psc static scpc:a\u{2003}b"}),
            None,
        ),
        (
            serde_json::json!({"input":[{"role":"system", "content":["psc static scpc:a", "psc static scpc:a"]}]}),
            None,
        ),
        (serde_json::json!({}), None),
    ] {
        let (_, legacy) = runtime_smart_context_static_request_projection(&value);
        assert_eq!(legacy.as_deref(), expected, "{value}");
        if let Some(expected) = expected {
            assert_eq!(
                runtime_smart_context_static_prompt_cache_key_from_body(
                    &serde_json::to_vec(&value).unwrap()
                )
                .as_deref(),
                Some(expected)
            );
        }
    }
}
