use super::*;

#[test]
fn development_postgres_defaults_to_explicit_plaintext() {
    let root = temp_dir("gateway-postgres-tls-development");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _postgres = TestEnvVarGuard::set(
        "PRODEX_GATEWAY_POSTGRES_URL_TEST",
        "postgres://prodex:prodex@127.0.0.1:5432/prodex",
    );
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.state.backend = Some("postgres".to_string());
    policy.state.postgres_url_env = Some("PRODEX_GATEWAY_POSTGRES_URL_TEST".to_string());
    let RuntimeGatewayStateStore::Postgres { tls, .. } =
        gateway_state_store_config(&paths, &policy).unwrap()
    else {
        panic!("expected postgres state store");
    };
    assert_eq!(
        tls.mode(),
        prodex_storage_postgres_runtime::PostgresTlsMode::Disable
    );
}

#[test]
fn gateway_state_store_config_resolves_verify_full_ca_under_prodex_home() {
    let root = temp_dir("gateway-postgres-tls-config");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _postgres = TestEnvVarGuard::set(
        "PRODEX_GATEWAY_POSTGRES_URL_TEST",
        "postgres://prodex:prodex@db.internal:5432/prodex",
    );
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.state.backend = Some("postgres".to_string());
    policy.state.postgres_url_env = Some("PRODEX_GATEWAY_POSTGRES_URL_TEST".to_string());
    policy.state.postgres_tls_mode = Some("verify-full".to_string());
    policy.state.postgres_tls_ca_path = Some("certs/postgres-ca.pem".to_string());

    let store = gateway_state_store_config(&paths, &policy).unwrap();
    let RuntimeGatewayStateStore::Postgres { tls, .. } = store else {
        panic!("expected postgres state store");
    };
    assert_eq!(
        tls.mode(),
        prodex_storage_postgres_runtime::PostgresTlsMode::VerifyFull
    );
    assert_eq!(
        tls.ca_path(),
        Some(root.join("certs/postgres-ca.pem").as_path())
    );
}
