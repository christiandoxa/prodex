use crate::{
    codex_cli_config_override_exact_value, codex_cli_config_override_value,
    codex_config_exact_value, codex_config_value,
};
use anyhow::{Context, Result};
use prodex_cli::SUPER_GEMINI_PROVIDER_ID;
use prodex_runtime_gemini::{
    GEMINI_DEFAULT_AUTO_COMPACT_LIMIT, GEMINI_DEFAULT_CONTEXT_WINDOW, GEMINI_DEFAULT_MODEL,
    gemini_model_catalog,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};

const GEMINI_MODEL_CATALOG_FILE: &str = "prodex-gemini-model-catalog.json";
const GEMINI_BASE_INSTRUCTIONS: &str = r#"You are Codex, a coding agent. You and the user share the same workspace.

Focus on the user's software task. Inspect the codebase before changing behavior, make narrow edits, preserve user changes, and verify with relevant tests or commands when feasible.

Use tools deliberately. For shell work, prefer fast focused commands. For file edits, keep changes minimal and explain non-obvious logic in short comments only when useful."#;

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGeminiModelResolution {
    catalog_models: Vec<RuntimeGeminiDynamicModel>,
    fallback_chains: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
struct RuntimeGeminiDynamicModel {
    slug: String,
    display_name: String,
    description: String,
}

impl RuntimeGeminiModelResolution {
    pub(crate) fn from_current_settings() -> Self {
        let cwd = std::env::current_dir().ok();
        Self::from_settings_sources(crate::gemini_settings_sources(cwd.as_deref()))
    }

    fn from_settings_sources(sources: Vec<crate::GeminiSettingsSource>) -> Self {
        let mut resolution = Self::default();
        for source in sources {
            let Some(model_configs) = source
                .value
                .get("modelConfigs")
                .or_else(|| source.value.get("model_configs"))
            else {
                continue;
            };
            resolution.apply_model_configs(model_configs);
        }
        resolution
    }

    pub(crate) fn fallback_chain(&self, model: &str) -> Option<Vec<String>> {
        let key = model.trim().to_ascii_lowercase();
        self.fallback_chains
            .get(&key)
            .filter(|chain| !chain.is_empty())
            .cloned()
    }

    fn catalog_models(&self) -> impl Iterator<Item = &RuntimeGeminiDynamicModel> {
        self.catalog_models.iter()
    }

    fn apply_model_configs(&mut self, model_configs: &serde_json::Value) {
        self.apply_model_definitions(model_configs);
        self.apply_model_aliases(model_configs, "aliases");
        self.apply_model_aliases(model_configs, "customAliases");
        self.apply_model_aliases(model_configs, "custom_aliases");
        self.apply_model_id_resolutions(model_configs);
        self.apply_model_chains(model_configs);
    }

    fn apply_model_definitions(&mut self, model_configs: &serde_json::Value) {
        let Some(definitions) = model_configs
            .get("modelDefinitions")
            .or_else(|| model_configs.get("model_definitions"))
            .and_then(serde_json::Value::as_object)
        else {
            return;
        };
        for (slug, definition) in definitions {
            if !definition
                .get("isVisible")
                .or_else(|| definition.get("is_visible"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }
            let display_name = definition
                .get("displayName")
                .or_else(|| definition.get("display_name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(slug);
            self.remember_catalog_model(slug, display_name, "Gemini CLI modelConfig model.");
        }
    }

    fn apply_model_aliases(&mut self, model_configs: &serde_json::Value, key: &str) {
        let Some(aliases) = model_configs
            .get(key)
            .and_then(serde_json::Value::as_object)
        else {
            return;
        };
        for (alias, config) in aliases {
            let Some(model) = config
                .get("modelConfig")
                .or_else(|| config.get("model_config"))
                .and_then(|config| config.get("model"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|model| !model.is_empty())
            else {
                continue;
            };
            self.remember_catalog_model(alias, alias, "Gemini CLI modelConfig alias.");
            self.remember_catalog_model(model, model, "Gemini CLI modelConfig target.");
            self.remember_chain(alias, [model]);
        }
    }

    fn apply_model_id_resolutions(&mut self, model_configs: &serde_json::Value) {
        let Some(resolutions) = model_configs
            .get("modelIdResolutions")
            .or_else(|| model_configs.get("model_id_resolutions"))
            .and_then(serde_json::Value::as_object)
        else {
            return;
        };
        for (model, resolution) in resolutions {
            let mut chain = Vec::new();
            if let Some(default) = resolution
                .get("default")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|target| !target.is_empty())
            {
                chain.push(default.to_string());
                self.remember_catalog_model(default, default, "Gemini CLI resolved model.");
            }
            if let Some(contexts) = resolution
                .get("contexts")
                .and_then(serde_json::Value::as_array)
            {
                for context in contexts {
                    if let Some(target) = context
                        .get("target")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|target| !target.is_empty())
                    {
                        chain.push(target.to_string());
                        self.remember_catalog_model(target, target, "Gemini CLI resolved model.");
                    }
                }
            }
            if !chain.is_empty() {
                self.remember_catalog_model(model, model, "Gemini CLI modelIdResolution alias.");
                self.remember_chain(model, chain.iter().map(String::as_str));
            }
        }
    }

    fn apply_model_chains(&mut self, model_configs: &serde_json::Value) {
        let Some(chains) = model_configs
            .get("modelChains")
            .or_else(|| model_configs.get("model_chains"))
            .and_then(serde_json::Value::as_object)
        else {
            return;
        };
        for (name, chain) in chains {
            let models = chain
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.get("model").and_then(serde_json::Value::as_str))
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .collect::<Vec<_>>();
            for model in &models {
                self.remember_catalog_model(model, model, "Gemini CLI modelChain model.");
            }
            self.remember_chain(name, models);
        }
    }

    fn remember_catalog_model(&mut self, slug: &str, display_name: &str, description: &str) {
        let slug = slug.trim();
        if slug.is_empty()
            || self
                .catalog_models
                .iter()
                .any(|model| model.slug.eq_ignore_ascii_case(slug))
        {
            return;
        }
        self.catalog_models.push(RuntimeGeminiDynamicModel {
            slug: slug.to_string(),
            display_name: display_name.trim().to_string(),
            description: description.to_string(),
        });
    }

    fn remember_chain<'a>(&mut self, model: &str, chain: impl IntoIterator<Item = &'a str>) {
        let mut seen = BTreeSet::new();
        let chain = chain
            .into_iter()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .filter(|item| seen.insert(item.to_ascii_lowercase()))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !chain.is_empty() {
            self.fallback_chains
                .insert(model.trim().to_ascii_lowercase(), chain);
        }
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
    if !gemini_provider_enabled(codex_home, user_args) {
        return Ok(user_args.to_vec());
    }
    if write_catalog {
        crate::prepare_gemini_cli_compat(codex_home)?;
    }
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }

    let model = gemini_model_for_launch(codex_home, user_args);
    let model_resolution = RuntimeGeminiModelResolution::from_current_settings();
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
        .unwrap_or_else(|| GEMINI_DEFAULT_MODEL.to_string())
}

fn gemini_u64_config_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    key: &str,
    default_value: u64,
) -> Result<u64> {
    let Some(value) = codex_cli_config_override_exact_value(user_args, key)
        .or_else(|| codex_config_exact_value(codex_home, key))
    else {
        return Ok(default_value);
    };
    runtime_catalog_u64_config_value("Gemini", key, &value)
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
        )
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
    model_resolution: &RuntimeGeminiModelResolution,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Vec<serde_json::Value> {
    let mut models = Vec::with_capacity(
        gemini_model_catalog().len() + model_resolution.catalog_models.len() + 1,
    );
    let mut seen = BTreeSet::new();

    for slug in std::iter::once(launch_model)
        .chain(
            model_resolution
                .catalog_models()
                .map(|model| model.slug.as_str()),
        )
        .chain(gemini_model_catalog().iter().map(|spec| spec.id))
    {
        let slug = slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_ascii_lowercase()) {
            continue;
        }
        let priority = models.len() + 1;
        let (display_name, description) = gemini_catalog_model_metadata(slug, model_resolution);
        models.push(gemini_catalog_model(
            slug,
            &display_name,
            &description,
            priority,
            context_window,
            auto_compact_token_limit,
        ));
    }

    models
}

