use super::deepseek_catalog_models;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

#[test]
fn deepseek_catalog_assembly_preserves_original_complete_catalogs() {
    let long_model = "x".repeat(4097);
    let padded_model = format!("{}deepseek-v4-flash", " ".repeat(4097));
    let cases = [
        ("known-model", "deepseek-v4-pro", 100_000, 90_000),
        ("case-folded-alias", "PRO", 1, u64::MAX),
        (
            "padded-model",
            " \u{3000}DEEPSEEK-V4-PRO ",
            u64::MAX,
            u64::MAX,
        ),
        ("auto-alias", "auto", 1, 0),
        ("unknown-unicode", "custom\"\\\n雪", 0, u64::MAX),
        ("empty-launch", " \u{3000}", 0, 0),
        ("long-query", long_model.as_str(), 1, u64::MAX),
        ("long-padded-model", padded_model.as_str(), 1, u64::MAX),
    ];
    let mut summary = Vec::new();
    for (name, launch, context, compact) in cases {
        let models = deepseek_catalog_models(launch, context, compact).unwrap();
        let digest = Sha256::digest(serde_json::to_vec(&models).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        summary.push(json!({"name":name,"models":models.len(),"sha256":digest,
            "first_context_window":models[0]["context_window"],
            "first_auto_compact_token_limit":models[0]["auto_compact_token_limit"]}));
    }
    let expected: Value = serde_json::from_str(include_str!("assembly_baseline.json")).unwrap();
    assert_eq!(Value::Array(summary), expected);
}
