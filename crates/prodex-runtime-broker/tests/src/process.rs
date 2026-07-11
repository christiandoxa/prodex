use std::ffi::OsString;
use std::path::PathBuf;

use super::*;

#[test]
fn broker_process_args_encode_optional_boolean_switches() {
    let config = RuntimeBrokerSpawnConfig {
        current_profile: "default",
        upstream_base_url: "https://upstream.example",
        include_code_review: true,
        upstream_no_proxy: true,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        broker_key: "key",
        instance_token: "broker-token",
        admin_token: "admin-token",
        listen_addr: Some("127.0.0.1:4567"),
    };
    let args = runtime_broker_process_args(config);

    assert_eq!(args[0], OsString::from("__runtime-broker"));
    assert!(args.contains(&OsString::from("--include-code-review")));
    assert!(args.contains(&OsString::from("--upstream-no-proxy")));
    assert!(args.contains(&OsString::from("--smart-context")));
    assert!(args.contains(&OsString::from("--model-context-window-tokens")));
    assert!(args.contains(&OsString::from("65536")));
    assert!(args.contains(&OsString::from("--listen-addr")));
    assert!(args.contains(&OsString::from("127.0.0.1:4567")));

    let plan = runtime_broker_process_command_plan("/bin/prodex", "/tmp/prodex-home", config);

    assert_eq!(plan.executable, PathBuf::from("/bin/prodex"));
    assert_eq!(plan.prodex_home, PathBuf::from("/tmp/prodex-home"));
    assert!(plan.args.contains(&OsString::from("--instance-token")));

    let disabled_args = runtime_broker_process_args(RuntimeBrokerSpawnConfig {
        smart_context_enabled: false,
        listen_addr: None,
        ..config
    });
    assert!(!disabled_args.contains(&OsString::from("--smart-context")));
    assert!(!disabled_args.contains(&OsString::from("--model-context-window-tokens")));
}

#[test]
fn broker_key_is_scoped_to_smart_context_mode() {
    let normal = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        false,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );
    let smart = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );

    assert_ne!(normal, smart);
}

#[test]
fn broker_key_is_scoped_to_smart_context_window_when_enabled() {
    let default_window = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );
    let custom_window = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        Some(65_536),
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );

    assert_ne!(default_window, custom_window);
}
