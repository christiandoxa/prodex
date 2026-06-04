use crate::{codex_cli_config_override_value, codex_config_value};
use anyhow::{Context, Result};
use prodex_cli::{
    SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
    SUPER_ANTHROPIC_DEFAULT_MODEL, SUPER_ANTHROPIC_PROVIDER_ID,
    SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
    SUPER_COPILOT_DEFAULT_MODEL, SUPER_COPILOT_PROVIDER_ID,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const EXTERNAL_MODEL_CATALOG_FILE: &str = "prodex-external-provider-model-catalog.json";

#[derive(Clone, Copy)]
enum ExternalCatalogProvider {
    Anthropic,
    Copilot,
}

pub(crate) fn prepare_external_provider_catalog_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    external_provider_catalog_codex_args(codex_home, user_args, true)
}

pub(crate) fn preview_external_provider_catalog_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    external_provider_catalog_codex_args(codex_home, user_args, false)
}

fn external_provider_catalog_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
    write_catalog: bool,
) -> Result<Vec<OsString>> {
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }
    let Some(provider) = external_catalog_provider(codex_home, user_args) else {
        return Ok(user_args.to_vec());
    };

    let model = external_catalog_model_for_launch(codex_home, user_args, provider);
    let context_window = external_catalog_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        provider.default_context_window() as u64,
    );
    let auto_compact_token_limit = external_catalog_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        provider.default_auto_compact_token_limit() as u64,
    )
    .min(context_window.saturating_sub(1));
    let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
    if write_catalog {
        write_external_model_catalog(
            codex_home,
            provider,
            &model,
            context_window,
            auto_compact_token_limit,
        )?;
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

fn external_catalog_provider(
    codex_home: &Path,
    user_args: &[OsString],
) -> Option<ExternalCatalogProvider> {
    let provider = codex_cli_config_override_value(user_args, "model_provider")
        .or_else(|| codex_config_value(codex_home, "model_provider"))?;
    if provider.eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID) {
        Some(ExternalCatalogProvider::Anthropic)
    } else if provider.eq_ignore_ascii_case(SUPER_COPILOT_PROVIDER_ID) {
        Some(ExternalCatalogProvider::Copilot)
    } else {
        None
    }
}

fn external_catalog_model_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    provider: ExternalCatalogProvider,
) -> String {
    codex_cli_config_override_value(user_args, "model")
        .or_else(|| codex_config_value(codex_home, "model"))
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| provider.default_model().to_string())
}

