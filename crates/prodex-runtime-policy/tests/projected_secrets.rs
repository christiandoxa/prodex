use prodex_runtime_policy::load_runtime_policy_from_root;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("prodex-policy-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write_policy(&self, body: &str) {
        fs::write(self.0.join("policy.toml"), body).unwrap();
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PRODUCTION_POLICY: &str = r#"
version = 1

[secrets]
production = true
projected_root = "projected"
projected_provider = "external"

[gateway]
require_auth = true
auth_token_ref = { provider = "external", name = "gateway-token" }
provider_api_key_ref = { provider = "external", name = "provider-key", version = "v1" }

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "external", name = "postgres-url" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "external", name = "admin-token" }

[[gateway.virtual_keys]]
name = "tenant-client"
token_ref = { provider = "external", name = "client-token" }

[gateway.sso]
proxy_token_ref = { provider = "external", name = "sso-token" }

[gateway.observability]
http_bearer_token_ref = { provider = "external", name = "telemetry-token" }

[gateway.guardrails]
webhook_bearer_token_ref = { provider = "external", name = "webhook-token" }
"#;

#[test]
fn production_policy_keeps_projected_secret_references() {
    let root = TestRoot::new("projected");
    root.write_policy(PRODUCTION_POLICY);

    let policy = load_runtime_policy_from_root(root.path()).unwrap().unwrap();

    assert!(policy.secrets.production);
    assert_eq!(
        policy.secrets.projected_root.as_deref(),
        Some(root.path().join("projected").as_path())
    );
    assert_eq!(
        policy.gateway.auth_token_ref.as_ref().unwrap().name(),
        "gateway-token"
    );
    assert_eq!(
        policy
            .gateway
            .provider_api_key_ref
            .as_ref()
            .unwrap()
            .version(),
        Some("v1")
    );
    assert!(policy.gateway.admin_tokens[0].token_env.is_empty());
    assert!(policy.gateway.virtual_keys[0].token_env.is_empty());
}

#[test]
fn production_policy_rejects_environment_secret_sources() {
    let root = TestRoot::new("raw-env");
    root.write_policy(&PRODUCTION_POLICY.replace(
        "token_ref = { provider = \"external\", name = \"client-token\" }",
        "token_env = \"PRODEX_CLIENT_TOKEN\"",
    ));

    let error = load_runtime_policy_from_root(root.path()).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("forbidden when secrets.production=true"));
    assert!(!message.contains("client-token"));
}

#[test]
fn production_policy_rejects_foreign_secret_provider() {
    let root = TestRoot::new("foreign-provider");
    root.write_policy(&PRODUCTION_POLICY.replace(
        "provider = \"external\", name = \"provider-key\"",
        "provider = \"other\", name = \"provider-key\"",
    ));

    let error = load_runtime_policy_from_root(root.path()).unwrap_err();
    assert!(
        format!("{error:#}")
            .contains("must use secrets.projected_provider when secrets.production=true")
    );
}

#[test]
fn production_policy_rejects_disabled_postgres_tls() {
    let root = TestRoot::new("postgres-plaintext");
    root.write_policy(&PRODUCTION_POLICY.replace(
        "postgres_url_ref = { provider = \"external\", name = \"postgres-url\" }",
        "postgres_url_ref = { provider = \"external\", name = \"postgres-url\" }\npostgres_tls_mode = \"disable\"",
    ));

    let error = load_runtime_policy_from_root(root.path()).unwrap_err();
    assert!(format!("{error:#}").contains("cannot be disable when secrets.production=true"));
}

#[test]
fn production_policy_accepts_verify_full_with_custom_ca() {
    let root = TestRoot::new("postgres-verify-full");
    root.write_policy(&PRODUCTION_POLICY.replace(
        "postgres_url_ref = { provider = \"external\", name = \"postgres-url\" }",
        "postgres_url_ref = { provider = \"external\", name = \"postgres-url\" }\npostgres_tls_mode = \"verify-full\"\npostgres_tls_ca_path = \"certs/postgres-ca.pem\"",
    ));

    let policy = load_runtime_policy_from_root(root.path()).unwrap().unwrap();
    assert_eq!(
        policy.gateway.state.postgres_tls_mode.as_deref(),
        Some("verify-full")
    );
    assert_eq!(
        policy.gateway.state.postgres_tls_ca_path.as_deref(),
        Some("certs/postgres-ca.pem")
    );
}
