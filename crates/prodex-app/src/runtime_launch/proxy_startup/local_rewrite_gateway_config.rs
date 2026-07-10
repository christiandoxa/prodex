use super::*;
use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct RuntimeGatewayAdminToken {
    pub(crate) name: String,
    pub(crate) token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash,
    pub(crate) role: RuntimeGatewayAdminRole,
    pub(crate) tenant_id: Option<String>,
    pub(crate) team_id: Option<String>,
    pub(crate) project_id: Option<String>,
    pub(crate) user_id: Option<String>,
    pub(crate) budget_id: Option<String>,
    pub(crate) allowed_key_prefixes: Vec<String>,
}

impl fmt::Debug for RuntimeGatewayAdminToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayAdminToken")
            .field("name", &"<redacted>")
            .field("token_hash", &"<redacted>")
            .field("role", &self.role)
            .field("tenant_id", &self.tenant_id.as_ref().map(|_| "<redacted>"))
            .field("team_id", &self.team_id.as_ref().map(|_| "<redacted>"))
            .field(
                "project_id",
                &self.project_id.as_ref().map(|_| "<redacted>"),
            )
            .field("user_id", &self.user_id.as_ref().map(|_| "<redacted>"))
            .field("budget_id", &self.budget_id.as_ref().map(|_| "<redacted>"))
            .field("allowed_key_prefixes", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeGatewaySsoConfig {
    pub(crate) proxy_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) require_tenant: bool,
    pub(crate) token_header: String,
    pub(crate) user_header: String,
    pub(crate) role_header: String,
    pub(crate) tenant_header: String,
    pub(crate) key_prefixes_header: String,
    pub(crate) oidc: Option<RuntimeGatewayOidcConfig>,
}

impl fmt::Debug for RuntimeGatewaySsoConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewaySsoConfig")
            .field(
                "proxy_token_hash",
                &self.proxy_token_hash.as_ref().map(|_| "<redacted>"),
            )
            .field("require_tenant", &self.require_tenant)
            .field("token_header", &"<redacted>")
            .field("user_header", &"<redacted>")
            .field("role_header", &"<redacted>")
            .field("tenant_header", &"<redacted>")
            .field("key_prefixes_header", &"<redacted>")
            .field("oidc", &self.oidc)
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeGatewayOidcConfig {
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) jwks_url: Option<String>,
    pub(crate) user_claim: String,
    pub(crate) role_claim: String,
    pub(crate) tenant_claim: String,
    pub(crate) key_prefixes_claim: String,
}

impl fmt::Debug for RuntimeGatewayOidcConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayOidcConfig")
            .field("issuer", &"<redacted>")
            .field("audience", &"<redacted>")
            .field("jwks_url", &self.jwks_url.as_ref().map(|_| "<redacted>"))
            .field("user_claim", &"<redacted>")
            .field("role_claim", &"<redacted>")
            .field("tenant_claim", &"<redacted>")
            .field("key_prefixes_claim", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RuntimeGatewayAdminRole {
    Admin,
    #[default]
    Viewer,
}

impl RuntimeGatewayAdminRole {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return None;
        }
        match value.to_ascii_lowercase().as_str() {
            "admin" | "write" | "writer" => Some(Self::Admin),
            "viewer" | "read" | "readonly" | "read-only" => Some(Self::Viewer),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeGatewayStateStore {
    File {
        key_store_path: PathBuf,
        usage_path: PathBuf,
        ledger_path: PathBuf,
    },
    Sqlite {
        path: PathBuf,
    },
    Postgres {
        url: String,
        state_path: PathBuf,
    },
    Redis {
        url: String,
        state_path: PathBuf,
    },
}

impl fmt::Debug for RuntimeGatewayStateStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { .. } => f
                .debug_struct("File")
                .field("key_store_path", &"<redacted>")
                .field("usage_path", &"<redacted>")
                .field("ledger_path", &"<redacted>")
                .finish(),
            Self::Sqlite { .. } => f
                .debug_struct("Sqlite")
                .field("path", &"<redacted>")
                .finish(),
            Self::Postgres { .. } => f
                .debug_struct("Postgres")
                .field("url", &"<redacted>")
                .field("state_path", &"<redacted>")
                .finish(),
            Self::Redis { .. } => f
                .debug_struct("Redis")
                .field("url", &"<redacted>")
                .field("state_path", &"<redacted>")
                .finish(),
        }
    }
}

impl RuntimeGatewayStateStore {
    pub(crate) fn file(paths: &AppPaths) -> Self {
        Self::File {
            key_store_path: paths.root.join("gateway-virtual-keys.json"),
            usage_path: paths.root.join("gateway-virtual-key-usage.json"),
            ledger_path: paths.root.join("gateway-billing-ledger.jsonl"),
        }
    }

    pub(crate) fn sqlite(path: PathBuf) -> Self {
        Self::Sqlite { path }
    }

    pub(crate) fn postgres(url_env: String, url: String) -> Self {
        Self::Postgres {
            state_path: PathBuf::from(format!("postgres:{url_env}")),
            url,
        }
    }

