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
        .and_then(|value| runtime_gemini_bool_str(&value))
}

pub(super) fn runtime_gemini_bool_value(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::Number(number) => Some(number.as_i64().unwrap_or_default() != 0),
        serde_json::Value::String(value) => runtime_gemini_bool_str(value),
        _ => None,
    }
}

fn runtime_gemini_bool_str(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
