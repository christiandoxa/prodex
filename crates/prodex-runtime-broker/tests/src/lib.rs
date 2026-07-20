use super::*;

mod continuity;
mod metrics;
mod process;
mod registry;
mod version_guard;

fn test_registry() -> RuntimeBrokerRegistry {
    RuntimeBrokerRegistry {
        pid: 42,
        listen_addr: "127.0.0.1:4567".to_string(),
        started_at: 100,
        upstream_base_url: "https://upstream.example".to_string(),
        include_code_review: true,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        current_profile: "work".to_string(),
        instance_id: "broker-token".to_string(),
        prodex_version: Some("0.7.0".to_string()),
        executable_path: Some("/tmp/prodex".to_string()),
        executable_sha256: Some("abc123".to_string()),
        openai_mount_path: Some("/backend-api/prodex".to_string()),
        realtime_ws_addr: None,
    }
}
