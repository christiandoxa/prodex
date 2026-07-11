use crate::{RuntimeDeepSeekWebSearchMode, codex_config_value};
use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::Path;

pub(crate) fn runtime_deepseek_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Result<Option<Vec<String>>> {
    runtime_provider_api_keys_from_request_or_env(
        value,
        "--api-key",
        &["DEEPSEEK_API_KEYS"],
        &["DEEPSEEK_API_KEY"],
    )
}

pub(crate) fn runtime_deepseek_strict_tools_enabled(codex_home: &Path) -> Result<bool> {
    if let Some(value) = runtime_deepseek_toml_config_value(codex_home, "deepseek.strict_tools") {
        return match value {
            toml::Value::Boolean(enabled) => Ok(enabled),
            toml::Value::String(value) => {
                runtime_deepseek_parse_bool("deepseek.strict_tools", &value)
            }
            _ => bail!("deepseek.strict_tools must be a boolean"),
        };
    }
    let Ok(value) = env::var("PRODEX_DEEPSEEK_STRICT_TOOLS") else {
        return Ok(false);
    };
    runtime_deepseek_parse_bool("PRODEX_DEEPSEEK_STRICT_TOOLS", &value)
}

pub(crate) fn runtime_deepseek_beta_base_url(codex_home: &Path) -> Result<String> {
    let Some((name, value)) = codex_config_value(codex_home, "deepseek.beta_base_url")
        .map(|value| ("deepseek.beta_base_url", value))
        .or_else(|| {
            env::var("PRODEX_DEEPSEEK_BETA_BASE_URL")
                .ok()
                .map(|value| ("PRODEX_DEEPSEEK_BETA_BASE_URL", value))
        })
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
    let parsed = reqwest::Url::parse(&normalized)
        .with_context(|| format!("{name} must be an http(s) URL with host"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        bail!("{name} must be an http(s) URL with host");
    }
    Ok(normalized)
}

pub(crate) fn runtime_deepseek_web_search_mode(
    codex_home: &Path,
) -> Result<RuntimeDeepSeekWebSearchMode> {
    if let Some(value) = runtime_deepseek_toml_config_value(codex_home, "deepseek.web_search_mode")
    {
        let toml::Value::String(value) = value else {
            bail!("deepseek.web_search_mode must be a string");
        };
        return runtime_deepseek_parse_web_search_mode("deepseek.web_search_mode", &value);
    }
    let Ok(value) = env::var("PRODEX_DEEPSEEK_WEB_SEARCH_MODE") else {
        return Ok(RuntimeDeepSeekWebSearchMode::default());
    };
    runtime_deepseek_parse_web_search_mode("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", &value)
}

pub(crate) fn runtime_anthropic_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Result<Option<Vec<String>>> {
    runtime_provider_api_keys_from_request_or_env(
        value,
        "--api-key",
        &["ANTHROPIC_API_KEYS"],
        &["ANTHROPIC_API_KEY"],
    )
}

pub(crate) fn runtime_copilot_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Result<Option<Vec<String>>> {
    runtime_provider_api_keys_from_request_or_env(
        value,
        "--api-key",
        &["GITHUB_COPILOT_API_KEYS"],
        &["GITHUB_COPILOT_API_KEY"],
    )
}

pub(crate) fn runtime_gemini_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Result<Option<Vec<String>>> {
    runtime_provider_api_keys_from_request_or_env(
        value,
        "--api-key",
        &["GEMINI_API_KEYS", "GOOGLE_API_KEYS"],
        &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
    )
}

fn runtime_deepseek_parse_web_search_mode(
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

fn runtime_deepseek_parse_bool(name: &str, value: &str) -> Result<bool> {
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

fn runtime_deepseek_toml_config_value(codex_home: &Path, key: &str) -> Option<toml::Value> {
    let contents = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    let value = toml::from_str::<toml::Value>(&contents).ok()?;
    let mut current = &value;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

fn runtime_provider_api_keys_from_list(value: &str) -> Option<Vec<String>> {
    let keys = value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

fn runtime_provider_api_keys_from_request_or_env(
    value: Option<&str>,
    value_name: &str,
    plural_env_names: &[&str],
    single_env_names: &[&str],
) -> Result<Option<Vec<String>>> {
    if let Some(value) = value {
        return Ok(Some(vec![runtime_provider_single_api_key(
            value, value_name,
        )?]));
    }
    for env_name in plural_env_names {
        if let Ok(value) = env::var(env_name) {
            return Ok(Some(
                runtime_provider_api_keys_from_list(&value)
                    .with_context(|| format!("{env_name} cannot be empty"))?,
            ));
        }
    }
    for env_name in single_env_names {
        if let Ok(value) = env::var(env_name) {
            return Ok(Some(vec![runtime_provider_single_api_key(
                &value, env_name,
            )?]));
        }
    }
    Ok(None)
}

fn runtime_provider_single_api_key(value: &str, name: &str) -> Result<String> {
    if value.is_empty() {
        bail!("{name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{name} must not contain whitespace");
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestEnvVarGuard;

    #[test]
    fn provider_api_key_list_accepts_common_separators() {
        assert_eq!(
            runtime_provider_api_keys_from_list(" one, two;three\nfour ").unwrap(),
            vec!["one", "two", "three", "four"]
        );
        assert!(runtime_provider_api_keys_from_list(" , ; \n ").is_none());
    }

    #[test]
    fn gemini_api_key_list_reads_plural_env_before_single_env() {
        let _gemini_keys = TestEnvVarGuard::set("GEMINI_API_KEYS", "one,two");
        let _google_keys = TestEnvVarGuard::unset("GOOGLE_API_KEYS");
        let _gemini_key = TestEnvVarGuard::set("GEMINI_API_KEY", "single");
        let _google_key = TestEnvVarGuard::unset("GOOGLE_API_KEY");

        let keys = runtime_gemini_api_keys_from_request_or_env(None)
            .unwrap()
            .unwrap();

        assert_eq!(keys, vec!["one", "two"]);
    }

    #[test]
    fn provider_api_key_resolvers_reject_empty_explicit_inputs() {
        let err = runtime_anthropic_api_keys_from_request_or_env(Some("")).unwrap_err();
        assert!(err.to_string().contains("--api-key cannot be empty"));

        let err = runtime_anthropic_api_keys_from_request_or_env(Some(" sk-ant ")).unwrap_err();
        assert!(
            err.to_string()
                .contains("--api-key must not contain whitespace")
        );

        {
            let _keys = TestEnvVarGuard::set("DEEPSEEK_API_KEYS", " , ; \n ");
            let _single = TestEnvVarGuard::set("DEEPSEEK_API_KEY", "deepseek-valid");
            let err = runtime_deepseek_api_keys_from_request_or_env(None).unwrap_err();
            assert!(
                err.to_string()
                    .contains("DEEPSEEK_API_KEYS cannot be empty")
            );
        }

        {
            let _keys = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEYS");
            let _single = TestEnvVarGuard::set("GITHUB_COPILOT_API_KEY", "");
            let err = runtime_copilot_api_keys_from_request_or_env(None).unwrap_err();
            assert!(
                err.to_string()
                    .contains("GITHUB_COPILOT_API_KEY cannot be empty")
            );
        }

        {
            let _keys = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEYS");
            let _single = TestEnvVarGuard::set("GITHUB_COPILOT_API_KEY", " copilot-token ");
            let err = runtime_copilot_api_keys_from_request_or_env(None).unwrap_err();
            assert!(
                err.to_string()
                    .contains("GITHUB_COPILOT_API_KEY must not contain whitespace")
            );
        }

        let _gemini_keys = TestEnvVarGuard::set("GEMINI_API_KEYS", " , ");
        let _google_keys = TestEnvVarGuard::set("GOOGLE_API_KEYS", "valid-google");
        let err = runtime_gemini_api_keys_from_request_or_env(None).unwrap_err();
        assert!(err.to_string().contains("GEMINI_API_KEYS cannot be empty"));
    }

    #[test]
    fn deepseek_strict_tools_reads_env_fallback() {
        let _strict = TestEnvVarGuard::set("PRODEX_DEEPSEEK_STRICT_TOOLS", "true");
        let _beta = TestEnvVarGuard::set(
            "PRODEX_DEEPSEEK_BETA_BASE_URL",
            "https://example.test/beta/",
        );
        let _search = TestEnvVarGuard::set("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", "off");

        assert!(runtime_deepseek_strict_tools_enabled(Path::new("")).unwrap());
        assert_eq!(
            runtime_deepseek_beta_base_url(Path::new("")).unwrap(),
            "https://example.test/beta"
        );
        assert_eq!(
            runtime_deepseek_web_search_mode(Path::new("")).unwrap(),
            RuntimeDeepSeekWebSearchMode::Off
        );
    }

    #[test]
    fn deepseek_beta_base_url_rejects_empty_and_padded_values() {
        {
            let _beta = TestEnvVarGuard::set("PRODEX_DEEPSEEK_BETA_BASE_URL", "");
            let err = runtime_deepseek_beta_base_url(Path::new("")).unwrap_err();
            assert!(
                err.to_string()
                    .contains("PRODEX_DEEPSEEK_BETA_BASE_URL cannot be empty")
            );
        }

        let _beta = TestEnvVarGuard::set(
            "PRODEX_DEEPSEEK_BETA_BASE_URL",
            " https://api.deepseek.com/beta ",
        );
        let err = runtime_deepseek_beta_base_url(Path::new("")).unwrap_err();
        assert!(
            err.to_string()
                .contains("PRODEX_DEEPSEEK_BETA_BASE_URL must not contain whitespace")
        );
    }

    #[test]
    fn deepseek_strict_tools_rejects_invalid_values() {
        for (value, message) in [
            ("", "PRODEX_DEEPSEEK_STRICT_TOOLS cannot be empty"),
            (
                " true ",
                "PRODEX_DEEPSEEK_STRICT_TOOLS must not contain whitespace",
            ),
            (
                "maybe",
                "PRODEX_DEEPSEEK_STRICT_TOOLS must be true or false",
            ),
        ] {
            let _strict = TestEnvVarGuard::set("PRODEX_DEEPSEEK_STRICT_TOOLS", value);
            let err = runtime_deepseek_strict_tools_enabled(Path::new("")).unwrap_err();
            assert!(err.to_string().contains(message));
        }

        let root = env::temp_dir().join(format!(
            "prodex-deepseek-strict-tools-config-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "[deepseek]\nstrict_tools = []\n").unwrap();

        let _strict = TestEnvVarGuard::set("PRODEX_DEEPSEEK_STRICT_TOOLS", "true");
        let err = runtime_deepseek_strict_tools_enabled(&root).unwrap_err();
        assert!(
            err.to_string()
                .contains("deepseek.strict_tools must be a boolean")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deepseek_web_search_mode_rejects_invalid_values() {
        for (value, message) in [
            ("", "PRODEX_DEEPSEEK_WEB_SEARCH_MODE cannot be empty"),
            (
                " off ",
                "PRODEX_DEEPSEEK_WEB_SEARCH_MODE must not contain whitespace",
            ),
            (
                "enabled",
                "PRODEX_DEEPSEEK_WEB_SEARCH_MODE must be auto, off, openai_chat, anthropic, or function_proxy",
            ),
        ] {
            let _search = TestEnvVarGuard::set("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", value);
            let err = runtime_deepseek_web_search_mode(Path::new("")).unwrap_err();
            assert!(err.to_string().contains(message));
        }

        let root = env::temp_dir().join(format!(
            "prodex-deepseek-web-search-config-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "[deepseek]\nweb_search_mode = \"\"\n",
        )
        .unwrap();

        let _search = TestEnvVarGuard::set("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", "off");
        let err = runtime_deepseek_web_search_mode(&root).unwrap_err();
        assert!(
            err.to_string()
                .contains("deepseek.web_search_mode cannot be empty")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deepseek_strict_tools_reads_profile_config_bool() {
        let root = env::temp_dir().join(format!("prodex-deepseek-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "[deepseek]\nstrict_tools = true\nweb_search_mode = \"function_proxy\"\n",
        )
        .unwrap();

        assert!(runtime_deepseek_strict_tools_enabled(&root).unwrap());
        assert_eq!(
            runtime_deepseek_web_search_mode(&root).unwrap(),
            RuntimeDeepSeekWebSearchMode::FunctionProxy
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copilot_api_key_list_reads_plural_env() {
        let _copilot_keys = TestEnvVarGuard::set("GITHUB_COPILOT_API_KEYS", "one;two");
        let _copilot_key = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEY");

        let keys = runtime_copilot_api_keys_from_request_or_env(None)
            .unwrap()
            .unwrap();

        assert_eq!(keys, vec!["one", "two"]);
    }
}
