use super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode;
use super::local_rewrite_gateway_config::{
    RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewaySsoConfig, RuntimeGatewayStateStore,
};
use super::local_rewrite_kiro::RuntimeKiroProfileAuth;
use super::provider_bridge::RuntimeProviderBridgeKind;
use super::*;
use prodex_domain::{SecretMaterial, SecretPurpose, SecretRef};
use secret_store::ProjectedSecretProvider;
use std::{fmt, sync::Arc};

#[derive(Clone)]
pub(crate) struct RuntimeProjectedProviderCredential {
    reference: SecretRef,
    provider: ProjectedSecretProvider,
}

impl RuntimeProjectedProviderCredential {
    pub(crate) fn new(reference: SecretRef, provider: ProjectedSecretProvider) -> Self {
        Self {
            reference,
            provider,
        }
    }

    pub(crate) fn reference(&self) -> &SecretRef {
        &self.reference
    }

    pub(super) fn provider(&self) -> &ProjectedSecretProvider {
        &self.provider
    }

    pub(super) fn with_reference(&self, reference: SecretRef) -> Option<Self> {
        (reference.provider() == self.reference.provider()).then(|| Self {
            reference,
            provider: self.provider.clone(),
        })
    }
}

impl fmt::Debug for RuntimeProjectedProviderCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeProjectedProviderCredential")
            .field("reference", &"<redacted>")
            .field("provider", &"<redacted>")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeGatewaySecret {
    source: Arc<RuntimeGatewaySecretSource>,
}

pub(super) enum RuntimeGatewaySecretSource {
    Projected {
        credential: RuntimeProjectedProviderCredential,
        purpose: SecretPurpose,
    },
    DevelopmentCompatibility(SecretMaterial),
}

impl RuntimeGatewaySecret {
    pub(crate) fn projected(
        credential: RuntimeProjectedProviderCredential,
        purpose: SecretPurpose,
    ) -> Self {
        Self {
            source: Arc::new(RuntimeGatewaySecretSource::Projected {
                credential,
                purpose,
            }),
        }
    }

    pub(crate) fn development_compatibility(material: SecretMaterial) -> Self {
        Self {
            source: Arc::new(RuntimeGatewaySecretSource::DevelopmentCompatibility(
                material,
            )),
        }
    }

    pub(super) fn source(&self) -> &RuntimeGatewaySecretSource {
        self.source.as_ref()
    }
}

impl fmt::Debug for RuntimeGatewaySecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewaySecret")
            .field("source", &"<redacted>")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeLocalRewriteProviderOptions {
    ProjectedCredential {
        provider: Box<RuntimeLocalRewriteProviderOptions>,
        credential: RuntimeProjectedProviderCredential,
    },
    Anthropic {
        auth: RuntimeAnthropicProviderAuth,
    },
    Copilot {
        auth: RuntimeCopilotProviderAuth,
    },
    OpenAiResponses {
        api_keys: Vec<String>,
    },
    #[allow(
        dead_code,
        reason = "retained for local embeddings compatibility and its regression tests"
    )]
    LocalEmbeddingsOnly {
        embedding_model: String,
    },
    DeepSeek {
        api_keys: Vec<String>,
        strict_tools: bool,
        beta_base_url: String,
        web_search_mode: RuntimeDeepSeekWebSearchMode,
    },
    Gemini {
        auth: RuntimeGeminiProviderAuth,
        thinking_budget_tokens: Option<u64>,
        model_resolution: crate::RuntimeGeminiModelResolution,
    },
    Kiro {
        auth: RuntimeKiroProfileAuth,
    },
}

impl RuntimeLocalRewriteProviderOptions {
    pub(crate) fn with_projected_credential(
        self,
        credential: RuntimeProjectedProviderCredential,
    ) -> Self {
        Self::ProjectedCredential {
            provider: Box::new(self),
            credential,
        }
    }

    pub(crate) fn into_runtime_parts(
        self,
    ) -> (
        RuntimeLocalRewriteProviderOptions,
        Option<RuntimeProjectedProviderCredential>,
    ) {
        match self {
            Self::ProjectedCredential {
                provider,
                credential,
            } => (*provider, Some(credential)),
            provider => (provider, None),
        }
    }

