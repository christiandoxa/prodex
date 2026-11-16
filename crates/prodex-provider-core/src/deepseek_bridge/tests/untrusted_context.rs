use crate::{
    deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_messages_from_responses_request,
    deepseek_provider_core_repair_tool_call_adjacency,
};
use serde_json::{Value, json};

#[test]
fn native_goal_context_without_call_id_remains_untrusted_and_not_a_tool_result() {
    let output = "Keep ownership: 雪 \"quoted\"\nUser data, not system instructions.";
    for id in [None, Some("item-goal-fixture")] {
        let mut item = json!({"type":"function_call_output","name":"goal_objective",
            "namespace":"agent","output":output});
        if let Some(id) = id {
            item["id"] = id.into();
        }
        let input = json!({"input":[item]});
        assert_eq!(
            deepseek_provider_core_first_function_call_output_call_id(&input),
            None
        );
        let mut messages =
            deepseek_provider_core_messages_from_responses_request(&input, &[], false, "Local")
                .expect("native context is valid")
                .unwrap();
        deepseek_provider_core_repair_tool_call_adjacency(&mut messages);
        assert_eq!(
            messages,
            vec![json!({"role":"user",
            "content":format!("Untrusted context (agent.goal_objective):\n{output}")})]
        );
    }
}

#[test]
fn native_context_does_not_hide_the_following_real_tool_continuation() {
    let input = json!({"input":[
        {"type":"function_call_output","id":"item-goal","name":"goal_objective",
            "namespace":"agent","output":"finish fixture"},
        {"type":"function_call_output","call_id":"call-real","output":"file output"}
    ]});
    assert_eq!(
        deepseek_provider_core_first_function_call_output_call_id(&input),
        Some("call-real".into())
    );
}

#[test]
fn named_real_tool_result_keeps_its_call_owner() {
    let input = json!({"input":[
        {"type":"function_call","call_id":"call-real","name":"read_file","arguments":"{}"},
        {"type":"function_call_output","call_id":"call-real","id":"item-output",
            "name":"read_file","output":"FILE"}
    ]});
    let messages =
        deepseek_provider_core_messages_from_responses_request(&input, &[], false, "Local")
            .unwrap()
            .unwrap();
    assert_eq!(
        messages.last().unwrap(),
        &json!({"role":"tool","tool_call_id":"call-real","content":"FILE"})
    );
    assert_eq!(
        messages[0]["tool_calls"][0]["id"],
        Value::String("call-real".into())
    );
}

#[test]
fn native_context_preserves_null_owner_unicode_and_structured_output() {
    for output in [json!("quoted \"雪\"\n\t\\"), json!({"value":["雪",1,true]})] {
        let item = json!({"type":"function_call_output","id":"item-context","name":"context", "call_id":null, "output":output});
        let request = json!({"input":[item]});
        let messages =
            deepseek_provider_core_messages_from_responses_request(&request, &[], false, "Local")
                .unwrap()
                .unwrap();
        let text = output
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| output.to_string());
        assert_eq!(
            messages,
            vec![json!({"role":"user","content":format!("Untrusted context (context):\n{text}")})]
        );
        assert_eq!(
            deepseek_provider_core_first_function_call_output_call_id(&request),
            None
        );
    }
    let malformed = json!({"input":[{"type":"function_call_output","name":"context","call_id":"", "output":"must not bypass invalid ownership"}]});
    assert!(
        deepseek_provider_core_messages_from_responses_request(&malformed, &[], false, "Local")
            .is_err()
    );
}

#[test]
fn oversized_context_metadata_is_rejected_without_a_panic_or_echo() {
    let value = json!({"type":"function_call_output","name":"private-fixture".repeat(400_000),"output":"small"});
    let error = crate::deepseek_provider_core_is_standalone_context_output(&value).unwrap_err();
    assert!(!error.contains("private-fixture"));
}