fn gemini_catalog_model_metadata(
    model: &str,
    model_resolution: &RuntimeGeminiModelResolution,
) -> (String, String) {
    if let Some(model) = model_resolution
        .catalog_models()
        .find(|spec| model.eq_ignore_ascii_case(&spec.slug))
    {
        return (model.display_name.clone(), model.description.clone());
    }
    gemini_model_catalog()
        .iter()
        .find(|spec| model.eq_ignore_ascii_case(spec.id))
        .map(|spec| (spec.display_name.to_string(), spec.description.to_string()))
        .unwrap_or((
            model.to_string(),
            "Gemini model routed through the Prodex Responses adapter.".to_string(),
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
        "input_modalities": ["text", "image"],
        "supports_search_tool": true
    })
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn gemini_codex_args_without_consumed_overrides(user_args: &[OsString]) -> Vec<OsString> {
    let mut args = Vec::with_capacity(user_args.len());
    let mut index = 0;
    while index < user_args.len() {
        let Some(arg) = user_args[index].to_str() else {
            args.push(user_args[index].clone());
            index += 1;
            continue;
        };
        if matches!(arg, "-c" | "--config")
            && let Some(next) = user_args.get(index + 1)
            && next.to_str().is_some_and(gemini_consumed_config_override)
        {
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--config=")
            && gemini_consumed_config_override(value)
        {
            index += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("-c")
            && !value.is_empty()
            && value.contains('=')
            && gemini_consumed_config_override(value)
        {
            index += 1;
            continue;
        }
        args.push(user_args[index].clone());
        index += 1;
    }
    args
}

fn gemini_consumed_config_override(value: &str) -> bool {
    value
        .trim_start()
        .split_once('=')
        .map(|(key, _)| key.trim() == "model_thinking_budget")
        .unwrap_or(false)
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
    fn gemini_model_resolution_reads_cli_model_configs() {
        let source = crate::GeminiSettingsSource {
            name: "test".to_string(),
            directory: PathBuf::from("/tmp"),
            value: serde_json::json!({
                "modelConfigs": {
                    "customAliases": {
                        "fast-review": {
                            "modelConfig": {
                                "model": "gemini-custom-fast"
                            }
                        }
                    },
                    "modelIdResolutions": {
                        "pro-custom": {
                            "default": "gemini-custom-pro",
                            "contexts": [
                                {"target": "gemini-2.5-pro"}
                            ]
                        }
                    },
                    "modelDefinitions": {
                        "gemini-visible": {
                            "displayName": "Visible Gemini",
                            "isVisible": true
                        }
                    },
                    "modelChains": {
                        "custom-chain": [
                            {"model": "gemini-visible"},
                            {"model": "gemini-2.5-flash"}
                        ]
                    }
                }
            }),
            mcp_servers: BTreeMap::new(),
        };

        let resolution = RuntimeGeminiModelResolution::from_settings_sources(vec![source]);

        assert_eq!(
            resolution.fallback_chain("fast-review"),
            Some(vec!["gemini-custom-fast".to_string()])
        );
        assert_eq!(
            resolution.fallback_chain("pro-custom"),
            Some(vec![
                "gemini-custom-pro".to_string(),
                "gemini-2.5-pro".to_string()
            ])
        );
        assert_eq!(
            resolution.fallback_chain("custom-chain"),
            Some(vec![
                "gemini-visible".to_string(),
                "gemini-2.5-flash".to_string()
            ])
        );

        let catalog = gemini_catalog_models("fast-review", &resolution, 100_000, 90_000);
        assert!(catalog.iter().any(|model| model["slug"] == "fast-review"));
        assert!(
            catalog
                .iter()
                .any(|model| model["slug"] == "gemini-custom-fast")
        );
        assert!(
            catalog
                .iter()
                .any(|model| model["display_name"] == "Visible Gemini")
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
        let resolution = RuntimeGeminiModelResolution::default();

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
