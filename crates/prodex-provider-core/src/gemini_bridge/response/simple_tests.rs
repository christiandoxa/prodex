use super::*;
use serde_json::{Value, json};

fn classify(parts: Value, deny: bool) -> (bool, Vec<(String, Value)>) {
    let value = json!({"candidates":[{"content":{"parts":parts}}]});
    let mut calls = Vec::new();
    let accepted = gemini_provider_core_simple_response(&value, |name, args| {
        calls.push((name.to_string(), args.clone()));
        deny.then(|| "fixture denial".to_string())
    });
    (accepted, calls)
}

#[test]
fn simple_response_keeps_message_only_and_missing_parts_shortcuts() {
    for parts in [
        json!([]),
        json!([{}]),
        json!([{"text":"visible"}]),
        json!([{"inlineData":{},"file_data":null,"executableCode":17,"videoMetadata":false}]),
        json!(null),
    ] {
        assert_eq!(classify(parts, true), (true, vec![]));
    }
    let value = json!({"candidates":[{"content":{"parts":[{"text":"first"}]}},{"content":{"parts":[null]}}]});
    assert!(gemini_provider_core_simple_response(&value, |_, _| panic!(
        "message shortcut invoked callback"
    )));
}

#[test]
fn simple_response_preserves_callback_order_and_rejection_after_callback() {
    let call = json!({"functionCall":{"name":"first","args":null}});
    let second = json!({"functionCall":{"name":"second","args":[1,2]}});
    assert_eq!(
        classify(json!([call.clone(), second.clone()]), false),
        (
            true,
            vec![
                ("first".to_string(), Value::Null),
                ("second".to_string(), json!([1, 2]))
            ]
        )
    );
    assert_eq!(
        classify(json!([call.clone(), second.clone()]), true),
        (false, vec![("first".to_string(), Value::Null)])
    );
    assert_eq!(
        classify(json!([{"text":"visible"}, call.clone()]), false),
        (false, vec![])
    );
    assert_eq!(
        classify(json!([call.clone(), {"text":"visible"}]), false),
        (false, vec![("first".to_string(), Value::Null)])
    );
    assert_eq!(
        classify(
            json!([{"functionCall":{"name":"first","args":null},"extra":true},second]),
            false
        ),
        (false, vec![("first".to_string(), Value::Null)])
    );
}

#[test]
fn simple_response_keeps_unicode_name_checks_and_argument_defaults() {
    for part in [
        json!(17),
        json!({"functionCall":null}),
        json!({"functionCall":{}}),
        json!({"functionCall":{"name":""}}),
        json!({"functionCall":{"name":"\u{2003}\u{a0}"}}),
        json!({"functionCall":{"name":true}}),
    ] {
        assert_eq!(classify(json!([part]), false), (false, vec![]));
    }
    assert_eq!(
        classify(
            json!([{"text":null,"thought":true},{"functionCall":{"name":"\u{2003}tool\u{a0}"},"thoughtSignature":"fixture"}]),
            false
        ),
        (true, vec![("\u{2003}tool\u{a0}".to_string(), json!({}))])
    );
    assert_eq!(
        classify(
            json!([{"text":null,"thought":true,"functionCall":{"name":"tool"}}]),
            false
        ),
        (false, vec![("tool".to_string(), json!({}))])
    );
}

#[test]
fn simple_response_plan_controls_buffered_translation_reuse_and_fallback() {
    use crate::{ProviderEndpoint, ProviderId, ProviderTransformResult, ProviderWireFormat};
    let translated = ProviderTransformResult::lossless(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        br#"{"id":"translated"}"#.to_vec(),
    );
    for extra_key in [false, true] {
        let mut part = json!({"functionCall":{"id":"call_fixture","name":"tool","args":null}});
        if extra_key {
            part["extra"] = json!(true);
        }
        let response = json!({"responseId":"resp_fixture", "candidates":[{
            "content":{"parts":[part]}, "finishReason":"STOP",
        }]});
        let mut calls = Vec::new();
        let mut result =
            super::super::gemini_provider_core_buffered_responses_value_with_fallback_ids(
                &response,
                Some(&translated),
                |name, args| {
                    calls.push((name.to_string(), args.clone()));
                    None
                },
                || "resp_fallback".to_string(),
                |_| "call_fallback".to_string(),
            );
        if extra_key {
            assert_eq!(result["id"], "resp_fixture");
            assert_eq!(calls, vec![("tool".to_string(), Value::Null); 2]);
            assert_eq!(result["output"][0]["name"], "tool");
        } else {
            result.as_object_mut().unwrap().remove("created_at");
            assert_eq!(result, json!({"id":"translated"}));
            assert_eq!(calls, vec![("tool".to_string(), Value::Null)]);
        }
    }
}
