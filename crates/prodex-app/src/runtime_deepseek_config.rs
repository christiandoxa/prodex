use crate::runtime_catalog_config::{parse_catalog_u64, toml_string_literal};
use crate::{
    RuntimeDeepSeekWebSearchMode, codex_cli_config_override_value, codex_config_value,
    codex_effective_config_exact_value, codex_effective_config_value,
};
use anyhow::{Context, Result, bail};
use prodex_cli::{
    SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW,
    SUPER_DEEPSEEK_DEFAULT_MODEL, SUPER_DEEPSEEK_PROVIDER_ID,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

const DEEPSEEK_MODEL_CATALOG_FILE: &str = "prodex-deepseek-model-catalog.json";
const DEEPSEEK_CATALOG_MODELS: &[(&str, &str, &str)] = &[
    (
        "auto",
        "DeepSeek Auto",
        "Prodex DeepSeek fallback chain routed through current DeepSeek models.",
    ),
    (
        "pro",
        "DeepSeek Pro",
        "Prodex DeepSeek Pro alias routed through DeepSeek V4 Pro.",
    ),
    (
        "flash",
        "DeepSeek Flash",
        "Prodex DeepSeek Flash alias routed through DeepSeek V4 Flash.",
    ),
    (
        "deepseek-v4-pro",
        "DeepSeek V4 Pro",
        "DeepSeek V4 Pro routed through the Prodex Responses adapter.",
    ),
    (
        "deepseek-v4-flash",
        "DeepSeek V4 Flash",
        "DeepSeek V4 Flash routed through the Prodex Responses adapter.",
    ),
    (
        "deepseek-chat",
        "DeepSeek Chat",
        "DeepSeek chat compatibility model routed through the Prodex Responses adapter.",
    ),
    (
        "deepseek-reasoner",
        "DeepSeek Reasoner",
        "DeepSeek reasoner compatibility model routed through the Prodex Responses adapter.",
    ),
];
const DEEPSEEK_BASE_INSTRUCTIONS: &str = r#"You are Codex, a coding agent. You and the user share the same workspace.

Focus on the user's software task. Inspect the codebase before changing behavior, make narrow edits, preserve user changes, and verify with relevant tests or commands when feasible.

Use tools deliberately. For shell work, prefer fast focused commands. For file edits, keep changes minimal and explain non-obvious logic in short comments only when useful."#;

pub(crate) fn runtime_deepseek_gateway_strict_tools(
    codex_home: &Path,
    environment: Option<&OsStr>,
) -> Result<bool> {
    if let Some(value) = runtime_deepseek_gateway_toml_value(codex_home, "deepseek.strict_tools") {
        return match value {
            toml::Value::Boolean(enabled) => Ok(enabled),
            toml::Value::String(value) => {
                runtime_deepseek_gateway_bool("deepseek.strict_tools", &value)
            }
            _ => bail!("deepseek.strict_tools must be a boolean"),
        };
    }
    let Some(value) = environment else {
        return Ok(false);
    };
    runtime_deepseek_gateway_bool(
        "PRODEX_DEEPSEEK_STRICT_TOOLS",
        runtime_deepseek_gateway_environment_text("PRODEX_DEEPSEEK_STRICT_TOOLS", value)?,
    )
}

pub(crate) fn runtime_deepseek_gateway_beta_base_url(
    codex_home: &Path,
    environment: Option<&OsStr>,
) -> Result<String> {
    let configured = codex_config_value(codex_home, "deepseek.beta_base_url")?
        .map(|value| ("deepseek.beta_base_url", value));
    let environment = if configured.is_none() {
        environment
            .map(|value| {
                runtime_deepseek_gateway_environment_text("PRODEX_DEEPSEEK_BETA_BASE_URL", value)
                    .map(str::to_string)
            })
            .transpose()?
    } else {
        None
    };
    let Some((name, value)) =
        configured.or_else(|| environment.map(|value| ("PRODEX_DEEPSEEK_BETA_BASE_URL", value)))
    else {
        return Ok("https://api.deepseek.com/beta".to_string());
    };
    if value.is_empty() {
        bail!("{name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{name} must not contain whitespace");
    }
    let normalized = value.trim_end_matches('/').to_string();
    let parsed = reqwest::Url::parse(&normalized).with_context(|| {
        format!("{name} must be an http(s) URL with host and no credentials, query, or fragment")
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        bail!("{name} must be an http(s) URL with host and no credentials, query, or fragment");
    }
    Ok(normalized)
}

pub(crate) fn runtime_deepseek_gateway_web_search_mode(
    codex_home: &Path,
    environment: Option<&OsStr>,
) -> Result<RuntimeDeepSeekWebSearchMode> {
    if let Some(value) = runtime_deepseek_gateway_toml_value(codex_home, "deepseek.web_search_mode")
    {
        let toml::Value::String(value) = value else {
            bail!("deepseek.web_search_mode must be a string");
        };
        return runtime_deepseek_gateway_web_search_value("deepseek.web_search_mode", &value);
    }
    let Some(value) = environment else {
        return Ok(RuntimeDeepSeekWebSearchMode::default());
    };
    runtime_deepseek_gateway_web_search_value(
        "PRODEX_DEEPSEEK_WEB_SEARCH_MODE",
        runtime_deepseek_gateway_environment_text("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", value)?,
    )
}

fn runtime_deepseek_gateway_bool(name: &str, value: &str) -> Result<bool> {
    if value.is_empty() {
        bail!("{name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{name} must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("{name} must be true or false"),
    }
}

fn runtime_deepseek_gateway_web_search_value(
    name: &str,
    value: &str,
) -> Result<RuntimeDeepSeekWebSearchMode> {
    if value.is_empty() {
        bail!("{name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{name} must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(RuntimeDeepSeekWebSearchMode::Auto),
        "off" | "disabled" | "disable" => Ok(RuntimeDeepSeekWebSearchMode::Off),
        "openai_chat" | "openai-chat" | "chat" => Ok(RuntimeDeepSeekWebSearchMode::OpenAiChat),
        "anthropic" => Ok(RuntimeDeepSeekWebSearchMode::Anthropic),
        "function_proxy" | "function-proxy" => Ok(RuntimeDeepSeekWebSearchMode::FunctionProxy),
        _ => bail!("{name} must be auto, off, openai_chat, anthropic, or function_proxy"),
    }
}

fn runtime_deepseek_gateway_toml_value(codex_home: &Path, key: &str) -> Option<toml::Value> {
    let contents = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    let value = toml::from_str::<toml::Value>(&contents).ok()?;
    let mut current = &value;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

fn runtime_deepseek_gateway_environment_text<'a>(name: &str, value: &'a OsStr) -> Result<&'a str> {
    value
        .to_str()
        .with_context(|| format!("{name} must be valid Unicode"))
}

pub(crate) fn prepare_deepseek_provider_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    deepseek_provider_codex_args(codex_home, user_args, true)
}

pub(crate) fn preview_deepseek_provider_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    deepseek_provider_codex_args(codex_home, user_args, false)
}

fn deepseek_provider_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
    write_catalog: bool,
) -> Result<Vec<OsString>> {
    if !deepseek_provider_enabled(codex_home, user_args)? {
        return Ok(user_args.to_vec());
    }
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }

    let model = deepseek_model_for_launch(codex_home, user_args)?;
    let context_window = deepseek_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW as u64,
    )?;
    let auto_compact_token_limit = deepseek_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT as u64,
    )?
    .min(context_window.saturating_sub(1));
    let catalog_path = codex_home.join(DEEPSEEK_MODEL_CATALOG_FILE);
    if write_catalog {
        write_deepseek_model_catalog(codex_home, &model, context_window, auto_compact_token_limit)?;
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

fn deepseek_provider_enabled(codex_home: &Path, user_args: &[OsString]) -> Result<bool> {
    Ok(
        codex_effective_config_value(codex_home, user_args, "model_provider")?
            .is_some_and(|provider| provider.eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID)),
    )
}

