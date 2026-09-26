use super::*;
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "mojo-core")]
#[test]
fn external_catalog_model_indices_preserve_unicode_and_long_ids() {
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
    assert_eq!(
        catalog_model::external_catalog_model_indices(&ids).unwrap(),
        [1, 3, 4, 5, 6, 7, 9, 11, 12]
    );
}

#[cfg(feature = "mojo-core")]
#[test]
fn external_provider_launch_uses_first_dynamic_match_and_preserves_os_args() {
    let duplicate_ids = [" gpt-5.3-codex ", "GPT-5.3-CODEX", "copilot-dynamic-only"];
    assert_eq!(
        prodex_mojo_core::rich::merge_catalog_ids(&[], &duplicate_ids).unwrap(),
        [0, 2]
    );

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let codex_home = crate::test_support::test_temp_root()
        .join(format!("prodex-external-catalog-merge-{stamp}"));
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join("config.toml"),
        "model_provider = \"prodex-anthropic\"\nmodel = \"claude-opus-4-8\"\n",
    )
    .unwrap();
    std::fs::write(
        codex_home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE),
        serde_json::to_vec(&serde_json::json!({
            "models": [
                {
                    "id": " gpt-5.3-codex ",
                    "name": "First Dynamic Name",
                    "description": "First description",
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

    #[cfg(unix)]
    let opaque_arg = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(b"--opaque=\xff".to_vec())
    };
    #[cfg(not(unix))]
    let opaque_arg = OsString::from("--opaque-argument");
    let user_args = vec![
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-copilot\""),
        OsString::from("-c"),
        OsString::from("model=\"gpt-5.3-codex\""),
        opaque_arg,
    ];

    let args = prepare_external_provider_catalog_codex_args(&codex_home, &user_args).unwrap();
    assert_eq!(&args[2..], user_args.as_slice());

    let contents = std::fs::read_to_string(codex_home.join(EXTERNAL_MODEL_CATALOG_FILE)).unwrap();
    let catalog: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let models = catalog["models"].as_array().unwrap();
    assert_eq!(models[0]["slug"], "gpt-5.3-codex");
    assert_eq!(models[0]["priority"], 1);
    assert_eq!(models[0]["display_name"], "First Dynamic Name");
    assert_eq!(models[0]["description"], "First description");
    assert_eq!(models[0]["context_window"], 333000);
    assert_eq!(models[0]["auto_compact_token_limit"], 316350);
    assert_eq!(models[1]["slug"], "copilot-dynamic-only");
    assert_eq!(models[1]["priority"], 2);
    assert_eq!(models[1]["display_name"], "Dynamic Only");
    assert_eq!(models[1]["context_window"], 250000);
    assert_eq!(models[1]["auto_compact_token_limit"], 237500);
    assert_eq!(
        models
            .iter()
            .filter(|model| model["slug"] == "gpt-5.3-codex")
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(codex_home);
}
