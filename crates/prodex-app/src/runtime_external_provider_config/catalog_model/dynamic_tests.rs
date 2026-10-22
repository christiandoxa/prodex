use super::{ExternalCatalogProvider, external_dynamic_catalog_models};
use crate::runtime_external_provider_config::COPILOT_RUNTIME_MODEL_CATALOG_FILE;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn catalog_home() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let home =
        crate::test_support::test_temp_root().join(format!("prodex-dynamic-catalog-{stamp}"));
    fs::create_dir_all(&home).unwrap();
    home
}

#[test]
fn dynamic_catalog_projection_preserves_original_field_precedence() {
    let home = catalog_home();
    let input = json!({"models":[
        {"id":" \u{3000}Model雪 \n", "slug":"ignored", "name":"  Name\n", "description":"Details\t", "max_prompt_tokens":200},
        {"id":null,"slug":"ignored"},
        {"id":" \u{a0}","model":"ignored"},
        {"id":4,"model":"ignored"},
        {"slug":"before-model","model":"ignored","name":null,"model_name":"ignored","context_window_tokens":1,"context_window":999},
        {"model":"empty-name","name":"","model_name":"ignored","description":false,"max_prompt_tokens":null,"capabilities":{"limits":{"max_prompt_tokens":444}},"context_window_tokens":0,"context_window":999},
        {"id":"fallback","model_name":"Raw Name ","description":"","max_prompt_tokens":1,"capabilities":{"limits":{"max_prompt_tokens":444}},"context_window_tokens":500},
        {"id":"unsigned-max","max_prompt_tokens":1.5,"context_window_tokens":1.0,"context_window":u64::MAX},
        {"id":"nested-prompt","capabilities":{"limits":{"max_prompt_tokens":u64::MAX}},"context_window_tokens":2},
        {"id":"negative-context","name":false,"model_name":"ignored","context_window_tokens":-1,"context_window":123},
        {"id":"prompt-first","max_prompt_tokens":2,"context_window_tokens":u64::MAX},
        {"id":"duplicate"},
        {"id":"DUPLICATE"},
        false
    ]});
    fs::write(
        home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE),
        serde_json::to_vec(&input).unwrap(),
    )
    .unwrap();
    let models = external_dynamic_catalog_models(&home, ExternalCatalogProvider::Copilot).unwrap();
    let observed = Value::Array(
        models
            .iter()
            .map(|model| {
                json!({
                    "slug":model.slug,"display_name":model.display_name,
                    "description":model.description,"context_window":model.context_window
                })
            })
            .collect(),
    );
    assert_eq!(
        observed,
        json!([
            {"slug":"Model雪","display_name":"  Name\n","description":"Details\t","context_window":200},
            {"slug":"before-model","display_name":null,"description":null,"context_window":null},
            {"slug":"empty-name","display_name":"","description":null,"context_window":null},
            {"slug":"fallback","display_name":"Raw Name ","description":"","context_window":500},
            {"slug":"unsigned-max","display_name":null,"description":null,"context_window":u64::MAX},
            {"slug":"nested-prompt","display_name":null,"description":null,"context_window":u64::MAX},
            {"slug":"negative-context","display_name":null,"description":null,"context_window":123},
            {"slug":"prompt-first","display_name":null,"description":null,"context_window":2},
            {"slug":"duplicate","display_name":null,"description":null,"context_window":null},
            {"slug":"DUPLICATE","display_name":null,"description":null,"context_window":null}
        ])
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn dynamic_catalog_projection_preserves_raw_limits_and_missing_array_errors() {
    let home = catalog_home();
    let path = home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE);
    assert!(
        external_dynamic_catalog_models(&home, ExternalCatalogProvider::Copilot)
            .unwrap()
            .is_empty()
    );
    let limit = prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT;
    for (count, expected) in [
        (limit, None),
        (
            limit + 1,
            Some(format!(
                "provider model catalog exceeds the hard limit of {limit} entries"
            )),
        ),
    ] {
        fs::write(
            &path,
            serde_json::to_vec(&json!({"models":vec![Value::Null;count]})).unwrap(),
        )
        .unwrap();
        let result = external_dynamic_catalog_models(&home, ExternalCatalogProvider::Copilot);
        match expected {
            Some(message) => assert_eq!(result.unwrap_err().to_string(), message),
            None => assert!(result.unwrap().is_empty()),
        }
    }
    for value in [
        json!({}),
        json!({"models":null}),
        json!({"models":{}}),
        json!([]),
    ] {
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error =
            external_dynamic_catalog_models(&home, ExternalCatalogProvider::Copilot).unwrap_err();
        assert_eq!(
            error.to_string(),
            "provider model catalog is missing models array"
        );
    }
    fs::write(&path, "invalid JSON").unwrap();
    assert!(
        external_dynamic_catalog_models(&home, ExternalCatalogProvider::Anthropic)
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(home).unwrap();
}
