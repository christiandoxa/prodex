use super::ProfileProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRoutePolicy {
    NativeCodex,
    ResponsesAdapter,
    ExternalCli,
    Unsupported,
}

impl RuntimeRoutePolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::NativeCodex => "native-codex",
            Self::ResponsesAdapter => "responses-adapter",
            Self::ExternalCli => "external-cli",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderQuotaShape {
    OpenAiWindows,
    GeminiBuckets,
    CopilotMonthly,
    ExternalStatus,
}

impl ProviderQuotaShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAiWindows => "openai-windows",
            Self::GeminiBuckets => "gemini-buckets",
            Self::CopilotMonthly => "copilot-monthly",
            Self::ExternalStatus => "external-status",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub runtime_route_policy: RuntimeRoutePolicy,
    pub quota_shape: ProviderQuotaShape,
    pub uses_openai_client_format: bool,
    pub supports_runtime_rotation: bool,
    pub supports_remote_compact_affinity: bool,
    pub supports_websocket_reuse: bool,
}

impl ProviderCapabilities {
    const fn new(
        runtime_route_policy: RuntimeRoutePolicy,
        quota_shape: ProviderQuotaShape,
        uses_openai_client_format: bool,
        supports_runtime_rotation: bool,
    ) -> Self {
        Self {
            runtime_route_policy,
            quota_shape,
            uses_openai_client_format,
            supports_runtime_rotation,
            supports_remote_compact_affinity: supports_runtime_rotation,
            supports_websocket_reuse: supports_runtime_rotation,
        }
    }
}

impl ProfileProvider {
    pub fn capabilities(&self) -> ProviderCapabilities {
        match self {
            Self::Openai => ProviderCapabilities::new(
                RuntimeRoutePolicy::NativeCodex,
                ProviderQuotaShape::OpenAiWindows,
                true,
                true,
            ),
            Self::Gemini { .. } => ProviderCapabilities::new(
                RuntimeRoutePolicy::ResponsesAdapter,
                ProviderQuotaShape::GeminiBuckets,
                true,
                false,
            ),
            Self::Copilot { .. } => ProviderCapabilities::new(
                RuntimeRoutePolicy::ResponsesAdapter,
                ProviderQuotaShape::CopilotMonthly,
                true,
                false,
            ),
            Self::Kiro { .. } => ProviderCapabilities::new(
                RuntimeRoutePolicy::ResponsesAdapter,
                ProviderQuotaShape::ExternalStatus,
                true,
                false,
            ),
            Self::Anthropic { .. } => ProviderCapabilities::new(
                RuntimeRoutePolicy::ResponsesAdapter,
                ProviderQuotaShape::ExternalStatus,
                true,
                false,
            ),
            Self::Agy { .. } => ProviderCapabilities::new(
                RuntimeRoutePolicy::ExternalCli,
                ProviderQuotaShape::ExternalStatus,
                false,
                false,
            ),
        }
    }

    pub fn supports_codex_runtime(&self) -> bool {
        matches!(
            self.capabilities().runtime_route_policy,
            RuntimeRoutePolicy::NativeCodex
        )
    }
}
