use super::*;

#[test]
fn governance_defaults_preserve_personal_mode() {
    let policy = parse_policy("version = 1");
    assert_eq!(
        policy.governance.mode,
        crate::RuntimeGovernanceMode::Personal
    );
    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("personal governance defaults should remain compatible");
}

fn control_plane_policy() -> RuntimePolicyFile {
    parse_policy(
        r#"
version = 1
service_mode = "control-plane"
[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway.workload_identity]
enabled = true
required_scope = "control_plane"
mtls_required = true
mtls_ca_ref = { provider = "kubernetes", name = "WORKLOAD_CA" }
tls_identity_ref = { provider = "kubernetes", name = "CONTROL_PLANE_TLS" }

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "kubernetes", name = "PRODEX_CONTROL_PLANE_ADMIN_TOKEN" }
role = "admin"
"#,
    )
}

#[test]
fn service_mode_defaults_to_gateway_and_accepts_explicit_control_plane() {
    assert_eq!(
        parse_policy("version = 1").service_mode,
        RuntimePolicyServiceMode::Gateway
    );
    validate_runtime_policy_file(&control_plane_policy(), Path::new("policy.toml"))
        .expect("projected admin auth, mTLS, and shared state should satisfy control-plane mode");
}

#[test]
fn control_plane_accepts_only_transport_mtls_workload_identity() {
    let valid = control_plane_policy();
    validate_runtime_policy_file(&valid, Path::new("policy.toml"))
        .expect("control-plane transport mTLS should validate without workload OIDC");

    let mut missing_mtls = valid.clone();
    missing_mtls.gateway.workload_identity = Default::default();
    let error = validate_runtime_policy_file(&missing_mtls, Path::new("policy.toml"))
        .expect_err("control-plane service mode must require native mTLS");
    assert!(error.to_string().contains("native mTLS"), "{error:#}");

    let mut disabled_mtls = valid.clone();
    disabled_mtls.gateway.workload_identity.mtls_required = Some(false);
    disabled_mtls.gateway.workload_identity.mtls_ca_ref = None;
    disabled_mtls.gateway.workload_identity.tls_identity_ref = None;
    let error = validate_runtime_policy_file(&disabled_mtls, Path::new("policy.toml"))
        .expect_err("control-plane service mode must not allow mTLS opt-out");
    assert!(error.to_string().contains("native mTLS"), "{error:#}");

    let mut oidc_enabled = valid.clone();
    oidc_enabled.gateway.workload_identity.issuer =
        Some("https://workload.example.com".to_string());
    let error = validate_runtime_policy_file(&oidc_enabled, Path::new("policy.toml"))
        .expect_err("control-plane transport identity must not activate workload OIDC");
    assert!(
        error.to_string().contains("transport identity"),
        "{error:#}"
    );

    let mut data_plane_scope = valid;
    data_plane_scope.gateway.workload_identity.required_scope = Some("data_plane".to_string());
    let error = validate_runtime_policy_file(&data_plane_scope, Path::new("policy.toml"))
        .expect_err("control-plane transport identity must not accept data-plane scope");
    assert!(error.to_string().contains("control_plane"), "{error:#}");

    data_plane_scope.gateway.workload_identity.required_scope = None;
    let error = validate_runtime_policy_file(&data_plane_scope, Path::new("policy.toml"))
        .expect_err("control-plane transport identity must declare its scope explicitly");
    assert!(error.to_string().contains("control_plane"), "{error:#}");
}

fn complete_bank_control_plane_policy() -> RuntimePolicyFile {
    let mut policy = parse_policy(&complete_bank_policy());
    policy.service_mode = RuntimePolicyServiceMode::ControlPlane;
    policy.gateway.provider = None;
    policy.gateway.require_auth = None;
    policy.gateway.auth_token_ref = None;
    policy.gateway.provider_api_key_ref = None;
    policy.gateway.trusted_proxies.clear();
    policy.gateway.sso = Default::default();
    policy.gateway.observability = Default::default();
    policy.gateway.workload_identity.issuer = None;
    policy.gateway.workload_identity.audience = None;
    policy.gateway.workload_identity.required_scope = Some("control_plane".to_string());
    policy.gateway.admin_tokens = parse_policy(
        r#"
version = 1

[[gateway.admin_tokens]]
name = "maker"
token_ref = { provider = "kubernetes", name = "CONTROL_PLANE_MAKER" }
role = "admin"
tenant_id = "00000000-0000-7000-8000-000000000001"

[[gateway.admin_tokens]]
name = "checker"
token_ref = { provider = "kubernetes", name = "CONTROL_PLANE_CHECKER" }
role = "admin"
tenant_id = "00000000-0000-7000-8000-000000000001"
"#,
    )
    .gateway
    .admin_tokens;
    policy
}

#[test]
fn bank_control_plane_uses_transport_mtls_and_projected_maker_checker() {
    let valid = complete_bank_control_plane_policy();
    validate_runtime_policy_file(&valid, Path::new("policy.toml"))
        .expect("bank control-plane should compose without data-plane SSO or SIEM");

    let mut missing_transport_mtls = valid.clone();
    missing_transport_mtls.gateway.workload_identity = Default::default();
    let error = validate_runtime_policy_file(&missing_transport_mtls, Path::new("policy.toml"))
        .expect_err("bank control-plane without transport mTLS must fail closed");
    assert!(
        error
            .to_string()
            .contains("control_plane workload identity"),
        "{error:#}"
    );

    let mut single_admin = valid.clone();
    single_admin.gateway.admin_tokens.pop();
    let error = validate_runtime_policy_file(&single_admin, Path::new("policy.toml"))
        .expect_err("bank control-plane without a checker must fail closed");
    assert!(error.to_string().contains("maker-checker"), "{error:#}");

    let mut duplicate_name = valid.clone();
    duplicate_name.gateway.admin_tokens[1].name = "maker".to_string();
    let mut duplicate_reference = valid.clone();
    duplicate_reference.gateway.admin_tokens[1].token_ref =
        duplicate_reference.gateway.admin_tokens[0]
            .token_ref
            .clone();
    for candidate in [duplicate_name, duplicate_reference] {
        let error = validate_runtime_policy_file(&candidate, Path::new("policy.toml"))
            .expect_err("bank control-plane maker and checker must be distinct");
        assert!(error.to_string().contains("maker-checker"), "{error:#}");
    }

    let mut uncovered_tenant = valid.clone();
    let mut observer = uncovered_tenant.gateway.admin_tokens[0].clone();
    observer.name = "observer".to_string();
    observer.role = Some("viewer".to_string());
    observer.token_ref = Some(prodex_domain::SecretRef::new(
        "kubernetes",
        "CONTROL_PLANE_OBSERVER",
        None::<String>,
    ));
    observer.tenant_id = Some("00000000-0000-7000-8000-000000000002".to_string());
    uncovered_tenant.gateway.admin_tokens.push(observer);
    let error = validate_runtime_policy_file(&uncovered_tenant, Path::new("policy.toml"))
        .expect_err("every bank tenant must have its own maker and checker");
    assert!(error.to_string().contains("maker-checker"), "{error:#}");

    let mut implicit_tenant = valid;
    implicit_tenant.gateway.admin_tokens[1].tenant_id = None;
    let error = validate_runtime_policy_file(&implicit_tenant, Path::new("policy.toml"))
        .expect_err("bank control-plane implicit tenant identity must fail closed");
    assert!(
        error.to_string().contains("explicit tenant_id"),
        "{error:#}"
    );
}
