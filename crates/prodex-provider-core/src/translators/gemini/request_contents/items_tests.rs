use super::{gemini_contents_from_request, gemini_request_contents_from_request_mojo};
use serde_json::json;

#[test]
fn tool_history_keeps_custom_unicode_names_and_matches_reversed_responses() {
    let request = json!({
        "input": [
            {
                "role": "assistant",
                "content": "Searching雪🙂",
                "tool_calls": [
                    {"id": "call-α", "function": {"name": "検索🙂", "arguments": "{\"q\":\"雪\"}"}},
                    {"id": "call-β", "function": {"name": "apply_patch", "arguments": "invalid"}}
                ]
            },
            {"role": "tool", "tool_call_id": "call-β", "content": "{\"patched\":true}"},
            {"role": "tool", "tool_call_id": "call-α", "content": "plain 雪🙂"}
        ]
    });

    assert_eq!(
        json!(gemini_contents_from_request(&request).expect("valid Gemini contents")),
        json!([
            {
                "role": "model",
                "parts": [
                    {"text": "Searching雪🙂"},
                    {"functionCall": {"name": "検索🙂", "args": {"q": "雪"}, "id": "call-α"}},
                    {"functionCall": {"name": "apply_patch", "args": {}, "id": "call-β"}}
                ]
            },
            {
                "role": "user",
                "parts": [
                    {"functionResponse": {"name": "apply_patch", "response": {"patched": true}, "id": "call-β"}},
                    {"functionResponse": {"name": "検索🙂", "response": {"output": "plain 雪🙂"}, "id": "call-α"}}
                ]
            }
        ])
    );
}

#[test]
fn user_media_parts_stay_interleaved_with_text() {
    let request = json!({
        "input": [{
            "role": "user",
            "content": [
                {"type": "input_text", "text": "before雪"},
                {"type": "input_image", "image_url": "https://example.com/image.png"},
                {"type": "input_audio", "mime_type": "audio/wav", "data": "c3ludGhldGlj"},
                {"type": "input_text", "text": "after🙂"}
            ]
        }]
    });

    assert_eq!(
        json!(gemini_contents_from_request(&request).expect("valid Gemini contents")),
        json!([{
            "role": "user",
            "parts": [
                {"text": "before雪"},
                {"fileData": {"fileUri": "https://example.com/image.png", "mimeType": "image/png"}},
                {"inlineData": {"mimeType": "audio/wav", "data": "c3ludGhldGlj"}},
                {"text": "after🙂"}
            ]
        }])
    );
}

#[test]
fn malformed_and_wrong_type_inputs_keep_existing_fallbacks() {
    let request = json!({
        "input": [
            null,
            7,
            "standalone",
            {"role": "assistant", "content": {"other": true}, "text": "fallback"},
            {"role": "assistant", "tool_calls": "wrong type"},
            {"role": "assistant", "tool_calls": [
                {"id": 9, "function": false},
                {"id": "call-bad", "function": {"name": [], "arguments": []}}
            ]},
            {"role": "tool", "tool_call_id": 7, "name": 9, "content": [
                "ignored", {"text": null, "content": "blocked by wrong text type"}, false
            ]},
            {"role": "user", "content": [
                {"text": 0}, {"content": "kept"},
                {"type": "input_image", "image_url": 1}, false
            ]}
        ]
    });

    assert_eq!(
        json!(gemini_contents_from_request(&request).expect("valid Gemini contents")),
        json!([
            {"role": "user", "parts": [{"text": "standalone"}]},
            {"role": "model", "parts": [{"text": "fallback"}]},
            {"role": "model", "parts": [
                {"functionCall": {"name": "tool_call", "args": {}}},
                {"functionCall": {"name": "tool_call", "args": {}, "id": "call-bad"}}
            ]},
            {"role": "user", "parts": [
                {"functionResponse": {"name": "tool_call", "response": {"output": ""}}}
            ]},
            {"role": "user", "parts": [{"text": "kept"}]}
        ])
    );

    for input in [json!(null), json!(7), json!({"content": "ignored"})] {
        assert_eq!(
            json!(
                gemini_contents_from_request(&json!({"input": input}))
                    .expect("valid Gemini contents")
            ),
            json!([{"role": "user", "parts": [{"text": ""}]}])
        );
    }
}

#[test]
fn contextual_user_instructions_are_not_emitted_as_contents() {
    let request = json!({
        "input": [
            {"role": "user", "content": "  <environment_context>synthetic</environment_context>"},
            {"role": "user", "content": "actual request"}
        ]
    });

    assert_eq!(
        json!(gemini_contents_from_request(&request).expect("valid Gemini contents")),
        json!([{"role": "user", "parts": [{"text": "actual request"}]}])
    );
}

#[test]
fn system_and_contextual_text_are_mojo_owned_with_malformed_fallbacks() {
    let request = json!({
        "input": [
            null,
            7,
            {"role": "system", "content": false, "text": "fallback"},
            {"role": "user", "content": "  <environment_context>synthetic</environment_context>"},
            {"role": "user", "content": "actual request"}
        ]
    });
    let (system, contents) =
        gemini_request_contents_from_request_mojo(&request).expect("valid Gemini contents");
    assert_eq!(
        system,
        Some(
            json!({"parts": [{"text": "fallback\n\n  <environment_context>synthetic</environment_context>"}]})
        )
    );
    assert_eq!(
        contents,
        vec![json!({"role": "user", "parts": [{"text": "actual request"}]})]
    );
}
