pub fn format_runtime_policy_summary(path: Option<&str>, version: Option<u32>) -> String {
    path.zip(version)
        .map(|(path, version)| format!("{path} (v{version})"))
        .unwrap_or_else(|| "not loaded".to_string())
}

pub fn format_runtime_proxy_contract_summary() -> String {
    "bounded retry, quota/transport separation, structured observability, profile-isolated secrets"
        .to_string()
}

pub fn runtime_policy_json_value(path: Option<&str>, version: Option<u32>) -> serde_json::Value {
    path.zip(version)
        .map(|(path, version)| serde_json::json!({"path": path, "version": version}))
        .unwrap_or(serde_json::Value::Null)
}

pub fn format_runtime_logs_summary(directory: &str, format: &str) -> String {
    format!("{directory} ({format})")
}

pub fn runtime_logs_json_value(directory: &str, format: &str) -> serde_json::Value {
    serde_json::json!({"directory": directory, "format": format})
}

pub fn format_secret_backend_summary_parts(
    backend: Option<&str>,
    keyring_service: Option<&str>,
    error: Option<&str>,
) -> String {
    if let Some(error) = error {
        return format!("invalid ({error})");
    }
    match (backend, keyring_service) {
        (Some(backend), Some(service)) => format!("{backend} ({service})"),
        (Some(backend), None) => backend.to_string(),
        (None, _) => "invalid (missing backend)".to_string(),
    }
}

pub fn secret_backend_json_value_parts(
    backend: Option<&str>,
    keyring_service: Option<&str>,
    error: Option<&str>,
) -> serde_json::Value {
    if let Some(error) = error {
        return serde_json::json!({"invalid": true, "error": error});
    }
    serde_json::json!({"backend": backend, "keyring_service": keyring_service})
}
