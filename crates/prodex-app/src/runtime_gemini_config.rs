use crate::runtime_catalog_config::{parse_catalog_u64, toml_string_literal};
use crate::{
    codex_cli_config_override_value, codex_effective_config_exact_value,
    codex_effective_config_value,
};
use anyhow::{Context, Result};
use prodex_mojo_core::super_provider_config::{
    RuntimeModelProviderClass, runtime_model_provider_class,
};
use prodex_provider_core::{
    PRODEX_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT as GEMINI_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_GEMINI_DEFAULT_CONTEXT_WINDOW as GEMINI_DEFAULT_CONTEXT_WINDOW,
    PRODEX_GEMINI_DEFAULT_MODEL as GEMINI_DEFAULT_MODEL, ProviderId, RuntimeModelCatalogInput,
    RuntimeModelCatalogPlanInput, plan_runtime_model_catalog, render_runtime_model_catalog,
};
use serde_json::json;
use std::ffi::OsString;
#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};

const GEMINI_MODEL_CATALOG_FILE: &str = "prodex-gemini-model-catalog.json";

#[cfg(test)]
#[path = "runtime_gemini_config/catalog_tests.rs"]
mod catalog_tests;
#[cfg(test)]
#[path = "runtime_gemini_config/override_tests.rs"]
mod override_tests;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuntimeGeminiModelResolution;

impl RuntimeGeminiModelResolution {
    pub(crate) fn from_current_settings() -> Result<Self> {
        Ok(Self)
    }

    pub(crate) fn fallback_chain(&self, model: &str) -> Option<Vec<String>> {
        let chain = prodex_provider_core::provider_model_fallback_chain(ProviderId::Gemini, model);
        (!chain.is_empty()).then_some(chain)
    }
}

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
    if !gemini_provider_enabled(codex_home, user_args)? {
        return Ok(user_args.to_vec());
    }
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }

    let model = gemini_model_for_launch(codex_home, user_args)?;
    let model_resolution = RuntimeGeminiModelResolution::from_current_settings()?;
    let context_window = gemini_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        GEMINI_DEFAULT_CONTEXT_WINDOW as u64,
    )?;
    let auto_compact_token_limit = gemini_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        GEMINI_DEFAULT_AUTO_COMPACT_LIMIT as u64,
    )?
    .min(context_window.saturating_sub(1));

    let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
    if write_catalog {
        write_gemini_model_catalog(
            codex_home,
            &model,
            &model_resolution,
            context_window,
            auto_compact_token_limit,
        )?;
    }

    let user_args = gemini_codex_args_without_consumed_overrides(user_args);
    let mut args = Vec::with_capacity(user_args.len() + 2);
    args.push(OsString::from("-c"));
    args.push(OsString::from(format!(
        "model_catalog_json={}",
        toml_string_literal(&catalog_path.to_string_lossy())
    )));
    args.extend(user_args);
    Ok(args)
}

fn gemini_provider_enabled(codex_home: &Path, user_args: &[OsString]) -> Result<bool> {
    Ok(
        codex_effective_config_value(codex_home, user_args, "model_provider")?.is_some_and(
            |provider| {
                runtime_model_provider_class(&provider)
                    .expect("runtime model-provider classification should accept Rust strings")
                    == Some(RuntimeModelProviderClass::Gemini)
            },
        ),
    )
}

fn gemini_model_for_launch(codex_home: &Path, user_args: &[OsString]) -> Result<String> {
    Ok(
        codex_effective_config_value(codex_home, user_args, "model")?
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| GEMINI_DEFAULT_MODEL.to_string()),
    )
}

fn gemini_u64_config_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    key: &str,
    default_value: u64,
) -> Result<u64> {
    let Some(value) = codex_effective_config_exact_value(codex_home, user_args, key)? else {
        return Ok(default_value);
    };
    parse_catalog_u64("Gemini", key, &value)
}