    pub(super) fn bridge_kind(&self) -> RuntimeProviderBridgeKind {
        match self {
            RuntimeLocalRewriteProviderOptions::ProjectedCredential { provider, .. } => {
                provider.bridge_kind()
            }
            RuntimeLocalRewriteProviderOptions::Anthropic { .. } => {
                RuntimeProviderBridgeKind::Anthropic
            }
            RuntimeLocalRewriteProviderOptions::Copilot { .. } => {
                RuntimeProviderBridgeKind::Copilot
            }
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { .. }
            | RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly { .. } => {
                RuntimeProviderBridgeKind::OpenAiResponses
            }
            RuntimeLocalRewriteProviderOptions::DeepSeek { .. } => {
                RuntimeProviderBridgeKind::DeepSeek
            }
            RuntimeLocalRewriteProviderOptions::Gemini { .. } => RuntimeProviderBridgeKind::Gemini,
            RuntimeLocalRewriteProviderOptions::Kiro { .. } => RuntimeProviderBridgeKind::Kiro,
        }
    }

    pub(super) fn configured_reasoning_reserve_tokens(&self) -> Option<u64> {
        match self {
            RuntimeLocalRewriteProviderOptions::ProjectedCredential { provider, .. } => {
                provider.configured_reasoning_reserve_tokens()
            }
            RuntimeLocalRewriteProviderOptions::Gemini {
                thinking_budget_tokens,
                ..
            } => *thinking_budget_tokens,
            _ => None,
        }
    }
}

pub(crate) struct RuntimeLocalRewriteProxyStartOptions<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) state: &'a AppState,
    pub(crate) upstream_base_url: String,
    pub(crate) provider: RuntimeLocalRewriteProviderOptions,
    pub(crate) upstream_no_proxy: bool,
    pub(crate) smart_context_enabled: bool,
    pub(crate) presidio_redaction_enabled: bool,
    pub(crate) model_context_window_tokens: Option<u64>,
    pub(crate) preferred_listen_addr: Option<&'a str>,
    pub(crate) gateway_auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(crate) gateway_admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(crate) gateway_sso: RuntimeGatewaySsoConfig,
    pub(crate) gateway_state_store: RuntimeGatewayStateStore,
    pub(crate) gateway_virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(crate) gateway_route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(crate) gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(crate) gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) gateway_call_id_header: Option<String>,
    pub(crate) gateway_observability: RuntimeGatewayObservabilityConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projected_secret(
        root: &std::path::Path,
        name: &str,
        purpose: SecretPurpose,
    ) -> RuntimeGatewaySecret {
        let credential = RuntimeProjectedProviderCredential::new(
            SecretRef::new("external", name, None::<String>),
            ProjectedSecretProvider::new(root, "external").unwrap(),
        );
        RuntimeGatewaySecret::projected(credential, purpose)
    }

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn projected_provider_credential_debug_is_redacted() {
        let root = std::env::temp_dir().join(format!(
            "prodex-provider-credential-debug-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let credential = RuntimeProjectedProviderCredential::new(
            SecretRef::new("debug-provider-secret", "debug-name-secret", None::<String>),
            ProjectedSecretProvider::new(&root, "debug-provider-secret").unwrap(),
        );

        let rendered = format!("{credential:?}");

        assert!(rendered.contains("RuntimeProjectedProviderCredential"));
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("debug-provider-secret"));
        assert!(!rendered.contains("debug-name-secret"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_secret_debug_redacts_projected_and_compatibility_sources() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-secret-debug-{}",
            prodex_domain::RequestId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let projected = projected_secret(
            &root,
            "debug-projected-secret-name",
            SecretPurpose::TelemetryExportCredential,
        );
        let compatibility = RuntimeGatewaySecret::development_compatibility(SecretMaterial::new(
            "debug-compatibility-secret".as_bytes().to_vec(),
            None::<String>,
        ));

        for rendered in [format!("{projected:?}"), format!("{compatibility:?}")] {
            assert!(rendered.contains("<redacted>"));
            assert!(!rendered.contains("debug-projected-secret-name"));
            assert!(!rendered.contains("debug-compatibility-secret"));
            assert!(!rendered.contains(root.to_string_lossy().as_ref()));
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
