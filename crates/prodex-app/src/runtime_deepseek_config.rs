use crate::{codex_cli_config_override_value, codex_config_value};
use anyhow::{Context, Result};
use prodex_cli::{
    SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW,
    SUPER_DEEPSEEK_DEFAULT_MODEL, SUPER_DEEPSEEK_PROVIDER_ID,
};
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const DEEPSEEK_MODEL_CATALOG_FILE: &str = "prodex-deepseek-model-catalog.json";
const DEEPSEEK_BASE_INSTRUCTIONS: &str = r#"You are Codex, a coding agent. You and the user share the same workspace.

Focus on the user's software task. Inspect the codebase before changing behavior, make narrow edits, preserve user changes, and verify with relevant tests or commands when feasible.

Use tools deliberately. For shell work, prefer fast focused commands. For file edits, keep changes minimal and explain non-obvious logic in short comments only when useful."#;

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
    if !deepseek_provider_enabled(codex_home, user_args) {
        return Ok(user_args.to_vec());
    }
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }

    let model = deepseek_model_for_launch(codex_home, user_args);
    let context_window = deepseek_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW as u64,
    );
    let auto_compact_token_limit = deepseek_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT as u64,
    )
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

fn deepseek_provider_enabled(codex_home: &Path, user_args: &[OsString]) -> bool {
    codex_cli_config_override_value(user_args, "model_provider")
        .or_else(|| codex_config_value(codex_home, "model_provider"))
        .is_some_and(|provider| provider.eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID))
}

fn deepseek_model_for_launch(codex_home: &Path, user_args: &[OsString]) -> String {
    codex_cli_config_override_value(user_args, "model")
        .or_else(|| codex_config_value(codex_home, "model"))
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| SUPER_DEEPSEEK_DEFAULT_MODEL.to_string())
}

fn deepseek_u64_config_for_launch(
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
        "models": [
            {
                "slug": model,
                "display_name": model,
                "description": "DeepSeek model routed through the Prodex Responses adapter.",
                "default_reasoning_level": "high",
                "supported_reasoning_levels": [
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
                "priority": 1,
                "additional_speed_tiers": [],
                "service_tiers": [],
                "default_service_tier": null,
                "availability_nux": null,
                "upgrade": null,
                "base_instructions": DEEPSEEK_BASE_INSTRUCTIONS,
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
            }
        ]
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize DeepSeek catalog")?;
    fs::write(&catalog_path, contents)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(catalog_path)
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
        assert_eq!(catalog["models"].as_array().unwrap().len(), 1);
        assert_eq!(catalog["models"][0]["slug"], "deepseek-v4-pro");
        assert_eq!(
            catalog["models"][0]["supported_reasoning_levels"][1]["effort"],
            "xhigh"
        );

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
}