fn external_catalog_u64_config_for_launch(
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

fn write_external_model_catalog(
    codex_home: &Path,
    provider: ExternalCatalogProvider,
    model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<PathBuf> {
    fs::create_dir_all(codex_home)
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
    let catalog = json!({
        "models": external_catalog_models(provider, model, context_window, auto_compact_token_limit)
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize provider catalog")?;
    fs::write(&catalog_path, contents)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(catalog_path)
}

fn external_catalog_models(
    provider: ExternalCatalogProvider,
    launch_model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Vec<serde_json::Value> {
    let mut models = Vec::with_capacity(provider.models().len() + 1);
    let mut seen = BTreeSet::new();
    for slug in std::iter::once(launch_model).chain(provider.models().iter().map(|model| model.0)) {
        let slug = slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_ascii_lowercase()) {
            continue;
        }
        let priority = models.len() + 1;
        let (display_name, description) = provider.model_metadata(slug);
        models.push(external_catalog_model(
            provider,
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

fn external_catalog_model(
    provider: ExternalCatalogProvider,
    slug: &str,
    display_name: &str,
    description: &str,
    priority: usize,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> serde_json::Value {
    let input_modalities = match provider {
        ExternalCatalogProvider::Anthropic | ExternalCatalogProvider::Copilot => {
            json!(["text", "image"])
        }
    };
    json!({
        "slug": slug,
        "display_name": display_name,
        "description": description,
        "default_reasoning_level": "high",
        "supported_reasoning_levels": [
            {
                "effort": "high",
                "description": "High reasoning effort"
            },
            {
                "effort": "xhigh",
                "description": "Max reasoning effort"
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
        "base_instructions": null,
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
        "input_modalities": input_modalities,
        "supports_search_tool": true
    })
}

impl ExternalCatalogProvider {
    fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_MODEL,
            Self::Copilot => SUPER_COPILOT_DEFAULT_MODEL,
        }
    }

    fn default_context_window(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
            Self::Copilot => SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
        }
    }

    fn default_auto_compact_token_limit(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Copilot => SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
        }
    }

    fn models(self) -> &'static [(&'static str, &'static str, &'static str)] {
        match self {
            Self::Anthropic => &[
                (
                    "auto",
                    "Claude Auto",
                    "Anthropic auto model routed through the Prodex Responses adapter.",
                ),
                (
                    "opus",
                    "Claude Opus",
                    "Claude Opus alias routed through the Prodex Responses adapter.",
                ),
                (
                    "sonnet",
                    "Claude Sonnet",
                    "Claude Sonnet alias routed through the Prodex Responses adapter.",
                ),
                (
                    "haiku",
                    "Claude Haiku",
                    "Claude Haiku alias routed through the Prodex Responses adapter.",
                ),
                (
                    "claude-opus-4-8",
                    "Claude Opus 4.8",
                    "Claude Opus 4.8 routed through the Prodex Responses adapter.",
                ),
                (
                    "claude-sonnet-4-6",
                    "Claude Sonnet 4.6",
                    "Claude Sonnet 4.6 routed through the Prodex Responses adapter.",
                ),
                (
                    "claude-haiku-4-5",
                    "Claude Haiku 4.5",
                    "Claude Haiku 4.5 routed through the Prodex Responses adapter.",
                ),
                (
                    "claude-opus-4-6",
                    "Claude Opus 4.6",
                    "Claude Opus 4.6 routed through the Prodex Responses adapter.",
                ),
                (
                    "claude-opus-4-20250514",
                    "Claude Opus 4",
                    "Claude Opus 4 routed through the Prodex Responses adapter.",
                ),
            ],
            Self::Copilot => &[
                (
                    "auto",
                    "GitHub Copilot Auto",
                    "GitHub Copilot auto model routed through the Prodex Responses adapter.",
                ),
                (
                    "codex",
                    "GitHub Copilot Codex",
                    "GitHub Copilot Codex alias routed through the Prodex Responses adapter.",
                ),
                (
                    "gpt-5.1-codex",
                    "GPT-5.1 Codex",
                    "GPT-5.1 Codex routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.4",
                    "GPT-5.4",
                    "GPT-5.4 routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.3-codex",
                    "GPT-5.3 Codex",
                    "GPT-5.3 Codex routed through GitHub Copilot.",
                ),
                (
                    "claude-sonnet-4-6",
                    "Claude Sonnet 4.6",
                    "Claude Sonnet 4.6 routed through GitHub Copilot.",
                ),
                (
                    "gemini-3.1-pro-preview",
                    "Gemini 3.1 Pro Preview",
                    "Gemini 3.1 Pro Preview routed through GitHub Copilot.",
                ),
            ],
        }
    }

    fn model_metadata(self, model: &str) -> (&str, &'static str) {
        self.models()
            .iter()
            .find(|(slug, _, _)| model.eq_ignore_ascii_case(slug))
            .map(|(_, display_name, description)| (*display_name, *description))
            .unwrap_or((
                model,
                "External provider model routed through the Prodex Responses adapter.",
            ))
    }
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
        std::env::temp_dir().join(format!("prodex-external-provider-config-{name}-{stamp}"))
    }

    #[test]
    fn external_provider_catalog_args_write_anthropic_catalog() {
        let codex_home = temp_codex_home("anthropic");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-anthropic\""),
            OsString::from("-c"),
            OsString::from("model=\"claude-sonnet-4-6\""),
        ];

        let args = prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Anthropic catalog should prepare");
        assert_eq!(args[0], OsString::from("-c"));
        assert!(args[1].to_string_lossy().starts_with("model_catalog_json="));

        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "claude-sonnet-4-6");
        assert_eq!(catalog["models"][0]["supports_search_tool"], true);
        assert_eq!(catalog["models"][0]["input_modalities"][1], "image");
    }

    #[test]
    fn external_provider_catalog_args_write_copilot_catalog() {
        let codex_home = temp_codex_home("copilot");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-copilot\""),
            OsString::from("-c"),
            OsString::from("model=\"gpt-5.1-codex\""),
        ];

        let args = prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Copilot catalog should prepare");
        assert_eq!(args[0], OsString::from("-c"));
        assert!(args[1].to_string_lossy().starts_with("model_catalog_json="));

        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "gpt-5.1-codex");
        assert_eq!(catalog["models"][0]["supports_search_tool"], true);
        assert_eq!(catalog["models"][0]["web_search_tool_type"], "text");
    }

    #[test]
    fn external_provider_catalog_args_respects_existing_catalog_override() {
        let codex_home = temp_codex_home("existing");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-anthropic\""),
            OsString::from("-c"),
            OsString::from("model_catalog_json=\"/tmp/custom.json\""),
        ];

        let args = prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("existing catalog should be preserved");

        assert_eq!(args, user_args);
        assert!(!codex_home.join(EXTERNAL_MODEL_CATALOG_FILE).exists());
    }
}
