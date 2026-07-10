use crate::{codex_cli_config_override_value, codex_config_value};
use anyhow::{Context, Result};
use prodex_cli::{
    SUPER_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_DEFAULT_CONTEXT_WINDOW, SUPER_DEFAULT_LOCAL_MODEL,
    SUPER_LOCAL_PROVIDER_ID,
};
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

const LOCAL_MODEL_CATALOG_FILE: &str = "prodex-local-model-catalog.json";

pub(crate) fn prepare_local_provider_catalog_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    local_provider_catalog_codex_args(codex_home, user_args, true)
}

pub(crate) fn preview_local_provider_catalog_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    local_provider_catalog_codex_args(codex_home, user_args, false)
}

pub(crate) fn prepare_provider_capability_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
) -> Result<Vec<OsString>> {
    let args = prepare_local_provider_catalog_codex_args(codex_home, user_args)?;
    let args = crate::prepare_external_provider_catalog_codex_args(codex_home, &args)?;
    let args = crate::prepare_deepseek_provider_codex_args(codex_home, &args)?;
    crate::prepare_gemini_provider_codex_args(codex_home, &args)
}

fn local_provider_catalog_codex_args(
    codex_home: &Path,
    user_args: &[OsString],
    write_catalog: bool,
) -> Result<Vec<OsString>> {
    if !local_provider_enabled(codex_home, user_args) {
        return Ok(user_args.to_vec());
    }
    if codex_cli_config_override_value(user_args, "model_catalog_json").is_some() {
        return Ok(user_args.to_vec());
    }

    let model = local_model_for_launch(codex_home, user_args);
    let context_window = local_u64_config_for_launch(
        codex_home,
        user_args,
        "model_context_window",
        SUPER_DEFAULT_CONTEXT_WINDOW as u64,
    );
    let auto_compact_token_limit = local_u64_config_for_launch(
        codex_home,
        user_args,
        "model_auto_compact_token_limit",
        SUPER_DEFAULT_AUTO_COMPACT_LIMIT as u64,
    )
    .min(context_window.saturating_sub(1));
    let catalog_path = codex_home.join(LOCAL_MODEL_CATALOG_FILE);
    if write_catalog {
        write_local_model_catalog(
            &catalog_path,
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

fn local_provider_enabled(codex_home: &Path, user_args: &[OsString]) -> bool {
    codex_cli_config_override_value(user_args, "model_provider")
        .or_else(|| codex_config_value(codex_home, "model_provider"))
        .is_some_and(|provider| provider.eq_ignore_ascii_case(SUPER_LOCAL_PROVIDER_ID))
}

fn local_model_for_launch(codex_home: &Path, user_args: &[OsString]) -> String {
    codex_cli_config_override_value(user_args, "model")
        .or_else(|| codex_config_value(codex_home, "model"))
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| SUPER_DEFAULT_LOCAL_MODEL.to_string())
}

fn local_u64_config_for_launch(
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

fn write_local_model_catalog(
    catalog_path: &Path,
    model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<()> {
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let catalog = json!({
        "models": [local_catalog_model(model, context_window, auto_compact_token_limit)]
    });
    let contents =
        serde_json::to_string_pretty(&catalog).context("failed to serialize local catalog")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(catalog_path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", catalog_path.display()))
}

fn local_catalog_model(
    slug: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> serde_json::Value {
    json!({
        "slug": slug,
        "display_name": slug,
        "description": "Local OpenAI-compatible model routed through Prodex.",
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
        "priority": 1,
        "additional_speed_tiers": [],
        "service_tiers": [],
        "default_service_tier": null,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": null,
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
        "supports_search_tool": true
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
        std::env::temp_dir().join(format!("prodex-local-provider-config-{name}-{stamp}"))
    }

    fn catalog_path_from_args(args: &[OsString]) -> PathBuf {
        let value = args
            .windows(2)
            .find_map(|window| {
                (window[0] == "-c").then(|| window[1].to_string_lossy().into_owned())
            })
            .and_then(|arg| arg.strip_prefix("model_catalog_json=").map(str::to_string))
            .expect("model_catalog_json override should be present");
        PathBuf::from(value.trim_matches('"'))
    }

    fn catalog_efforts(path: &Path) -> Vec<String> {
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        catalog["models"][0]["supported_reasoning_levels"]
            .as_array()
            .expect("reasoning levels should be present")
            .iter()
            .map(|level| level["effort"].as_str().unwrap().to_string())
            .collect()
    }

    fn assert_contains_standard_efforts(efforts: &[String]) {
        for expected in ["low", "medium", "high", "xhigh"] {
            assert!(
                efforts.iter().any(|effort| effort == expected),
                "missing {expected} in {efforts:?}"
            );
        }
    }

    #[test]
    fn local_provider_catalog_args_write_tool_capable_catalog() {
        let codex_home = temp_codex_home("catalog");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-local\""),
            OsString::from("-c"),
            OsString::from("model=\"local/qwen\""),
        ];

        let args = prepare_local_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("local catalog should prepare");
        assert_eq!(args[0], OsString::from("-c"));
        assert!(args[1].to_string_lossy().starts_with("model_catalog_json="));

        let catalog_path = codex_home.join(LOCAL_MODEL_CATALOG_FILE);
        let catalog: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&catalog_path).unwrap()).unwrap();
        assert_eq!(catalog["models"][0]["slug"], "local/qwen");
        assert_eq!(catalog["models"][0]["apply_patch_tool_type"], "freeform");
        assert_eq!(catalog["models"][0]["supports_search_tool"], true);
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
    fn provider_capability_catalogs_expose_standard_reasoning_efforts() {
        for (provider_id, model) in [
            ("prodex-local", "local/qwen"),
            ("prodex-copilot", "gpt-5.5"),
            ("prodex-anthropic", "claude-sonnet-4-6"),
            ("prodex-deepseek", "deepseek-v4-pro"),
            ("prodex-gemini", "gemini-2.5-pro"),
        ] {
            let codex_home = temp_codex_home(provider_id);
            let user_args = vec![
                OsString::from("-c"),
                OsString::from(format!("model_provider=\"{provider_id}\"")),
                OsString::from("-c"),
                OsString::from(format!("model=\"{model}\"")),
            ];

            let args = prepare_provider_capability_codex_args(&codex_home, &user_args)
                .expect("provider catalog should prepare");
            let catalog_path = catalog_path_from_args(&args);
            assert_contains_standard_efforts(&catalog_efforts(&catalog_path));

            let _ = fs::remove_dir_all(codex_home);
        }
    }

    #[test]
    fn local_provider_catalog_args_respects_existing_catalog_override() {
        let codex_home = temp_codex_home("existing");
        let user_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-local\""),
            OsString::from("-c"),
            OsString::from("model_catalog_json=\"/tmp/custom.json\""),
        ];

        let args = prepare_local_provider_catalog_codex_args(&codex_home, &user_args)
            .expect("local catalog should prepare");

        assert_eq!(args, user_args);
        assert!(!codex_home.join(LOCAL_MODEL_CATALOG_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn local_catalog_write_replaces_symlink_without_touching_target() {
        let codex_home = temp_codex_home("catalog-symlink");
        fs::create_dir_all(&codex_home).unwrap();
        let target = codex_home.join("target.json");
        let catalog_path = codex_home.join(LOCAL_MODEL_CATALOG_FILE);
        fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &catalog_path).unwrap();

        write_local_model_catalog(&catalog_path, "local/qwen", 100_000, 90_000).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "do not touch");
        assert!(
            !fs::symlink_metadata(&catalog_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let _ = fs::remove_dir_all(codex_home);
    }
}
