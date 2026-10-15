use super::{
    gemini_provider_core_pair_tool_calls, gemini_provider_core_repair_orphan_tool_responses,
};
use serde_json::json;

const MISSING: &str = "The tool execution result was missing from the Codex conversation history. Treat this tool result as unavailable and recover by inspecting current workspace state before continuing.";

fn response(id: &str, name: &str) -> serde_json::Value {
    json!({"functionResponse": {"id": id, "name": name, "response": {"error": MISSING}}})
}
fn call(id: &str, name: &str) -> serde_json::Value {
    json!({"functionCall": {"id": id, "name": name, "args": {}}, "thoughtSignature": "skip_thought_signature_validator"})
}

#[test]
fn mojo_pairing_matches_id_and_name_without_deduplicating_missing_calls() {
    let model = json!({"role": "model", "parts": [
        {"functionCall": {"id": "a", "name": "lookup"}},
        {"functionCall": {"id": "b", "name": "検索"}},
        {"functionCall": {"id": "b", "name": "検索"}},
    ]});
    let result =
        json!({"functionResponse": {"id": "a", "name": "lookup", "response": {"real": true}}});
    let wrong_name =
        json!({"functionResponse": {"id": "b", "name": "other", "response": {"real": true}}});
    let mut contents = vec![
        model.clone(),
        json!({"role": "user", "parts": [result.clone(), wrong_name.clone()], "metadata": 7}),
    ];
    gemini_provider_core_pair_tool_calls(&mut contents);
    assert_eq!(
        contents,
        vec![
            model,
            json!({"role": "user", "parts": [result, wrong_name, response("b", "検索"), response("b", "検索")], "metadata": 7})
        ]
    );
}

#[test]
fn mojo_pairing_inserts_before_a_malformed_user_and_preserves_following_model() {
    let model = json!({"role": "model", "parts": [{"functionCall": null}]});
    let malformed = json!({"role": "user", "parts": "invalid", "keep": true});
    let next =
        json!({"role": "model", "parts": [{"functionCall": {"id": "next", "name": "tool"}}]});
    let mut contents = vec![model.clone(), malformed.clone(), next.clone()];
    gemini_provider_core_pair_tool_calls(&mut contents);
    assert_eq!(
        contents,
        vec![
            model,
            json!({"role": "user", "parts": [response("", "unknown")]}),
            malformed,
            next,
            json!({"role": "user", "parts": [response("next", "tool")]}),
        ]
    );
}

#[test]
fn mojo_orphan_repair_appends_only_missing_id_name_pairs_to_previous_model() {
    let existing_call = json!({"functionCall": {"id": "a", "name": "known", "args": {"keep": 1}}});
    let user = json!({"role": "user", "parts": [
        {"functionResponse": {"id": "a", "name": "known", "response": {"ok": true}}},
        {"functionResponse": {"id": "a", "name": "other", "response": {"ok": false}}},
        {"functionResponse": null},
    ]});
    let mut contents = vec![
        json!({"role": "model", "parts": [existing_call.clone()], "keep": "first"}),
        user.clone(),
    ];
    gemini_provider_core_repair_orphan_tool_responses(&mut contents);
    assert_eq!(
        contents,
        vec![
            json!({"role": "model", "parts": [existing_call, call("a", "other"), call("", "unknown")], "keep": "first"}),
            user,
        ]
    );
}

#[test]
fn mojo_orphan_repair_preserves_the_invalid_previous_parts_edge_case() {
    let user =
        json!({"role": "user", "parts": [{"functionResponse": {"id": "x", "name": "tool"}}]});
    let mut contents = vec![json!({"role": "model", "parts": false}), user.clone()];
    let before = contents.clone();
    gemini_provider_core_repair_orphan_tool_responses(&mut contents);
    assert_eq!(contents, before);
    let mut unpaired = vec![user.clone(), user.clone()];
    gemini_provider_core_repair_orphan_tool_responses(&mut unpaired);
    let synthetic = json!({"role": "model", "parts": [call("x", "tool")]});
    assert_eq!(
        unpaired,
        vec![synthetic.clone(), user.clone(), synthetic, user]
    );
}