fn deepseek_model_for_launch(codex_home: &Path, user_args: &[OsString]) -> Result<String> {
    Ok(
        codex_effective_config_value(codex_home, user_args, "model")?
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| SUPER_DEEPSEEK_DEFAULT_MODEL.to_string()),
    )
}

fn deepseek_u64_config_for_launch(
    codex_home: &Path,
    user_args: &[OsString],
    key: &str,
    default_value: u64,
) -> Result<u64> {
    let Some(value) = codex_effective_config_exact_value(codex_home, user_args, key)? else {
        return Ok(default_value);
    };
    parse_catalog_u64("DeepSeek", key, &value)
}

fn write_deepseek_model_catalog(
    codex_home: &Path,
    model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<PathBuf> {
    fs::create_dir_all(codex_home)
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    let catalog_path = codex_home.join(DEEPSEEK_MODEL_CATALOG_FILE);
    let catalog = json!({
        "models": deepseek_catalog_models(model, context_window, auto_compact_token_limit)
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize DeepSeek catalog")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&catalog_path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(catalog_path)
}

fn deepseek_catalog_models(
    launch_model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Vec<serde_json::Value> {
    let mut models = Vec::with_capacity(DEEPSEEK_CATALOG_MODELS.len() + 1);
    let mut seen = BTreeSet::new();

    for slug in std::iter::once(launch_model)
        .chain(DEEPSEEK_CATALOG_MODELS.iter().map(|(slug, _, _)| *slug))
    {
        let slug = slug.trim();
        if slug.is_empty() || !seen.insert(slug.to_ascii_lowercase()) {
            continue;
        }
        let priority = models.len() + 1;
        let (display_name, description) = deepseek_catalog_model_metadata(slug);
        models.push(deepseek_catalog_model(
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

fn deepseek_catalog_model_metadata(model: &str) -> (&str, &'static str) {
    DEEPSEEK_CATALOG_MODELS
        .iter()
        .find(|(slug, _, _)| model.eq_ignore_ascii_case(slug))
        .map(|(_, display_name, description)| (*display_name, *description))
        .unwrap_or((
            model,
            "DeepSeek model routed through the Prodex Responses adapter.",
        ))
}

fn deepseek_catalog_model(
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
                "effort": "low",
                "description": "DeepSeek low reasoning effort"
            },
            {
                "effort": "medium",
                "description": "DeepSeek medium reasoning effort"
            },
            {
                "effort": "high",
                "description": "DeepSeek high reasoning effort"
            },
            {
                "effort": "xhigh",
                "description": "DeepSeek max reasoning effort"
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
        "base_instructions": DEEPSEEK_BASE_INSTRUCTIONS,
        "supports_reasoning_summaries": false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_codex_home(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-deepseek-config-{name}-{stamp}"))
    }

    #[test]
    fn deepseek_provider_codex_args_write_single_model_catalog() {
        let codex_home = temp_codex_home("catalog");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-deepseek\""),
            OsString::from("-c"),
            OsString::from("model=\"deepseek-v4-pro\""),
        ];

        let args = prepare_deepseek_provider_codex_args(&codex_home, &user_args)
            .expect("DeepSeek args should prepare");
        let catalog_path = codex_home.join(DEEPSEEK_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&catalog_path).expect("catalog should be written"),
        )
        .expect("catalog should parse");

        assert!(
            args.iter()
                .any(|arg| { arg.to_string_lossy().contains("model_catalog_json=") })
        );
        assert_eq!(catalog["models"][0]["slug"], "deepseek-v4-pro");
        assert_eq!(catalog["models"][0]["supports_search_tool"], false);
        assert_eq!(catalog["models"][0]["supports_reasoning_summaries"], false);
        assert_eq!(
            catalog["models"][0]["experimental_supported_tools"],
            json!([])
        );
        assert_eq!(catalog["models"][0]["input_modalities"], json!(["text"]));
        assert_eq!(
            catalog["models"][0]["supports_image_detail_original"],
            false
        );
        assert!(
            catalog["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|model| model["slug"] == "deepseek-v4-flash")
        );
        assert!(
            catalog["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|model| model["slug"] == "auto")
        );
        assert!(
            catalog["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|model| model["slug"] == "deepseek-chat")
        );
        assert!(
            catalog["models"]
                .as_array()
                .unwrap()
                .iter()
                .any(|model| model["slug"] == "deepseek-reasoner")
        );
        let efforts = catalog["models"][0]["supported_reasoning_levels"]
            .as_array()
            .expect("reasoning levels should be present")
            .iter()
            .map(|level| level["effort"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(efforts, vec!["low", "medium", "high", "xhigh"]);

        let _ = fs::remove_dir_all(codex_home);
    }

    #[test]
    fn deepseek_provider_codex_args_respects_existing_catalog_override() {
        let codex_home = temp_codex_home("existing");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-deepseek\""),
            OsString::from("-c"),
            OsString::from("model_catalog_json=\"/tmp/custom.json\""),
        ];

        let args = prepare_deepseek_provider_codex_args(&codex_home, &user_args)
            .expect("DeepSeek args should prepare");

        assert_eq!(args, user_args);
        assert!(!codex_home.join(DEEPSEEK_MODEL_CATALOG_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn deepseek_catalog_write_replaces_symlink_without_touching_target() {
        let codex_home = temp_codex_home("catalog-symlink");
        fs::create_dir_all(&codex_home).unwrap();
        let target = codex_home.join("target.json");
        let catalog_path = codex_home.join(DEEPSEEK_MODEL_CATALOG_FILE);
        fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &catalog_path).unwrap();

        write_deepseek_model_catalog(&codex_home, "deepseek-v4-pro", 100_000, 90_000).unwrap();

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
    fn deepseek_provider_codex_args_rejects_invalid_numeric_catalog_overrides() {
        for (value, message) in [
            ("", "DeepSeek model_context_window cannot be empty"),
            (
                " 128000 ",
                "DeepSeek model_context_window must not contain whitespace",
            ),
            (
                "not-a-number",
                "DeepSeek model_context_window must be an unsigned integer",
            ),
            ("1", "DeepSeek model_context_window must be greater than 1"),
        ] {
            let codex_home = temp_codex_home("invalid-numeric");
            let user_args = vec![
                OsString::from("-c"),
                OsString::from("model_provider=\"prodex-deepseek\""),
                OsString::from("-c"),
                OsString::from(format!(
                    "model_context_window={}",
                    toml_string_literal(value)
                )),
            ];

            let err = prepare_deepseek_provider_codex_args(&codex_home, &user_args).unwrap_err();

            assert!(err.to_string().contains(message));
            let _ = fs::remove_dir_all(codex_home);
        }
    }
}
