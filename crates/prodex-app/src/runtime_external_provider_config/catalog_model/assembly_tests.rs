use super::ExternalCatalogProvider::{Anthropic, Copilot, Kiro};
use super::external_catalog_models;
use crate::{
    profile_commands::KIRO_MODEL_CATALOG_FILE,
    runtime_external_provider_config::COPILOT_RUNTIME_MODEL_CATALOG_FILE,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn catalog_home(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let home = crate::test_support::test_temp_root().join(format!(
        "prodex-catalog-plan-{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&home).unwrap();
    home
}

#[test]
fn external_catalog_assembly_preserves_current_complete_catalogs() {
    let home = catalog_home("whole");
    let fixture = json!({"models":[
        {"id":" gpt-5.4 ","name":"First Dynamic Name", "description":"First description\n雪", "max_prompt_tokens":333000},
        {"id":"GPT-5.4","name":"Second Dynamic Name", "max_prompt_tokens":444000},
        {"id":"synthetic-only", "name":"Dynamic Only"},
        {"id":"unsigned-limit", "name":"", "description":"", "context_window_tokens":u64::MAX},
        {"id":" β ", "context_window_tokens":1010},
        {"id":"Β", "context_window_tokens":2020}
    ]});
    let bytes = serde_json::to_vec(&fixture).unwrap();
    fs::write(home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE), &bytes).unwrap();
    fs::write(home.join(KIRO_MODEL_CATALOG_FILE), &bytes).unwrap();
    let long_model = "x".repeat(4097);
    let padded_model = format!("{}gpt-6.1-sol", " ".repeat(4097));
    let cases = [
        ("copilot-dynamic", Copilot, "gpt-5.4", 100000, 90000),
        (
            "copilot-untrimmed-query",
            Copilot,
            " GPT-5.4 ",
            100000,
            90000,
        ),
        (
            "copilot-registry-fallback",
            Copilot,
            "gpt-6.1-sol",
            100000,
            90000,
        ),
        ("copilot-empty-launch", Copilot, " \u{3000}", 0, u64::MAX),
        ("copilot-unicode-query", Copilot, " β ", 10, 10),
        (
            "kiro-untrimmed-query",
            Kiro,
            " synthetic-only ",
            11,
            u64::MAX,
        ),
        (
            "kiro-saturated-budget",
            Kiro,
            "unsigned-limit",
            u64::MAX,
            u64::MAX,
        ),
        (
            "anthropic-registry-alias",
            Anthropic,
            " CLAUDE-OPUS-4-8 ",
            100000,
            90000,
        ),
        (
            "anthropic-unknown-zero",
            Anthropic,
            "unseen\n雪",
            0,
            u64::MAX,
        ),
        (
            "anthropic-unknown-max",
            Anthropic,
            "unseen",
            u64::MAX,
            u64::MAX,
        ),
        (
            "copilot-long-query",
            Copilot,
            long_model.as_str(),
            100000,
            90000,
        ),
        (
            "copilot-long-padded-alias",
            Copilot,
            padded_model.as_str(),
            100000,
            90000,
        ),
    ];
    let mut summary = Vec::new();
    for (name, provider, launch, context, compact) in cases {
        let models = external_catalog_models(&home, provider, launch, context, compact).unwrap();
        assert!(
            models
                .iter()
                .all(|entry| !prodex_provider_core::provider_model_is_retired(
                    provider.provider_id(),
                    entry["slug"].as_str().unwrap()
                )),
            "retired model in {name}"
        );
        let digest = Sha256::digest(serde_json::to_vec(&models).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let record = json!({"name":name,"models":models.len(),"sha256":digest, "first_context_window":models[0]["context_window"], "first_auto_compact_token_limit":models[0]["auto_compact_token_limit"]});
        summary.push(record);
    }
    let expected: Value = serde_json::from_str(include_str!("assembly_baseline.json")).unwrap();
    assert_eq!(Value::Array(summary), expected);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn external_catalog_assembly_preserves_merged_limit_and_raw_duplicate_admission() {
    let home = catalog_home("limits");
    let path = home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE);
    let limit = prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT;
    let unique = (0..limit)
        .map(|index| json!({"id":format!("synthetic-{index}")}))
        .collect::<Vec<_>>();
    fs::write(
        &path,
        serde_json::to_vec(&json!({"models":unique})).unwrap(),
    )
    .unwrap();
    let error = external_catalog_models(&home, Copilot, "synthetic-0", 100000, 90000).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("provider model catalog exceeds the hard limit of {limit} entries")
    );
    fs::write(
        &path,
        serde_json::to_vec(&json!({"models":vec![json!({"id":"duplicate"});limit]})).unwrap(),
    )
    .unwrap();
    let models = external_catalog_models(&home, Copilot, "duplicate", 100000, 90000).unwrap();
    assert_eq!(
        models
            .iter()
            .filter(|model| model["slug"] == "duplicate")
            .count(),
        1
    );
    assert!(models.len() < limit);
    fs::remove_dir_all(home).unwrap();
}
