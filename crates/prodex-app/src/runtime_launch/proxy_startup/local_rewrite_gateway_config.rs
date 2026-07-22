use super::local_rewrite_options::RuntimeGatewaySecret;
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
    pub(crate) browser: Option<RuntimeGatewayBrowserConfig>,
    pub(crate) workload_identity: Option<RuntimeGatewayWorkloadIdentityConfig>,
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
            .field("browser", &self.browser)
            .field("workload_identity", &self.workload_identity)
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeGatewayBrowserConfig {
    pub(crate) authorization_url: String,
    pub(crate) token_url: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: Option<RuntimeGatewaySecret>,
    pub(crate) redirect_uri: String,
}

impl fmt::Debug for RuntimeGatewayBrowserConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayBrowserConfig")
            .field("authorization_url", &"<redacted>")
            .field("token_url", &"<redacted>")
            .field("client_id", &"<redacted>")
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("redirect_uri", &"<redacted>")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeGatewayWorkloadIdentityConfig {
    pub(crate) oidc: RuntimeGatewayOidcConfig,
    pub(crate) subject_claim: String,
    pub(crate) tenant_claim: String,
    pub(crate) scope_claim: String,
    pub(crate) required_scope: String,
    pub(crate) mtls_required: bool,
}

impl fmt::Debug for RuntimeGatewayWorkloadIdentityConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayWorkloadIdentityConfig")
            .field("oidc", &self.oidc)
            .field("subject_claim", &"<redacted>")
            .field("tenant_claim", &"<redacted>")
            .field("scope_claim", &"<redacted>")
            .field("required_scope", &self.required_scope)
            .field("mtls_required", &self.mtls_required)
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeGatewayOidcConfig {
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) jwks_url: Option<String>,
    pub(crate) jwks_origin_allowlist: Vec<String>,
    pub(crate) user_claim: String,
    pub(crate) role_claim: String,
    pub(crate) tenant_claim: String,
    pub(crate) key_prefixes_claim: String,
    pub(crate) authentication_strength: Option<String>,
    pub(crate) reauthentication_max_age_seconds: Option<u64>,
}