    pub(crate) fn redis(url_env: String, url: String) -> Self {
        Self::Redis {
            state_path: PathBuf::from(format!("redis:{url_env}")),
            url,
        }
    }

    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::File { .. } => "file",
            Self::Sqlite { .. } => "sqlite",
            Self::Postgres { .. } => "postgres",
            Self::Redis { .. } => "redis",
        }
    }

    pub(super) fn key_store_path(&self) -> &Path {
        match self {
            Self::File { key_store_path, .. } => key_store_path,
            Self::Sqlite { path } => path,
            Self::Postgres { state_path, .. } => state_path,
            Self::Redis { state_path, .. } => state_path,
        }
    }

    pub(super) fn usage_path(&self) -> &Path {
        match self {
            Self::File { usage_path, .. } => usage_path,
            Self::Sqlite { path } => path,
            Self::Postgres { state_path, .. } => state_path,
            Self::Redis { state_path, .. } => state_path,
        }
    }

    pub(super) fn ledger_path(&self) -> &Path {
        match self {
            Self::File { ledger_path, .. } => ledger_path,
            Self::Sqlite { path } => path,
            Self::Postgres { state_path, .. } => state_path,
            Self::Redis { state_path, .. } => state_path,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeGatewayObservabilityConfig {
    pub(crate) sinks: Vec<String>,
    pub(crate) jsonl_path: Option<PathBuf>,
    pub(crate) http_endpoint: Option<String>,
    pub(crate) http_schema: String,
    pub(crate) http_bearer_token: Option<String>,
}

impl fmt::Debug for RuntimeGatewayObservabilityConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayObservabilityConfig")
            .field("sinks", &self.sinks)
            .field(
                "jsonl_path",
                &self.jsonl_path.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "http_endpoint",
                &self.http_endpoint.as_ref().map(|_| "<redacted>"),
            )
            .field("http_schema", &self.http_schema)
            .field(
                "http_bearer_token",
                &self.http_bearer_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeGatewayGuardrailWebhookConfig {
    pub(crate) url: Option<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) bearer_token: Option<String>,
    pub(crate) fail_closed: bool,
}

impl fmt::Debug for RuntimeGatewayGuardrailWebhookConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayGuardrailWebhookConfig")
            .field("url", &self.url.as_ref().map(|_| "<redacted>"))
            .field("phases", &self.phases)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "<redacted>"),
            )
            .field("fail_closed", &self.fail_closed)
            .finish()
    }
}

impl RuntimeGatewayGuardrailWebhookConfig {
    pub(super) fn enabled_for(&self, phase: &str) -> bool {
        self.url.is_some()
            && (self.phases.is_empty()
                || self
                    .phases
                    .iter()
                    .any(|configured| configured.eq_ignore_ascii_case(phase)))
    }
}

impl RuntimeGatewayObservabilityConfig {
    pub(super) fn sink_enabled(&self, sink: &str) -> bool {
        self.sinks
            .iter()
            .any(|configured| configured.eq_ignore_ascii_case(sink))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_redacted(rendered: &str, raw_values: &[&str]) {
        assert!(rendered.contains("<redacted>"), "{rendered}");
        for raw in raw_values {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }

    #[test]
    fn gateway_runtime_config_debug_output_redacts_sensitive_fields() {
        let admin = RuntimeGatewayAdminToken {
            name: "tenant-admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                "admin-token-secret",
            ),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: Some("tenant-a".to_string()),
            team_id: Some("team-a".to_string()),
            project_id: Some("project-a".to_string()),
            user_id: Some("user-a".to_string()),
            budget_id: Some("budget-a".to_string()),
            allowed_key_prefixes: vec!["sk-tenant-a-".to_string()],
        };
        assert_redacted(
            &format!("{admin:?}"),
            &[
                "tenant-admin",
                "admin-token-secret",
                "tenant-a",
                "team-a",
                "project-a",
                "user-a",
                "budget-a",
                "sk-tenant-a-",
            ],
        );

        let sso = RuntimeGatewaySsoConfig {
            proxy_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                "sso-proxy-secret",
            )),
            require_tenant: true,
            token_header: "x-prodex-sso-token".to_string(),
            user_header: "x-prodex-sso-user".to_string(),
            role_header: "x-prodex-sso-role".to_string(),
            tenant_header: "x-prodex-sso-tenant".to_string(),
            key_prefixes_header: "x-prodex-sso-key-prefixes".to_string(),
            oidc: Some(RuntimeGatewayOidcConfig {
                issuer: "https://issuer.example.test".to_string(),
                audience: "prodex-gateway".to_string(),
                jwks_url: Some("https://issuer.example.test/jwks.json".to_string()),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
            }),
        };
        assert_redacted(
            &format!("{sso:?}"),
            &[
                "sso-proxy-secret",
                "x-prodex-sso-token",
                "https://issuer.example.test",
                "prodex-gateway",
                "prodex_role",
            ],
        );

        let state = RuntimeGatewayStateStore::Postgres {
            url: "postgres://prodex:secret@db.example.test/prodex".to_string(),
            state_path: PathBuf::from("postgres:PRODEX_DATABASE_URL"),
        };
        assert_redacted(
            &format!("{state:?}"),
            &[
                "postgres://prodex:secret@db.example.test/prodex",
                "PRODEX_DATABASE_URL",
            ],
        );

        let observability = RuntimeGatewayObservabilityConfig {
            sinks: vec!["otlp-http".to_string()],
            jsonl_path: Some(PathBuf::from("/var/log/prodex/runtime.jsonl")),
            http_endpoint: Some("https://otel.example.test/v1/logs".to_string()),
            http_schema: "otlp-http-json".to_string(),
            http_bearer_token: Some("otlp-bearer-secret".to_string()),
        };
        assert_redacted(
            &format!("{observability:?}"),
            &[
                "/var/log/prodex/runtime.jsonl",
                "https://otel.example.test/v1/logs",
                "otlp-bearer-secret",
            ],
        );

        let webhook = RuntimeGatewayGuardrailWebhookConfig {
            url: Some("https://guardrail.example.test/check".to_string()),
            phases: vec!["pre".to_string()],
            bearer_token: Some("guardrail-bearer-secret".to_string()),
            fail_closed: true,
        };
        assert_redacted(
            &format!("{webhook:?}"),
            &[
                "https://guardrail.example.test/check",
                "guardrail-bearer-secret",
            ],
        );
    }
}
