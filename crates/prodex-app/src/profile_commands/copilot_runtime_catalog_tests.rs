use super::copilot_runtime_model_catalog_from_token;
use serde_json::{Value, json};

fn entry(id: &str, name: &str, context: u64) -> Value {
    json!({
        "id":id, "object":"model", "owned_by":"github-copilot", "display_name":name,
        "description":format!("GitHub Copilot model available for this account: {name}."),
        "context_window":context,
        "input_cost_per_million_microusd":0, "output_cost_per_million_microusd":0
    })
}

#[test]
fn copilot_catalog_preserves_ordered_traversal_and_array_boundaries() {
    let input = json!({
        "a":[{"DATA":[{"id":"array-first"}]}],
        "models":[
            {"id":"primary","name":"First"},
            {"id":null,"model":"blocked"},
            {"id":"primary","name":"Duplicate"},
            {"models":[{"id":"hidden-in-model-entry"}]}
        ],
        "nested":{
            "AVAILABLE_MODELS":[{"model":"second"}],
            "chat_models":[false,{"slug":"third"}],
            "model_catalog":[{"name":"fourth"}]
        },
        "ｍodels":[{"id":"not-an-ascii-model-key"}]
    });
    assert_eq!(
        copilot_runtime_model_catalog_from_token(&input).unwrap(),
        vec![
            entry("array-first", "array-first", 200_000),
            entry("primary", "First", 200_000),
            entry("second", "second", 200_000),
            entry("third", "third", 200_000),
            entry("fourth", "fourth", 200_000),
        ]
    );
    for input in [
        json!([{"id":"root-array-entry"}]),
        json!(null),
        json!(7),
        json!("models"),
    ] {
        assert_eq!(
            copilot_runtime_model_catalog_from_token(&input).unwrap(),
            Vec::<Value>::new()
        );
    }
    assert_eq!(
        copilot_runtime_model_catalog_from_token(&json!([
            {"id":"root-array-entry"},{"data":[{"id":"nested-array-model"}]}
        ]))
        .unwrap(),
        vec![entry("nested-array-model", "nested-array-model", 200_000)]
    );
}

#[test]
fn copilot_catalog_preserves_presence_first_names_and_unicode_trim() {
    for input in [
        json!({"id":null,"model":"blocked"}),
        json!({"id":"   ","slug":"blocked"}),
        json!({"id":9,"model":"blocked"}),
        json!({"model":null,"slug":"blocked"}),
    ] {
        assert_eq!(
            copilot_runtime_model_catalog_from_token(&json!({"models":[input]})).unwrap(),
            Vec::<Value>::new()
        );
    }
    for (input, id, name) in [
        (
            json!({"model":" synthetic\u{00a0}","slug":"ignored","name":"\u{2003}Display\u{202f}"}),
            "synthetic",
            "Display",
        ),
        (
            json!({"slug":"chosen","name":null,"display_name":"ignored"}),
            "chosen",
            "chosen",
        ),
        (
            json!({"id":"chosen","display_name":" \tName\n ","label":"ignored"}),
            "chosen",
            "Name",
        ),
        (json!({"name":"Only name"}), "Only name", "Only name"),
        (
            json!({"id":"a\\\"b","name":"line\nnext"}),
            "a\\\"b",
            "line\nnext",
        ),
    ] {
        assert_eq!(
            copilot_runtime_model_catalog_from_token(&json!({"models":[input]})).unwrap(),
            vec![entry(id, name, 200_000)]
        );
    }
}

#[test]
fn copilot_catalog_preserves_unsigned_limits_shadowing_and_prompt_priority() {
    for (fields, context, max_context, max_prompt) in [
        (
            json!({"context_window":null,"context_window_tokens":100}),
            200_000,
            None,
            None,
        ),
        (
            json!({"context_window":1,"context_window_tokens":100}),
            200_000,
            None,
            None,
        ),
        (
            json!({"context_window":100,"max_prompt_tokens":50}),
            50,
            Some(100),
            Some(50),
        ),
        (
            json!({"context_window":100.0,"context_window_tokens":200}),
            200_000,
            None,
            None,
        ),
        (json!({"context_window":-100}), 200_000, None, None),
        (json!({"context_window":"100"}), 200_000, None, None),
        (
            json!({"context_window":u64::MAX}),
            u64::MAX,
            Some(u64::MAX),
            None,
        ),
        (
            json!({"context_window":100,"max_prompt_tokens":1}),
            100,
            Some(100),
            None,
        ),
        (
            json!({"context_window":2,"max_prompt_tokens":2}),
            2,
            Some(2),
            Some(2),
        ),
        (
            json!({"context_window_tokens":77,"max_context_tokens":88,"max_input_tokens":99}),
            77,
            Some(77),
            None,
        ),
        (
            json!({"context_window":0,"capabilities":{"limits":{"max_context_window_tokens":500}}}),
            200_000,
            None,
            None,
        ),
        (
            json!({"context_window":100,"max_prompt_tokens":null,"capabilities":{"limits":{"max_prompt_tokens":10}}}),
            100,
            Some(100),
            None,
        ),
        (
            json!({"capabilities":{"limits":{"max_context_window_tokens":400,"max_prompt_tokens":300}}}),
            300,
            Some(400),
            Some(300),
        ),
    ] {
        let mut input = fields.clone();
        input["id"] = json!("synthetic");
        let mut expected = entry("synthetic", "synthetic", context);
        if let Some(value) = max_context {
            expected["max_context_window"] = json!(value);
        }
        if let Some(value) = max_prompt {
            expected["max_prompt_tokens"] = json!(value);
        }
        if let Some(value) = fields.get("capabilities") {
            expected["capabilities"] = value.clone();
        }
        assert_eq!(
            copilot_runtime_model_catalog_from_token(&json!({"models":[input]})).unwrap(),
            vec![expected],
            "{fields}"
        );
    }
}

#[test]
fn copilot_catalog_preserves_recursive_null_pruning_and_raw_entry_limit() {
    let mut expected = entry("synthetic", "synthetic", 200_000);
    expected["capabilities"] = json!({"array":[null,{"keep":[null,{"ok":true}]}],"kept":false});
    assert_eq!(
        copilot_runtime_model_catalog_from_token(&json!({"models":[{
            "id":"synthetic","capabilities":{"drop":null,"array":[null,{"drop":null,"keep":[null,{"drop":null,"ok":true}]}],"kept":false}
        }]})).unwrap(),vec![expected]
    );
    assert_eq!(
        copilot_runtime_model_catalog_from_token(
            &json!({"models":[{"id":"synthetic","capabilities":null}]})
        )
        .unwrap(),
        vec![entry("synthetic", "synthetic", 200_000)]
    );
    let limit = prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT;
    assert_eq!(
        copilot_runtime_model_catalog_from_token(&json!({"models":vec![Value::Null;limit]}))
            .unwrap(),
        Vec::<Value>::new()
    );
    for models in [
        vec![Value::Null; limit + 1],
        vec![json!({"id":"duplicate"}); limit + 1],
    ] {
        assert_eq!(
            copilot_runtime_model_catalog_from_token(&json!({"models":models}))
                .unwrap_err()
                .to_string(),
            "Copilot model catalog exceeds the hard limit of 1024 entries"
        );
    }
    assert_eq!(
        copilot_runtime_model_catalog_from_token(
            &json!({"models":vec![json!({"id":"duplicate"});limit]})
        )
        .unwrap(),
        vec![entry("duplicate", "duplicate", 200_000)]
    );
}