impl fmt::Debug for RuntimeGatewayOidcConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayOidcConfig")
            .field("issuer", &"<redacted>")
            .field("audience", &"<redacted>")
            .field("jwks_url", &self.jwks_url.as_ref().map(|_| "<redacted>"))
            .field(
                "jwks_origin_allowlist_count",
                &self.jwks_origin_allowlist.len(),
            )
            .field("user_claim", &"<redacted>")
            .field("role_claim", &"<redacted>")
            .field("tenant_claim", &"<redacted>")
            .field("key_prefixes_claim", &"<redacted>")
            .field(
                "authentication_strength",
                &self.authentication_strength.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "reauthentication_max_age_seconds",
                &self
                    .reauthentication_max_age_seconds
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
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
        coordination_redis_url: Option<String>,
        tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
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

    #[cfg(test)]
    pub(crate) fn postgres(url_env: String, url: String) -> Self {
        Self::postgres_with_coordination(url_env, url, None)
    }

    #[cfg(test)]
    pub(crate) fn postgres_with_coordination(
        url_env: String,
        url: String,
        coordination_redis_url: Option<String>,
    ) -> Self {
        Self::postgres_with_coordination_and_tls(
            url_env,
            url,
            coordination_redis_url,
            prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable(),
        )
    }

    pub(crate) fn postgres_with_coordination_and_tls(
        url_env: String,
        url: String,
        coordination_redis_url: Option<String>,
        tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
    ) -> Self {
        Self::Postgres {
            state_path: PathBuf::from(format!("postgres:{url_env}")),
            url,
            coordination_redis_url,
            tls,
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

    pub(crate) fn coordination_redis_url(&self) -> Option<&str> {
        match self {
            Self::Postgres {
                coordination_redis_url,
                ..
            } => coordination_redis_url.as_deref(),
            Self::Redis { url, .. } => Some(url),
            Self::File { .. } | Self::Sqlite { .. } => None,
        }
    }
}

pub(crate) fn runtime_gateway_postgres_repository(
    state_store: &RuntimeGatewayStateStore,
    worker_count: usize,
) -> anyhow::Result<Option<prodex_storage_postgres_runtime::PostgresRepository>> {
    let RuntimeGatewayStateStore::Postgres { url, tls, .. } = state_store else {
        return Ok(None);
    };
    let config = prodex_storage_postgres_runtime::PostgresRuntimeConfig::new(
        url.clone(),
        worker_count.clamp(1, 32),
    )
    .map_err(|_| anyhow::anyhow!("failed to configure PostgreSQL gateway accounting pool"))?;
    prodex_storage_postgres_runtime::PostgresRepository::from_config_with_tls_config(&config, tls)
        .map(Some)
        .map_err(|_| anyhow::anyhow!("failed to configure PostgreSQL gateway accounting pool"))
}

pub(crate) fn runtime_gateway_redis_rate_limit_executor(
    state_store: &RuntimeGatewayStateStore,
    runtime_shared: &RuntimeRotationProxyShared,
) -> anyhow::Result<Option<prodex_storage_redis_runtime::RedisRateLimitExecutor>> {
    let RuntimeGatewayStateStore::Postgres {
        coordination_redis_url: Some(url),
        ..
    } = state_store
    else {
        return Ok(None);
    };
    let config = prodex_storage_redis_runtime::RedisRuntimeConfig::new(url.clone())
        .map_err(|_| anyhow::anyhow!("failed to configure Redis gateway rate limiter"))?;
    runtime_shared
        .async_runtime
        .handle()
        .block_on(prodex_storage_redis_runtime::RedisRateLimitExecutor::connect(&config))
        .map(Some)
        .map_err(|_| anyhow::anyhow!("failed to connect Redis gateway rate limiter"))
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeGatewayObservabilityConfig {
    pub(crate) sinks: Vec<String>,
    pub(crate) jsonl_path: Option<PathBuf>,
    pub(crate) http_endpoint: Option<String>,
    pub(crate) http_schema: String,
    pub(crate) http_bearer_token: Option<RuntimeGatewaySecret>,
    pub(crate) siem_worker: Option<
        Arc<
            crate::app_commands::runtime_launch::gateway_config::gateway_siem_export::RuntimeSiemWorkerConfig,
        >,
    >,
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
            .field(
                "siem_worker",
                &self.siem_worker.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeGatewayGuardrailWebhookConfig {
    pub(crate) url: Option<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) bearer_token: Option<RuntimeGatewaySecret>,
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
    fn postgres_repository_follows_backend_and_redacts_invalid_urls() {
        let file = RuntimeGatewayStateStore::File {
            key_store_path: PathBuf::from("keys"),
            usage_path: PathBuf::from("usage"),
            ledger_path: PathBuf::from("ledger"),
        };
        assert!(
            runtime_gateway_postgres_repository(&file, 4)
                .unwrap()
                .is_none()
        );

        let postgres = RuntimeGatewayStateStore::postgres(
            "PRODEX_DATABASE_URL".to_string(),
            "postgres://localhost/prodex".to_string(),
        );
        assert!(
            runtime_gateway_postgres_repository(&postgres, 0)
                .unwrap()
                .is_some()
        );

        let secret = "not-a-postgres-url-with-secret";
        let invalid = RuntimeGatewayStateStore::postgres(
            "PRODEX_DATABASE_URL".to_string(),
            secret.to_string(),
        );
        let error = runtime_gateway_postgres_repository(&invalid, 4).unwrap_err();
        assert_eq!(
            error.to_string(),
            "failed to configure PostgreSQL gateway accounting pool"
        );
        assert!(!error.to_string().contains(secret));
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
                jwks_origin_allowlist: Vec::new(),
                user_claim: "email".to_string(),
                role_claim: "prodex_role".to_string(),
                tenant_claim: "prodex_tenant".to_string(),
                key_prefixes_claim: "prodex_key_prefixes".to_string(),
                authentication_strength: None,
                reauthentication_max_age_seconds: None,
            }),
            browser: None,
            workload_identity: None,
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
            coordination_redis_url: Some("redis://:secret@redis.example.test/0".to_string()),
            tls: prodex_storage_postgres_runtime::PostgresTlsConfig::verify_full(None),
        };
        assert_redacted(
            &format!("{state:?}"),
            &[
                "postgres://prodex:secret@db.example.test/prodex",
                "PRODEX_DATABASE_URL",
                "redis://:secret@redis.example.test/0",
            ],
        );

        let observability = RuntimeGatewayObservabilityConfig {
            sinks: vec!["otlp-http".to_string()],
            jsonl_path: Some(PathBuf::from("/var/log/prodex/runtime.jsonl")),
            http_endpoint: Some("https://otel.example.test/v1/logs".to_string()),
            http_schema: "otlp-http-json".to_string(),
            http_bearer_token: Some(RuntimeGatewaySecret::development_compatibility(
                prodex_domain::SecretMaterial::new(
                    "otlp-bearer-secret".as_bytes().to_vec(),
                    None::<String>,
                ),
            )),
            siem_worker: None,
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
            bearer_token: Some(RuntimeGatewaySecret::development_compatibility(
                prodex_domain::SecretMaterial::new(
                    "guardrail-bearer-secret".as_bytes().to_vec(),
                    None::<String>,
                ),
            )),
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
