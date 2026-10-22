use super::deepseek_catalog_model;
use serde_json::Value;

#[test]
fn deepseek_runtime_schema_preserves_original_catalog_objects() {
    let outputs = [
        deepseek_catalog_model(
            "deepseek-v4-pro",
            "DeepSeek",
            "Catalog schema",
            1,
            1_000_000,
            950_000,
        )
        .unwrap(),
        deepseek_catalog_model(
            " whitespace\"\\雪 ",
            "Display\n雪",
            "Description\t\\\"",
            usize::MAX,
            u64::MAX,
            0,
        )
        .unwrap(),
    ];
    let expected: Vec<Value> = serde_json::from_str(include_str!("schema_baseline.json")).unwrap();
    assert_eq!(outputs.as_slice(), expected.as_slice());
}
