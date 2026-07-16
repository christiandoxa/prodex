use crate::profile_commands::KIRO_MODEL_CATALOG_FILE;
use crate::{
    codex_cli_config_override_value, codex_effective_config_exact_value,
    codex_effective_config_value,
};
use anyhow::{Context, Result};
use prodex_cli::{
    SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
    SUPER_ANTHROPIC_DEFAULT_MODEL, SUPER_ANTHROPIC_PROVIDER_ID,
    SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
    SUPER_COPILOT_DEFAULT_MODEL, SUPER_COPILOT_PROVIDER_ID, SUPER_KIRO_DEFAULT_AUTO_COMPACT_LIMIT,
    SUPER_KIRO_DEFAULT_CONTEXT_WINDOW, SUPER_KIRO_DEFAULT_MODEL, SUPER_KIRO_PROVIDER_ID,
    super_copilot_prompt_token_limit_for_model,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const EXTERNAL_MODEL_CATALOG_FILE: &str = "prodex-external-provider-model-catalog.json";
pub(crate) const COPILOT_RUNTIME_MODEL_CATALOG_FILE: &str =
    "prodex-copilot-runtime-model-catalog.json";

#[derive(Clone, Copy)]
enum ExternalCatalogProvider {
    Anthropic,
    Copilot,
    Kiro,
}

pub(crate) fn write_copilot_runtime_model_catalog(
    codex_home: &Path,
    model_catalog: &[serde_json::Value],
) -> Result<()> {
    fs::create_dir_all(codex_home)
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    let catalog_path = codex_home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE);
    let catalog = json!({ "models": model_catalog });
    let contents = serde_json::to_string_pretty(&catalog)
        .context("failed to serialize Copilot model catalog")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&catalog_path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(())
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
    let Some(provider) = external_catalog_provider(codex_home, user_args)? else {
        return Ok(user_args.to_vec());
    };
    let model = external_catalog_model_for_launch(codex_home, user_args, provider)?;
    let context_window = external_catalog_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        provider.default_context_window() as u64,
    )?;
    let auto_compact_token_limit = external_catalog_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        provider.default_auto_compact_token_limit() as u64,
    )?
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
) -> Result<Option<ExternalCatalogProvider>> {
    let Some(provider) = codex_effective_config_value(codex_home, user_args, "model_provider")?
    else {
        return Ok(None);
    };
    Ok(
        if provider.eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID) {
            Some(ExternalCatalogProvider::Anthropic)
        } else if provider.eq_ignore_ascii_case(SUPER_COPILOT_PROVIDER_ID) {
            Some(ExternalCatalogProvider::Copilot)
        } else if provider.eq_ignore_ascii_case(SUPER_KIRO_PROVIDER_ID) {
            Some(ExternalCatalogProvider::Kiro)
        } else {
            None
        },
    )
}

fn external_catalog_model_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    provider: ExternalCatalogProvider,
) -> Result<String> {
    Ok(
        codex_effective_config_value(codex_home, user_args, "model")?
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| provider.default_model().to_string()),
    )
}

fn external_catalog_u64_config_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    key: &str,
    default_value: u64,
) -> Result<u64> {
    let Some(value) = codex_effective_config_exact_value(codex_home, user_args, key)? else {
        return Ok(default_value);
    };
    runtime_catalog_u64_config_value("external provider", key, &value)
}

