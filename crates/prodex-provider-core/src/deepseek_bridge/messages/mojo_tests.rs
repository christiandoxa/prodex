use super::*;
use serde_json::json;

#[test]
fn thinking_normalization_preserves_malformed_fields_and_unicode() {
    let mut messages = vec![
        json!(null),
        json!({"role":"assistant", "tool_calls":[{"id":"a"}], "metadata":{"trace":"雪"}}),
        json!({"role":"assistant", "tool_calls":[], "content":null}),
        json!({"role":"user", "tool_calls":[{"id":"b"}], "content":null}),
        json!({"role":"assistant", "tool_calls":[null], "content":"🔥", "reasoning_content":false}),
        json!({"role":"assistant", "tool_calls":"malformed", "content":null}),
    ];

    deepseek_provider_core_normalize_thinking_tool_call_messages(&mut messages);

    assert_eq!(
        json!(messages),
        json!([
            null,
            {"role":"assistant", "tool_calls":[{"id":"a"}], "metadata":{"trace":"雪"}, "content":"", "reasoning_content":""},
            {"role":"assistant", "tool_calls":[], "content":null},
            {"role":"user", "tool_calls":[{"id":"b"}], "content":null},
            {"role":"assistant", "tool_calls":[null], "content":"🔥", "reasoning_content":""},
            {"role":"assistant", "tool_calls":"malformed", "content":null}
        ])
    );
}

#[test]
fn assistant_content_normalization_only_fills_null_or_missing_content() {
    assert_eq!(
        deepseek_provider_core_normalize_assistant_tool_call_content(json!({
            "role":"assistant", "tool_calls":[{"id":"a"}], "content":null,
            "metadata":{"unicode":"🌍"}
        })),
        json!({
            "role":"assistant", "tool_calls":[{"id":"a"}], "content":"",
            "metadata":{"unicode":"🌍"}
        })
    );
    let malformed = json!({"role":"assistant", "tool_calls":"bad", "content":null});
    assert_eq!(
        deepseek_provider_core_normalize_assistant_tool_call_content(malformed.clone()),
        malformed
    );
}

#[test]
fn adjacency_keeps_first_output_and_array_order() {
    let mut messages = vec![
        json!({"role":"tool", "tool_call_id":"a", "content":"first"}),
        json!({"role":"tool", "tool_call_id":"a", "content":"duplicate"}),
        json!({"role":"assistant", "tool_calls":[{"id":"a"}, {"id":"a"}, {"id":"missing"}]}),
        json!({"role":"assistant", "tool_calls":[{"id":"a"}], "content":"keep prose"}),
        json!({"role":"user", "tool_calls":[{"id":"b"}]}),
        json!({"role":"tool", "tool_call_id":"b", "content":"second result"}),
    ];

    deepseek_provider_core_repair_tool_call_adjacency(&mut messages);

    assert_eq!(
        json!(messages),
        json!([
            {"role":"assistant", "tool_calls":[{"id":"a"}], "content":""},
            {"role":"tool", "tool_call_id":"a", "content":"first"},
            {"role":"assistant", "content":"keep prose"},
            {"role":"user", "tool_calls":[{"id":"b"}]},
            {"role":"tool", "tool_call_id":"b", "content":"second result"}
        ])
    );
}

#[test]
fn adjacency_keeps_malformed_content_and_honors_unicode_blankness() {
    let mut messages = vec![
        json!({"role":"tool", "tool_call_id":" ", "content":"unmatched"}),
        json!({"role":"assistant", "tool_calls":[{"id":"a"}], "content":false}),
        json!({"role":"assistant", "tool_calls":[{"id":"b"}], "reasoning_content":[null]}),
        json!({"role":"assistant", "tool_calls":[{"id":"c"}], "content":"\u{001c}"}),
        json!({"role":"assistant", "tool_calls":[{"id":"d"}], "content":"\u{2003}"}),
    ];

    deepseek_provider_core_repair_tool_call_adjacency(&mut messages);

    assert_eq!(
        json!(messages),
        json!([
            {"role":"tool", "tool_call_id":" ", "content":"unmatched"},
            {"role":"assistant", "content":false},
            {"role":"assistant", "reasoning_content":[null]},
            {"role":"assistant", "content":"\u{001c}"}
        ])
    );
}

#[test]
fn metadata_merge_keeps_scalar_when_incoming_value_is_object() {
    let mut response = json!({"metadata":{"scalar":false,"object":{"old":1,"nested":{"a":1}}}});
    let incoming = json!({"scalar":{"new":2},"object":{"new":2,"nested":{"b":2}},"missing":{}});
    deepseek_provider_core_merge_response_metadata(&mut response, Some(incoming));
    assert_eq!(
        response,
        json!({"metadata":{"scalar":false,"object":{"old":1,"new":2,"nested":{"b":2}},"missing":{}}})
    );
}

#[test]
fn adjacency_preserves_order_for_two_thousand_out_of_order_calls() {
    let mut messages = (0..2048)
        .map(|index| json!({"role":"assistant", "tool_calls":[{"id":format!("call_{index:05}")}]}))
        .collect::<Vec<_>>();
    messages.extend((0..2048).rev().map(|index| {
        json!({"role":"tool", "tool_call_id":format!("call_{index:05}"), "content":format!("output {index}")})
    }));

    deepseek_provider_core_repair_tool_call_adjacency(&mut messages);

    assert_eq!(messages.len(), 4096);
    for (index, (call, output)) in messages
        .iter()
        .step_by(2)
        .zip(messages.iter().skip(1).step_by(2))
        .enumerate()
    {
        assert_eq!(call["tool_calls"][0]["id"], format!("call_{index:05}"));
        assert_eq!(output["tool_call_id"], format!("call_{index:05}"));
        assert_eq!(output["content"], format!("output {index}"));
    }
}
