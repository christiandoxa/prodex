use crate::{RuntimeDeepSeekWebSearchMode, codex_config_value};
use std::env;
use std::fs;
use std::path::Path;

pub(crate) fn runtime_deepseek_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("DEEPSEEK_API_KEYS")
                .ok()
                .and_then(|value| runtime_provider_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("DEEPSEEK_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
}

pub(crate) fn runtime_deepseek_strict_tools_enabled(codex_home: &Path) -> bool {
    runtime_deepseek_toml_config_value(codex_home, "deepseek.strict_tools")
        .and_then(|value| match value {
            toml::Value::Boolean(enabled) => Some(enabled),
            toml::Value::String(value) => Some(runtime_deepseek_config_bool(&value)),
            _ => None,
        })
        .or_else(|| {
            env::var("PRODEX_DEEPSEEK_STRICT_TOOLS")
                .ok()
                .map(|value| runtime_deepseek_config_bool(&value))
        })
        .unwrap_or(false)
}

pub(crate) fn runtime_deepseek_beta_base_url(codex_home: &Path) -> String {
    codex_config_value(codex_home, "deepseek.beta_base_url")
        .or_else(|| env::var("PRODEX_DEEPSEEK_BETA_BASE_URL").ok())
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.deepseek.com/beta".to_string())
}

pub(crate) fn runtime_deepseek_web_search_mode(codex_home: &Path) -> RuntimeDeepSeekWebSearchMode {
    codex_config_value(codex_home, "deepseek.web_search_mode")
        .or_else(|| env::var("PRODEX_DEEPSEEK_WEB_SEARCH_MODE").ok())
        .and_then(|value| runtime_deepseek_parse_web_search_mode(&value))
        .unwrap_or_default()
}

pub(crate) fn runtime_anthropic_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("ANTHROPIC_API_KEYS")
                .ok()
                .and_then(|value| runtime_provider_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("ANTHROPIC_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
}

pub(crate) fn runtime_copilot_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("GITHUB_COPILOT_API_KEYS")
                .ok()
                .and_then(|value| runtime_provider_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("GITHUB_COPILOT_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
}

pub(crate) fn runtime_gemini_api_keys_from_request_or_env(
    value: Option<&str>,
) -> Option<Vec<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            ["GEMINI_API_KEYS", "GOOGLE_API_KEYS"]
                .into_iter()
                .find_map(|key| {
                    env::var(key)
                        .ok()
                        .and_then(|value| runtime_provider_api_keys_from_list(&value))
                })
                .or_else(|| {
                    ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
                        .into_iter()
                        .find_map(|key| {
                            env::var(key)
                                .ok()
                                .map(|value| value.trim().to_string())
                                .filter(|value| !value.is_empty())
                                .map(|value| vec![value])
                        })
                })
        })
}

fn runtime_deepseek_parse_web_search_mode(value: &str) -> Option<RuntimeDeepSeekWebSearchMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "auto" => Some(RuntimeDeepSeekWebSearchMode::Auto),
        "off" | "disabled" | "disable" => Some(RuntimeDeepSeekWebSearchMode::Off),
        "openai_chat" | "openai-chat" | "chat" => Some(RuntimeDeepSeekWebSearchMode::OpenAiChat),
        "anthropic" => Some(RuntimeDeepSeekWebSearchMode::Anthropic),
        "function_proxy" | "function-proxy" => Some(RuntimeDeepSeekWebSearchMode::FunctionProxy),
        _ => None,
    }
}

fn runtime_deepseek_config_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
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

        let keys = runtime_gemini_api_keys_from_request_or_env(None).unwrap();

        assert_eq!(keys, vec!["one", "two"]);
    }

    #[test]
    fn deepseek_strict_tools_reads_env_fallback() {
        let _strict = TestEnvVarGuard::set("PRODEX_DEEPSEEK_STRICT_TOOLS", "true");
        let _beta = TestEnvVarGuard::set(
            "PRODEX_DEEPSEEK_BETA_BASE_URL",
            "https://example.test/beta/",
        );
        let _search = TestEnvVarGuard::set("PRODEX_DEEPSEEK_WEB_SEARCH_MODE", "off");

        assert!(runtime_deepseek_strict_tools_enabled(Path::new("")));
        assert_eq!(
            runtime_deepseek_beta_base_url(Path::new("")),
            "https://example.test/beta"
        );
        assert_eq!(
            runtime_deepseek_web_search_mode(Path::new("")),
            RuntimeDeepSeekWebSearchMode::Off
        );
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

        assert!(runtime_deepseek_strict_tools_enabled(&root));
        assert_eq!(
            runtime_deepseek_web_search_mode(&root),
            RuntimeDeepSeekWebSearchMode::FunctionProxy
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copilot_api_key_list_reads_plural_env() {
        let _copilot_keys = TestEnvVarGuard::set("GITHUB_COPILOT_API_KEYS", "one;two");
        let _copilot_key = TestEnvVarGuard::unset("GITHUB_COPILOT_API_KEY");

        let keys = runtime_copilot_api_keys_from_request_or_env(None).unwrap();

        assert_eq!(keys, vec!["one", "two"]);
    }
}
