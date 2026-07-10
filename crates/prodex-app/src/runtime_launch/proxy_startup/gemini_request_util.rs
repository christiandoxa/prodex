use prodex_provider_core::{gemini_provider_core_bool_str, gemini_provider_core_bool_value};
use std::env;
use std::path::PathBuf;

pub(in super::super::super) fn runtime_gemini_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

pub(in super::super::super) fn runtime_gemini_config_dir() -> Option<PathBuf> {
    crate::gemini_cli_config_home_for(runtime_gemini_home_dir().as_deref())
}

pub(super) fn runtime_gemini_env_bool(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .and_then(|value| gemini_provider_core_bool_str(&value))
}

pub(super) fn runtime_gemini_bool_value(value: &serde_json::Value) -> Option<bool> {
    gemini_provider_core_bool_value(value)
}
