use super::main_model_choices_from_catalog;
use prodex_provider_core::ProviderModelChoice;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

#[test]
fn main_catalog_projection_preserves_original_complete_choices() {
    let mut cases: Vec<Value> =
        serde_json::from_str(include_str!("projection_inputs.json")).unwrap();
    let long = "x".repeat(4097);
    cases.extend([
        json!({"name":"long-model","entries":[{"id":long}]}),
        json!({"name":"long-label","entries":[{"id":"label-model","display_name":long}]}),
        json!({"name":"long-alias","entries":[{"id":"alias-model","aliases":[long]}]}),
        json!({"name":"many-raw","entries":vec![json!({});513]}),
    ]);
    let mut summary = Vec::new();
    for case in cases {
        let entries = case["entries"].as_array().unwrap().clone();
        let choices = main_model_choices_from_catalog(entries).map(|choices| {
            choices
                .into_iter()
                .map(|choice| {
                    let kind = match choice.choice {
                        ProviderModelChoice::ProviderDefault => json!({"kind":"provider_default"}),
                        ProviderModelChoice::Model(model) => json!({"kind":"model","model":model}),
                        ProviderModelChoice::Custom => json!({"kind":"custom"}),
                    };
                    json!({"choice":kind,"label":choice.label,"efforts":choice.efforts,
                    "aliases":choice.aliases,"default_effort":choice.default_effort})
                })
                .collect::<Vec<_>>()
        });
        let bytes = serde_json::to_vec(&choices).unwrap();
        let digest = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        summary.push(json!({"name":case["name"],"bytes":bytes.len(),"sha256":digest}));
    }
    let expected: Value = serde_json::from_str(include_str!("projection_baseline.json")).unwrap();
    assert_eq!(Value::Array(summary), expected);
}
