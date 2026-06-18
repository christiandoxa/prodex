use super::*;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewaySsoConfig {
    pub(crate) proxy_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) token_header: String,
    pub(crate) user_header: String,
    pub(crate) role_header: String,
    pub(crate) tenant_header: String,
    pub(crate) key_prefixes_header: String,
    pub(crate) oidc: Option<RuntimeGatewayOidcConfig>,
    pub(crate) default_role: RuntimeGatewayAdminRole,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGatewayOidcConfig {
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) jwks_url: Option<String>,
    pub(crate) user_claim: String,
    pub(crate) role_claim: String,
    pub(crate) tenant_claim: String,
    pub(crate) key_prefixes_claim: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RuntimeGatewayAdminRole {
    #[default]
    Admin,
    Viewer,
}

impl RuntimeGatewayAdminRole {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
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

    pub(super) fn can_write(self) -> bool {
        matches!(self, Self::Admin)
    }
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewayObservabilityConfig {
    pub(crate) sinks: Vec<String>,
    pub(crate) jsonl_path: Option<PathBuf>,
    pub(crate) http_endpoint: Option<String>,
    pub(crate) http_schema: String,
    pub(crate) http_bearer_token: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeGatewayGuardrailWebhookConfig {
    pub(crate) url: Option<String>,
    pub(crate) phases: Vec<String>,
    pub(crate) bearer_token: Option<String>,
    pub(crate) fail_closed: bool,
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