fn runtime_catalog_u64_config_value(provider: &str, key: &str, value: &str) -> Result<u64> {
    if value.is_empty() {
        anyhow::bail!("{provider} {key} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        anyhow::bail!("{provider} {key} must not contain whitespace");
    }
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{provider} {key} must be an unsigned integer"))?;
    if parsed <= 1 {
        anyhow::bail!("{provider} {key} must be greater than 1");
    }
    Ok(parsed)
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
        "models": external_catalog_models(
            codex_home,
            provider,
            model,
            context_window,
            auto_compact_token_limit,
        )
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize provider catalog")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&catalog_path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(catalog_path)
}

fn external_catalog_models(
    codex_home: &Path,
    provider: ExternalCatalogProvider,
    launch_model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Vec<serde_json::Value> {
    let dynamic_models = external_dynamic_catalog_models(codex_home, provider);
    let mut models = Vec::with_capacity(provider.models().len() + dynamic_models.len() + 1);
    let mut seen = BTreeSet::new();
    let launch_model_context_window = dynamic_models
        .iter()
        .find(|model| model.slug.eq_ignore_ascii_case(launch_model))
        .and_then(|model| model.context_window)
        .or_else(|| provider.model_prompt_token_limit(launch_model));
    let default_compact_limit = auto_compact_token_limit;
    for slug in std::iter::once((launch_model, launch_model_context_window))
        .chain(dynamic_models.iter().map(|model| {
            (
                model.slug.as_str(),
                model
                    .context_window
                    .or_else(|| provider.model_prompt_token_limit(&model.slug)),
            )
        }))
        .chain(
            provider
                .models()
                .iter()
                .map(|model| (model.0, provider.model_prompt_token_limit(model.0))),
        )
    {
        let (slug, per_model_context_window) = slug;
        let slug = slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_ascii_lowercase()) {
            continue;
        }
        let priority = models.len() + 1;
        let dynamic_model = dynamic_models
            .iter()
            .find(|model| model.slug.eq_ignore_ascii_case(slug));
        let (fallback_display_name, fallback_description) = provider.model_metadata(slug);
        let display_name = dynamic_model
            .and_then(|model| model.display_name.as_deref())
            .unwrap_or(fallback_display_name);
        let description = dynamic_model
            .and_then(|model| model.description.as_deref())
            .unwrap_or(fallback_description);
        let model_context_window = per_model_context_window.unwrap_or(context_window);
        let model_compact_limit = per_model_context_window
            .map(|cw| cw.saturating_mul(95).saturating_div(100))
            .unwrap_or(default_compact_limit)
            .min(model_context_window.saturating_sub(1));
        models.push(external_catalog_model(
            provider,
            slug,
            display_name,
            description,
            priority,
            model_context_window,
            model_compact_limit,
        ));
    }
    models
}

#[derive(Clone, Debug)]
struct ExternalDynamicCatalogModel {
    slug: String,
    display_name: Option<String>,
    description: Option<String>,
    context_window: Option<u64>,
}

fn external_dynamic_catalog_models(
    codex_home: &Path,
    provider: ExternalCatalogProvider,
) -> Vec<ExternalDynamicCatalogModel> {
    let catalog_file = match provider {
        ExternalCatalogProvider::Copilot => COPILOT_RUNTIME_MODEL_CATALOG_FILE,
        ExternalCatalogProvider::Kiro => KIRO_MODEL_CATALOG_FILE,
        ExternalCatalogProvider::Anthropic => return Vec::new(),
    };
    let catalog_path = codex_home.join(catalog_file);
    let Ok(contents) = fs::read_to_string(catalog_path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return Vec::new();
    };
    let Some(models) = value.get("models").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    models
        .iter()
        .filter_map(|model| {
            let slug = model
                .get("id")
                .or_else(|| model.get("slug"))
                .or_else(|| model.get("model"))
                .and_then(serde_json::Value::as_str)?
                .trim();
            let context_window = copilot_catalog_entry_prompt_token_limit(model)
                .or_else(|| {
                    model
                        .get("context_window_tokens")
                        .and_then(serde_json::Value::as_u64)
                })
                .or_else(|| {
                    model
                        .get("context_window")
                        .and_then(serde_json::Value::as_u64)
                })
                .filter(|cw| *cw > 1);
            if slug.is_empty() || !seen.insert(slug.to_ascii_lowercase()) {
                return None;
            }
            if matches!(provider, ExternalCatalogProvider::Copilot)
                && !copilot_dynamic_model_is_responses_compatible(provider, slug)
            {
                return None;
            }
            Some(ExternalDynamicCatalogModel {
                slug: slug.to_string(),
                display_name: model
                    .get("name")
                    .or_else(|| model.get("model_name"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                description: model
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                context_window,
            })
        })
        .collect()
}

fn copilot_catalog_entry_prompt_token_limit(model: &serde_json::Value) -> Option<u64> {
    model
        .get("max_prompt_tokens")
        .or_else(|| {
            model
                .get("capabilities")
                .and_then(|c| c.get("limits"))
                .and_then(|l| l.get("max_prompt_tokens"))
        })
        .and_then(serde_json::Value::as_u64)
        .filter(|tokens| *tokens > 1)
}

fn copilot_dynamic_model_is_responses_compatible(
    provider: ExternalCatalogProvider,
    slug: &str,
) -> bool {
    let lower = slug.trim().to_ascii_lowercase();
    provider
        .models()
        .iter()
        .any(|(model, _, _)| model.eq_ignore_ascii_case(&lower))
        || lower.starts_with("gpt-5")
        || lower.starts_with("claude-")
        || lower.starts_with("gemini-")
        || lower.starts_with("mai-")
        || lower.starts_with("raptor-")
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
        ExternalCatalogProvider::Anthropic
        | ExternalCatalogProvider::Copilot
        | ExternalCatalogProvider::Kiro => {
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
                "effort": "low",
                "description": "Low reasoning effort"
            },
            {
                "effort": "medium",
                "description": "Medium reasoning effort"
            },
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
        "base_instructions": "",
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
            Self::Kiro => SUPER_KIRO_DEFAULT_MODEL,
        }
    }

    fn default_context_window(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
            Self::Copilot => SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
            Self::Kiro => SUPER_KIRO_DEFAULT_CONTEXT_WINDOW,
        }
    }

    fn default_auto_compact_token_limit(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Copilot => SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Kiro => SUPER_KIRO_DEFAULT_AUTO_COMPACT_LIMIT,
        }
    }

    fn model_prompt_token_limit(self, model: &str) -> Option<u64> {
        match self {
            Self::Anthropic => None,
            Self::Copilot => super_copilot_prompt_token_limit_for_model(model).map(|v| v as u64),
            Self::Kiro => None,
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
                    "gpt-5.3-codex",
                    "GPT-5.3 Codex",
                    "GPT-5.3 Codex routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.5",
                    "GPT-5.5",
                    "GPT-5.5 routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.4",
                    "GPT-5.4",
                    "GPT-5.4 routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.4-mini",
                    "GPT-5.4 Mini",
                    "GPT-5.4 Mini routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.4-nano",
                    "GPT-5.4 Nano",
                    "GPT-5.4 Nano routed through GitHub Copilot.",
                ),
                (
                    "gpt-5-mini",
                    "GPT-5 Mini",
                    "GPT-5 Mini routed through GitHub Copilot.",
                ),
                (
                    "claude-sonnet-4-6",
                    "Claude Sonnet 4.6",
                    "Claude Sonnet 4.6 routed through GitHub Copilot.",
                ),
                (
                    "claude-opus-4-8",
                    "Claude Opus 4.8",
                    "Claude Opus 4.8 routed through GitHub Copilot.",
                ),
                (
                    "claude-opus-4-7",
                    "Claude Opus 4.7",
                    "Claude Opus 4.7 routed through GitHub Copilot.",
                ),
                (
                    "claude-opus-4-6-fast",
                    "Claude Opus 4.6 Fast",
                    "Claude Opus 4.6 Fast routed through GitHub Copilot.",
                ),
                (
                    "claude-opus-4-6",
                    "Claude Opus 4.6",
                    "Claude Opus 4.6 routed through GitHub Copilot.",
                ),
                (
                    "claude-opus-4-5",
                    "Claude Opus 4.5",
                    "Claude Opus 4.5 routed through GitHub Copilot.",
                ),
                (
                    "claude-fable-5",
                    "Claude Fable 5",
                    "Claude Fable 5 routed through GitHub Copilot.",
                ),
                (
                    "claude-sonnet-4-5",
                    "Claude Sonnet 4.5",
                    "Claude Sonnet 4.5 routed through GitHub Copilot.",
                ),
                (
                    "claude-haiku-4-5",
                    "Claude Haiku 4.5",
                    "Claude Haiku 4.5 routed through GitHub Copilot.",
                ),
                (
                    "gemini-3.1-pro-preview",
                    "Gemini 3.1 Pro Preview",
                    "Gemini 3.1 Pro Preview routed through GitHub Copilot.",
                ),
                (
                    "gemini-2.5-pro",
                    "Gemini 2.5 Pro",
                    "Gemini 2.5 Pro routed through GitHub Copilot.",
                ),
                (
                    "gemini-3-flash",
                    "Gemini 3 Flash",
                    "Gemini 3 Flash routed through GitHub Copilot.",
                ),
                (
                    "gemini-3.5-flash",
                    "Gemini 3.5 Flash",
                    "Gemini 3.5 Flash routed through GitHub Copilot.",
                ),
                (
                    "mai-code-1-flash",
                    "MAI-Code-1-Flash",
                    "MAI-Code-1-Flash routed through GitHub Copilot.",
                ),
                (
                    "raptor-mini",
                    "Raptor Mini",
                    "Raptor Mini routed through GitHub Copilot.",
                ),
                (
                    "gpt-5.1-codex",
                    "GPT-5.1 Codex",
                    "Legacy GPT-5.1 Codex entry kept for GitHub Copilot compatibility.",
                ),
            ],
            Self::Kiro => &[(
                "auto",
                "Kiro Auto",
                "Kiro selects the model for the task using the imported account catalog.",
            )],
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
            OsString::from("model=\"gpt-5.3-codex\""),
        ];

        let args = prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Copilot catalog should prepare");
        assert_eq!(args[0], OsString::from("-c"));
        assert!(args[1].to_string_lossy().starts_with("model_catalog_json="));

        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "gpt-5.3-codex");
        assert_eq!(catalog["models"][0]["context_window"], 272000);
        assert_eq!(catalog["models"][0]["auto_compact_token_limit"], 258400);
        assert_eq!(catalog["models"][0]["supports_search_tool"], true);
        assert_eq!(catalog["models"][0]["web_search_tool_type"], "text");
    }

    #[test]
    fn external_provider_catalog_args_use_imported_kiro_models() {
        let codex_home = temp_codex_home("kiro");
        fs::create_dir_all(&codex_home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(
            codex_home.join(KIRO_MODEL_CATALOG_FILE),
            serde_json::to_vec(&json!({
                "models": [
                    {
                        "id": "auto",
                        "name": "auto",
                        "description": "Models chosen by task",
                        "context_window_tokens": 1_000_000
                    },
                    {
                        "id": "deepseek-3.2",
                        "name": "deepseek-3.2",
                        "description": "Experimental preview of DeepSeek V3.2",
                        "context_window_tokens": 164_000
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-kiro\""),
            OsString::from("-c"),
            OsString::from("model=\"auto\""),
        ];

        let args = prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Kiro catalog should prepare");
        assert!(args[1].to_string_lossy().starts_with("model_catalog_json="));

        let catalog: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(codex_home.join(EXTERNAL_MODEL_CATALOG_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(catalog["models"][0]["slug"], "auto");
        assert_eq!(catalog["models"][0]["context_window"], 1_000_000);
        assert_eq!(catalog["models"][1]["slug"], "deepseek-3.2");
        assert_eq!(catalog["models"][1]["context_window"], 164_000);
        assert_eq!(
            catalog["models"][1]["description"],
            "Experimental preview of DeepSeek V3.2"
        );
    }

    #[test]
    fn external_provider_catalog_args_write_full_copilot_reasoning_efforts() {
        let codex_home = temp_codex_home("copilot-efforts");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-copilot\""),
            OsString::from("-c"),
            OsString::from("model=\"gpt-5.5\""),
        ];

        prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Copilot catalog should prepare");

        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "gpt-5.5");
        assert_eq!(catalog["models"][0]["context_window"], 922000);
        assert_eq!(catalog["models"][0]["auto_compact_token_limit"], 875900);
        let efforts = catalog["models"][0]["supported_reasoning_levels"]
            .as_array()
            .expect("reasoning levels should be present")
            .iter()
            .map(|level| level["effort"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(efforts, vec!["low", "medium", "high", "xhigh"]);
    }

    #[test]
    fn external_provider_catalog_args_includes_copilot_runtime_token_catalog() {
        let codex_home = temp_codex_home("copilot-dynamic");
        write_copilot_runtime_model_catalog(
            &codex_home,
            &[
                json!({
                    "id": "gpt-5.6-account-only-model",
                    "object": "model",
                    "owned_by": "github-copilot"
                }),
                json!({
                    "id": "gpt-4o",
                    "object": "model",
                    "owned_by": "github-copilot"
                }),
            ],
        )
        .expect("dynamic Copilot catalog should write");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-copilot\""),
        ];

        prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Copilot catalog should prepare");

        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert!(
            catalog["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|model| { model["slug"] == "gpt-5.6-account-only-model" })
        );
        assert!(
            !catalog["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|model| { model["slug"] == "gpt-4o" })
        );
    }

    #[test]
    fn external_provider_catalog_uses_copilot_dynamic_prompt_limit_as_context_budget() {
        let codex_home = temp_codex_home("copilot-dynamic-prompt-limit");
        write_copilot_runtime_model_catalog(
            &codex_home,
            &[json!({
                "id": "gpt-5.6-account-only-model",
                "object": "model",
                "owned_by": "github-copilot",
                "capabilities": {
                    "limits": {
                        "max_context_window_tokens": 1050000,
                        "max_prompt_tokens": 333000,
                        "max_output_tokens": 128000
                    }
                }
            })],
        )
        .expect("dynamic Copilot catalog should write");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-copilot\""),
            OsString::from("-c"),
            OsString::from("model=\"gpt-5.6-account-only-model\""),
        ];

        prepare_external_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("Copilot catalog should prepare");

        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "gpt-5.6-account-only-model");
        assert_eq!(catalog["models"][0]["context_window"], 333000);
        assert_eq!(catalog["models"][0]["auto_compact_token_limit"], 316350);
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

    #[cfg(unix)]
    #[test]
    fn external_catalog_write_replaces_symlink_without_touching_target() {
        let codex_home = temp_codex_home("external-catalog-symlink");
        fs::create_dir_all(&codex_home).unwrap();
        let target = codex_home.join("target.json");
        let catalog_path = codex_home.join(EXTERNAL_MODEL_CATALOG_FILE);
        fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &catalog_path).unwrap();

        write_external_model_catalog(
            &codex_home,
            ExternalCatalogProvider::Anthropic,
            "claude-sonnet-4-6",
            100_000,
            90_000,
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "do not touch");
        assert!(
            !fs::symlink_metadata(&catalog_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let _ = fs::remove_dir_all(codex_home);
    }

    #[cfg(unix)]
    #[test]
    fn copilot_runtime_catalog_write_replaces_symlink_without_touching_target() {
        let codex_home = temp_codex_home("copilot-runtime-catalog-symlink");
        fs::create_dir_all(&codex_home).unwrap();
        let target = codex_home.join("target.json");
        let catalog_path = codex_home.join(COPILOT_RUNTIME_MODEL_CATALOG_FILE);
        fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &catalog_path).unwrap();

        write_copilot_runtime_model_catalog(
            &codex_home,
            &[json!({
                "id": "gpt-5.6-account-only-model",
                "object": "model",
                "owned_by": "github-copilot"
            })],
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "do not touch");
        assert!(
            !fs::symlink_metadata(&catalog_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let _ = fs::remove_dir_all(codex_home);
    }

    #[test]
    fn external_provider_catalog_args_rejects_invalid_numeric_overrides() {
        for (value, message) in [
            ("", "external provider model_context_window cannot be empty"),
            (
                " 200000 ",
                "external provider model_context_window must not contain whitespace",
            ),
            (
                "not-a-number",
                "external provider model_context_window must be an unsigned integer",
            ),
            (
                "1",
                "external provider model_context_window must be greater than 1",
            ),
        ] {
            let codex_home = temp_codex_home("invalid-numeric");
            let user_args = vec![
                OsString::from("-c"),
                OsString::from("model_provider=\"prodex-anthropic\""),
                OsString::from("-c"),
                OsString::from(format!(
                    "model_context_window={}",
                    toml_string_literal(value)
                )),
            ];

            let err =
                prepare_external_provider_catalog_codex_args(&codex_home, &user_args).unwrap_err();

            assert!(err.to_string().contains(message));
            let _ = fs::remove_dir_all(codex_home);
        }
    }
}
