use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "mojo-core")]
#[test]
fn external_catalog_model_merge_matches_rust_for_unicode_and_long_ids() {
    let long_upper = format!("{}-MODEL", "A".repeat(65_537));
    let long_lower = long_upper.to_ascii_lowercase();
    let ids = [
        "\u{3000}\t",
        " alpha ",
        "ALPHA",
        "\u{1c}",
        "\u{1d}",
        "\u{1e}",
        "\u{1f}",
        long_upper.as_str(),
        long_lower.as_str(),
        " beta ",
        "BETA",
        "β",
        "Β",
    ];

    let mojo_indices = prodex_mojo_core::rich::merge_catalog_ids(&[], &ids).unwrap();
    assert_eq!(
        catalog_model::external_catalog_model_indices(&ids).unwrap(),
        mojo_indices
    );
    assert_eq!(
        mojo_indices,
        catalog_model::external_catalog_model_indices_rust(&ids)
    );
    assert_eq!(mojo_indices, [1, 3, 4, 5, 6, 7, 9, 11, 12]);
}

#[test]
fn external_catalog_merge_keeps_first_dynamic_metadata_and_order() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let codex_home = crate::test_support::test_temp_root()
        .join(format!("prodex-external-catalog-merge-{stamp}"));
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE),
        serde_json::to_vec(&serde_json::json!({
            "models": [
                {
                    "id": "gpt-5.3-codex",
                    "name": "First Dynamic Name",
                    "max_prompt_tokens": 333000
                },
                {
                    "id": "GPT-5.3-CODEX",
                    "name": "Second Dynamic Name",
                    "max_prompt_tokens": 444000
                },
                {
                    "id": "copilot-dynamic-only",
                    "name": "Dynamic Only",
                    "context_window_tokens": 250000
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let models = catalog_model::external_catalog_models(
        &codex_home,
        ExternalCatalogProvider::Copilot,
        "custom-launch-model",
        100_000,
        90_000,
    )
    .unwrap();

    assert_eq!(models[0]["slug"], "custom-launch-model");
    assert_eq!(models[0]["priority"], 1);
    assert_eq!(models[1]["slug"], "gpt-5.3-codex");
    assert_eq!(models[1]["display_name"], "First Dynamic Name");
    assert_eq!(models[1]["context_window"], 333000);
    assert_eq!(models[1]["auto_compact_token_limit"], 316350);
    assert_eq!(models[2]["slug"], "copilot-dynamic-only");
    assert_eq!(
        models
            .iter()
            .filter(|model| model["slug"] == "gpt-5.3-codex")
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(codex_home);
}
