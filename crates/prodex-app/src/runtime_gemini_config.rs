use crate::{codex_cli_config_override_value, codex_config_value};
use anyhow::{Context, Result};
use prodex_cli::{
    SUPER_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_GEMINI_DEFAULT_CONTEXT_WINDOW,
    SUPER_GEMINI_DEFAULT_MODEL, SUPER_GEMINI_PROVIDER_ID,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const GEMINI_MODEL_CATALOG_FILE: &str = "prodex-gemini-model-catalog.json";
// Mirrors Gemini CLI's visible models and aliases. The runtime bridge resolves
// aliases such as auto/pro/flash into concrete fallback chains before commit.
const GEMINI_CATALOG_MODELS: &[(&str, &str, &str)] = &[
    (
        "auto",
        "Gemini Auto",
        "Prodex Gemini fallback chain using Gemini CLI-style model routing.",
    ),
    (
        "pro",
        "Gemini Pro",
        "Prodex Gemini Pro alias routed through Gemini preview and stable Pro models.",
    ),
    (
        "flash",
        "Gemini Flash",
        "Prodex Gemini Flash alias routed through Gemini preview and stable Flash models.",
    ),
    (
        "flash-lite",
        "Gemini Flash Lite",
        "Prodex Gemini Flash-Lite alias routed through Gemini Flash-Lite models.",
    ),
    (
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
    ),
    (
        "gemini-3-pro-preview",
        "Gemini 3 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
    ),
    (
        "gemini-3.1-pro-preview-customtools",
        "Gemini 3.1 Pro Preview Custom Tools",
        "Gemini CLI preview Pro model variant for custom tools routed through Prodex.",
    ),
    (
        "gemini-3-flash-preview",
        "Gemini 3 Flash Preview",
        "Gemini CLI preview Flash model routed through the Prodex Responses adapter.",
    ),
    (
        "gemini-3.1-flash-lite",
        "Gemini 3.1 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
    ),
    (
        "gemini-2.5-pro",
        "Gemini 2.5 Pro",
        "Gemini CLI stable Pro model routed through the Prodex Responses adapter.",
    ),
    (
        "gemini-2.5-flash",
        "Gemini 2.5 Flash",
        "Gemini CLI stable Flash model routed through the Prodex Responses adapter.",
    ),
    (
        "gemini-2.5-flash-lite",
        "Gemini 2.5 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
    ),
    (
        "gemma-4-31b-it",
        "Gemma 4 31B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
    ),
    (
        "gemma-4-26b-a4b-it",
        "Gemma 4 26B A4B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
    ),
];
const GEMINI_BASE_INSTRUCTIONS: &str = r#"You are Codex, a coding agent. You and the user share the same workspace.

Focus on the user's software task. Inspect the codebase before changing behavior, make narrow edits, preserve user changes, and verify with relevant tests or commands when feasible.

Use tools deliberately. For shell work, prefer fast focused commands. For file edits, keep changes minimal and explain non-obvious logic in short comments only when useful."#;

pub(crate) fn prepare_gemini_provider_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    gemini_provider_codex_args(codex_home, user_args, true)
}

pub(crate) fn preview_gemini_provider_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    gemini_provider_codex_args(codex_home, user_args, false)
}

fn gemini_provider_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
    write_catalog: bool,
) -> Result<Vec<OsString>> {
    if !gemini_provider_enabled(codex_home, user_args) {
        return Ok(user_args.to_vec());
    }
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }

    let model = gemini_model_for_launch(codex_home, user_args);
    let context_window = gemini_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        SUPER_GEMINI_DEFAULT_CONTEXT_WINDOW as u64,
    );
    let auto_compact_token_limit = gemini_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        SUPER_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT as u64,
    )
    .min(context_window.saturating_sub(1));
    let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
    if write_catalog {
        write_gemini_model_catalog(codex_home, &model, context_window, auto_compact_token_limit)?;
    }

    let mut args = Vec::with_capacity(user_args.len() + 2);
    args.push(OsString::from("-c"));
    args.push(OsString::from(format!(
        "model_catalog_json={}",
        toml_string_literal(&catalog_path.to_string_lossy())
    )));
    args.extend(user_args.iter().cloned());
    Ok(args)
}

fn gemini_provider_enabled(codex_home: &Path, user_args: &[OsString]) -> bool {
    codex_cli_config_override_value(user_args, "model_provider")
        .or_else(|| codex_config_value(codex_home, "model_provider"))
        .is_some_and(|provider| provider.eq_ignore_ascii_case(SUPER_GEMINI_PROVIDER_ID))
}

fn gemini_model_for_launch(codex_home: &Path, user_args: &[OsString]) -> String {
    codex_cli_config_override_value(user_args, "model")
        .or_else(|| codex_config_value(codex_home, "model"))
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| SUPER_GEMINI_DEFAULT_MODEL.to_string())
}

