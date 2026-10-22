#[test]
fn local_catalog_preserves_original_complete_file_bytes() {
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};

    let home = temp_codex_home("schema-capture");
    let path = home.join(LOCAL_MODEL_CATALOG_FILE);
    let long_model = "x".repeat(4097);
    let cases = [
        ("ordinary", "synthetic-model", 128_000, 120_000),
        ("raw-unicode", " \tcustom\"\\\n雪 ", 0, u64::MAX),
        ("empty-slug", "", 1, 0),
        ("zero-budget", "zero-model", 0, 0),
        ("max-unsigned", "max-model", u64::MAX, u64::MAX),
        ("compact-above-context", "compact-model", 8, u64::MAX),
        ("long-slug", long_model.as_str(), 1, u64::MAX),
    ];
    let mut summary = Vec::new();
    for (name, model, context, compact) in cases {
        write_local_model_catalog(&path, model, context, compact).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&contents).unwrap();
        let digest = Sha256::digest(contents.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        summary.push(json!({"name":name,"bytes":contents.len(),"sha256":digest,
            "context_window":value["models"][0]["context_window"],
            "auto_compact_token_limit":value["models"][0]["auto_compact_token_limit"]}));
    }
    fs::remove_dir_all(&home).unwrap();
    let expected: Value = serde_json::from_str(include_str!("catalog_baseline.json")).unwrap();
    assert_eq!(Value::Array(summary), expected);
}