fn write_gemini_model_catalog(
    codex_home: &Path,
    model: &str,
    model_resolution: &RuntimeGeminiModelResolution,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<PathBuf> {
    crate::create_codex_home_if_missing(codex_home)?;
    let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
    let catalog = json!({
        "models": gemini_catalog_models(
            model,
            model_resolution,
            context_window,
            auto_compact_token_limit,
        )?
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize Gemini catalog")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&catalog_path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(catalog_path)
}

fn gemini_catalog_models(
    launch_model: &str,
    _model_resolution: &RuntimeGeminiModelResolution,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<Vec<serde_json::Value>> {
    let plan = plan_runtime_model_catalog(RuntimeModelCatalogPlanInput {
        provider: ProviderId::Gemini,
        launch_model,
        context_window,
        auto_compact_token_limit,
        dynamic_models: &[],
    })?;
    plan.into_iter()
        .map(|model| {
            let metadata =
                prodex_provider_core::provider_catalog_entry(ProviderId::Gemini, &model.slug);
            render_runtime_model_catalog(RuntimeModelCatalogInput {
                provider: ProviderId::Gemini,
                slug: &model.slug,
                display_name: &model.display_name,
                description: &model.description,
                priority: model.priority,
                context_window: model.context_window,
                auto_compact_token_limit: model.auto_compact_token_limit,
                supported_reasoning_efforts: metadata
                    .and_then(|entry| entry.supported_reasoning_efforts.as_deref()),
                default_reasoning_effort: metadata.and_then(|entry| entry.default_reasoning_effort),
            })
            .context("failed to render Gemini model catalog")
        })
        .collect()
}

fn gemini_codex_args_without_consumed_overrides(user_args: &[OsString]) -> Vec<OsString> {
    let mut args = user_args.to_vec();
    crate::app_commands::runtime_launch::remove_codex_config_overrides(
        &mut args,
        "model_thinking_budget",
        prodex_mojo_core::codex_config::ConfigOverrideRemoval::AllUtf8Forms,
    );
    args
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
        assert!(
            catalog["models"][0]["supported_reasoning_levels"]
                .as_array()
                .unwrap()
                .iter()
                .any(|level| level["effort"] == "high")
        );
        assert_eq!(catalog["models"][0]["supports_search_tool"], true);
        assert_eq!(catalog["models"][0]["input_modalities"][0], "text");
        assert_eq!(catalog["models"][0]["input_modalities"][1], "image");
        let model_slugs = catalog["models"].as_array().unwrap();
        assert!(model_slugs.len() > 1);
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-3.1-pro-preview")
        );
        assert!(model_slugs.iter().any(|model| model["slug"] == "auto"));
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "auto-gemini-3")
        );
        assert!(model_slugs.iter().any(|model| model["slug"] == "flash"));
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-2.5-flash")
        );
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-3-flash")
        );
        assert!(
            model_slugs
                .iter()
                .any(|model| model["slug"] == "gemini-3.5-flash")
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

    #[test]
    fn gemini_provider_codex_args_defaults_to_auto_model() {
        let codex_home = temp_codex_home("default-auto");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
        ];

        prepare_gemini_provider_codex_args(&codex_home, &user_args)
            .expect("Gemini args should prepare");

        let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "auto");
    }

    #[test]
    fn gemini_provider_codex_args_consumes_custom_thinking_budget_override() {
        let codex_home = temp_codex_home("custom-thinking-budget");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("-c"),
            OsString::from("model=\"gemini-2.5-pro\""),
            OsString::from("-c"),
            OsString::from("model_thinking_budget=1024"),
        ];

        let args = prepare_gemini_provider_codex_args(&codex_home, &user_args)
            .expect("Gemini args should prepare");
        assert!(
            !args
                .iter()
                .any(|arg| arg.to_string_lossy().contains("model_thinking_budget"))
        );

        let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        let model_slugs = catalog["models"].as_array().unwrap();

        let levels = model_slugs[0]["supported_reasoning_levels"]
            .as_array()
            .unwrap();
        assert!(levels.iter().any(|level| level["effort"] == "low"));
        assert!(model_slugs[0].get("parameters").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn gemini_catalog_write_replaces_symlink_without_touching_target() {
        let codex_home = temp_codex_home("catalog-symlink");
        fs::create_dir_all(&codex_home).unwrap();
        let target = codex_home.join("target.json");
        let catalog_path = codex_home.join(GEMINI_MODEL_CATALOG_FILE);
        fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &catalog_path).unwrap();
        let resolution = RuntimeGeminiModelResolution;

        write_gemini_model_catalog(&codex_home, "auto", &resolution, 100_000, 90_000).unwrap();

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
    fn gemini_provider_codex_args_rejects_invalid_numeric_catalog_overrides() {
        for (value, message) in [
            ("", "Gemini model_context_window cannot be empty"),
            (
                " 128000 ",
                "Gemini model_context_window must not contain whitespace",
            ),
            (
                "not-a-number",
                "Gemini model_context_window must be an unsigned integer",
            ),
            ("1", "Gemini model_context_window must be greater than 1"),
        ] {
            let codex_home = temp_codex_home("invalid-numeric");
            let user_args = vec![
                OsString::from("-c"),
                OsString::from("model_provider=\"prodex-gemini\""),
                OsString::from("-c"),
                OsString::from(format!(
                    "model_context_window={}",
                    toml_string_literal(value)
                )),
            ];

            let err = prepare_gemini_provider_codex_args(&codex_home, &user_args).unwrap_err();

            assert!(err.to_string().contains(message));
            let _ = fs::remove_dir_all(codex_home);
        }
    }
}
