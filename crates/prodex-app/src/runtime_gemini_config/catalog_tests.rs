use super::{
    GEMINI_DEFAULT_AUTO_COMPACT_LIMIT, GEMINI_DEFAULT_CONTEXT_WINDOW, GEMINI_DEFAULT_MODEL,
    RuntimeGeminiModelResolution, gemini_catalog_models,
};
use prodex_provider_core::{ProviderId, provider_model_catalog};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

#[test]
fn gemini_catalog_preserves_original_complete_catalogs() {
    let known = provider_model_catalog(ProviderId::Gemini)[0].id;
    let upper = format!(" \u{3000}{} ", known.to_uppercase());
    let long_model = "x".repeat(4097);
    let padded = format!("{}{known}", " ".repeat(4097));
    let cases = [
        (
            "default",
            GEMINI_DEFAULT_MODEL,
            GEMINI_DEFAULT_CONTEXT_WINDOW as u64,
            GEMINI_DEFAULT_AUTO_COMPACT_LIMIT as u64,
        ),
        ("known", known, 200_000, 190_000),
        ("case-folded-padded", upper.as_str(), u64::MAX, u64::MAX),
        ("unknown-unicode", "custom\"\\\n雪", 0, u64::MAX),
        ("empty", "", 0, 0),
        ("unicode-blank", " \u{0085}\u{00a0}\u{3000}", 1, 0),
        ("compact-above-context", "synthetic-model", 8, u64::MAX),
        ("max-unsigned", "synthetic-model", u64::MAX, u64::MAX),
        ("long-query", long_model.as_str(), 1, u64::MAX),
        ("long-padded", padded.as_str(), 1, u64::MAX),
    ];
    let mut summary = Vec::new();
    for (name, launch, context, compact) in cases {
        let models =
            gemini_catalog_models(launch, &RuntimeGeminiModelResolution, context, compact).unwrap();
        let digest = Sha256::digest(serde_json::to_vec(&models).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        summary.push(json!({"name":name,"models":models.len(),"sha256":digest,
            "first_context_window":models[0]["context_window"],
            "first_auto_compact_token_limit":models[0]["auto_compact_token_limit"]}));
    }
    let expected: Value = serde_json::from_str(include_str!("catalog_baseline.json")).unwrap();
    assert_eq!(Value::Array(summary), expected);
}
