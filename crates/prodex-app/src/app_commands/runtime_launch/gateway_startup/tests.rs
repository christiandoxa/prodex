use super::*;
use crate::TestEnvVarGuard;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn gateway_args() -> GatewayArgs {
    GatewayArgs {
        command: None,
        listen: None,
        provider: None,
        harness: None,
        base_url: None,
        api_key: None,
        auth_token: None,
        smart_context: false,
        presidio: false,
        no_presidio: false,
    }
}

#[test]
fn gateway_refresh_args_retain_explicit_harness_selection() {
    let mut args = gateway_args();
    args.harness = Some(prodex_provider_core::HarnessMode::Minimal);

    let refreshed = gateway_refresh_args(&args);

    assert_eq!(refreshed.harness, args.harness);
}

#[test]
fn gateway_mode_mismatch_fails_before_secret_resolution_or_bind() {
    let root = temp_root("gateway-service-mode-mismatch");
    std::fs::write(
        root.join(prodex_runtime_policy::PRODEX_POLICY_FILE_NAME),
        r#"
version = 1
service_mode = "control-plane"

[secrets]
production = true
projected_root = "projected"
projected_provider = "external"

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "external", name = "postgres-url" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "external", name = "admin-token" }
role = "admin"
"#,
    )
    .unwrap();
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let mut args = gateway_args();
    args.listen = Some("not-a-listener-address".to_string());
    args.api_key = Some("must-not-be-read".to_string());

    let error = start_gateway_backend(args).unwrap_err().to_string();

    assert!(error.contains("service_mode=control-plane"), "{error}");
    assert!(!error.contains("secret"), "{error}");
    assert!(!error.contains("bind"), "{error}");
}

#[test]
fn runtime_config_failure_precedes_secret_resolution_and_bind() {
    let root = temp_root("gateway-runtime-config-pre-bind");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _replicas = TestEnvVarGuard::set("PRODEX_GATEWAY_REPLICA_COUNT", "0");
    let _gate = TestEnvVarGuard::set(
        "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS",
        "sometimes",
    );
    let mut args = gateway_args();
    args.listen = Some("not-a-listener-address".to_string());
    args.api_key = Some(String::new());

    let error = start_gateway_backend(args).unwrap_err().to_string();

    assert!(error.contains("PRODEX_GATEWAY_REPLICA_COUNT must be at least 1"));
    assert!(error.contains(
        "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS must be one of true,false,1,0,yes,no,on,off"
    ));
    assert!(!error.contains("gateway provider API key"));
    assert!(!error.contains("bind"));
}

#[test]
fn workers_reuse_the_pre_start_runtime_config_snapshot() {
    let root = temp_root("gateway-runtime-config-snapshot");
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _home = TestEnvVarGuard::set("HOME", "/tmp/gateway-home-before-start");
    let _gemini_home = TestEnvVarGuard::set("GEMINI_CLI_HOME", "/tmp/gateway-gemini-before-start");
    let _system = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
        "/tmp/gateway-system-before-start/settings.json",
    );
    let _defaults = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
        "/tmp/gateway-system-before-start/defaults.json",
    );
    let _replicas = TestEnvVarGuard::set("PRODEX_GATEWAY_REPLICA_COUNT", "2");
    let _gate = TestEnvVarGuard::set("PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS", "false");
    let mut args = gateway_args();
    args.listen = Some("127.0.0.1:0".to_string());
    let backend = start_gateway_backend(args).unwrap();

    let _changed_home = TestEnvVarGuard::set("HOME", "/tmp/gateway-home-after-start");
    let _changed_gemini_home =
        TestEnvVarGuard::set("GEMINI_CLI_HOME", "/tmp/gateway-gemini-after-start");
    let _changed_system = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
        "/tmp/gateway-system-after-start/settings.json",
    );
    let _changed_defaults = TestEnvVarGuard::set(
        "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
        "/tmp/gateway-system-after-start/defaults.json",
    );
    let _changed_replicas = TestEnvVarGuard::set("PRODEX_GATEWAY_REPLICA_COUNT", "7");
    let _changed_gate =
        TestEnvVarGuard::set("PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS", "true");

    let config = backend.runtime_config();
    assert_eq!(config.gateway.replica_count, 2);
    assert!(!config.gateway.require_multi_replica_accounting_checks);
    assert_eq!(
        config.gemini.home_dir.as_deref(),
        Some(std::path::Path::new("/tmp/gateway-home-before-start"))
    );
    assert_eq!(
        config.gemini.config_dir.as_deref(),
        Some(std::path::Path::new(
            "/tmp/gateway-gemini-before-start/.gemini"
        ))
    );
    assert_eq!(
        config.gemini.system_settings_path.as_deref(),
        Some(std::path::Path::new(
            "/tmp/gateway-system-before-start/settings.json"
        ))
    );
    assert_eq!(
        config.gemini.system_defaults_path.as_deref(),
        Some(std::path::Path::new(
            "/tmp/gateway-system-before-start/defaults.json"
        ))
    );
}