fn gemini_u64_config_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    key: &str,
    default_value: u64,
) -> u64 {
    codex_cli_config_override_value(user_args, key)
        .or_else(|| codex_config_value(codex_home, key))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 1)
        .unwrap_or(default_value)
}

fn write_gemini_model_catalog(
    codex_home: &Path,
    model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<PathBuf> {
    fs::create_dir_all(codex_home)
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
    let catalog = json!({
        "models": gemini_catalog_models(model, context_window, auto_compact_token_limit)
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize Gemini catalog")?;
    fs::write(&catalog_path, contents)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(catalog_path)
}

fn gemini_catalog_models(
    launch_model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Vec<serde_json::Value> {
    let mut models = Vec::with_capacity(GEMINI_CATALOG_MODELS.len() + 1);
    let mut seen = BTreeSet::new();

    for slug in
        std::iter::once(launch_model).chain(GEMINI_CATALOG_MODELS.iter().map(|(slug, _, _)| *slug))
    {
        let slug = slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_ascii_lowercase()) {
            continue;
        }
        let priority = models.len() + 1;
        let (display_name, description) = gemini_catalog_model_metadata(slug);
        models.push(gemini_catalog_model(
            slug,
            display_name,
            description,
            priority,
            context_window,
            auto_compact_token_limit,
        ));
    }

    models
}

fn gemini_catalog_model_metadata(model: &str) -> (&str, &'static str) {
    GEMINI_CATALOG_MODELS
        .iter()
        .find(|(slug, _, _)| model.eq_ignore_ascii_case(slug))
        .map(|(_, display_name, description)| (*display_name, *description))
        .unwrap_or((
            model,
            "Gemini model routed through the Prodex Responses adapter.",
        ))
}

fn gemini_catalog_model(
    slug: &str,
    display_name: &str,
    description: &str,
    priority: usize,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> serde_json::Value {
    json!({
        "slug": slug,
        "display_name": display_name,
        "description": description,
        "default_reasoning_level": "high",
        "supported_reasoning_levels": [
            {
                "effort": "minimal",
                "description": "Disable Gemini thinking budget where supported"
            },
            {
                "effort": "low",
                "description": "Small Gemini thinking budget"
            },
            {
                "effort": "medium",
                "description": "Default Gemini thinking budget"
            },
            {
                "effort": "high",
                "description": "High Gemini thinking budget"
            },
            {
                "effort": "xhigh",
                "description": "Maximum Gemini thinking effort"
            }
        ],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": priority,
        "additional_speed_tiers": [],
        "service_tiers": [],
        "default_service_tier": null,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": GEMINI_BASE_INSTRUCTIONS,
        "supports_reasoning_summaries": true,
        "default_reasoning_summary": "none",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": "freeform",
        "web_search_tool_type": "text",
        "truncation_policy": {
            "mode": "tokens",
            "limit": 10000
        },
        "supports_parallel_tool_calls": true,
        "supports_image_detail_original": false,
        "context_window": context_window,
        "max_context_window": context_window,
        "auto_compact_token_limit": auto_compact_token_limit,
        "effective_context_window_percent": 95,
        "experimental_supported_tools": [],
        "input_modalities": ["text"],
        "supports_search_tool": false
    })
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_codex_home(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gemini-config-{name}-{stamp}"))
    }

    #[test]
    fn gemini_provider_codex_args_write_model_catalog() {
        let codex_home = temp_codex_home("catalog");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("-c"),
            OsString::from("model=\"gemini-2.5-pro\""),
        ];

        let args = prepare_gemini_provider_codex_args(&codex_home, &user_args)
            .expect("Gemini args should prepare");
        assert_eq!(args[0], OsString::from("-c"));
        assert!(args[1].to_string_lossy().starts_with("model_catalog_json="));

        let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "gemini-2.5-pro");
        assert_eq!(catalog["models"][0]["default_reasoning_level"], "high");
        let model_slugs = catalog["models"].as_array().unwrap();
        assert!(model_slugs.len() > 1);
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-3.1-pro-preview")
        );
        assert!(model_slugs.iter().any(|model| model["slug"] == "auto"));
        assert!(model_slugs.iter().any(|model| model["slug"] == "flash"));
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-2.5-flash")
        );
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-2.5-flash-lite")
        );
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemma-4-31b-it")
        );
    }

    #[test]
    fn gemini_provider_codex_args_keeps_custom_launch_model_in_catalog() {
        let codex_home = temp_codex_home("custom-catalog");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("-c"),
            OsString::from("model=\"gemini-custom\""),
        ];

        prepare_gemini_provider_codex_args(&codex_home, &user_args)
            .expect("Gemini args should prepare");

        let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        let model_slugs = catalog["models"].as_array().unwrap();
        assert_eq!(model_slugs[0]["slug"], "gemini-custom");
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-2.5-pro")
        );
    }
}
