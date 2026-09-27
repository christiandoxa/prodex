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

fn provider_policy_tag(provider: &ProfileProvider) -> u8 {
    match provider {
        ProfileProvider::Openai => 0,
        ProfileProvider::Gemini { .. } => 1,
        ProfileProvider::Anthropic { .. } => 2,
        ProfileProvider::Copilot { .. } => 3,
        ProfileProvider::Kiro { .. } => 4,
        ProfileProvider::Agy { .. } => 5,
    }
}

pub(super) fn provider_runtime_pool_priority(provider: &ProfileProvider) -> usize {
    prodex_mojo_core::state_policy::provider_capabilities(provider_policy_tag(provider))
        .expect("Mojo provider capability policy returned invalid output")
        .provider_priority
}

impl ProfileProvider {
    pub fn capabilities(&self) -> ProviderCapabilities {
        let plan = prodex_mojo_core::state_policy::provider_capabilities(provider_policy_tag(self))
            .expect("Mojo provider capability policy returned invalid output");
        ProviderCapabilities {
            runtime_route_policy: match plan.route_policy {
                0 => RuntimeRoutePolicy::NativeCodex,
                1 => RuntimeRoutePolicy::ResponsesAdapter,
                2 => RuntimeRoutePolicy::ExternalCli,
                _ => unreachable!("validated Mojo provider route-policy tag"),
            },
            quota_shape: match plan.quota_shape {
                0 => ProviderQuotaShape::OpenAiWindows,
                1 => ProviderQuotaShape::GeminiBuckets,
                2 => ProviderQuotaShape::CopilotMonthly,
                3 => ProviderQuotaShape::ExternalStatus,
                _ => unreachable!("validated Mojo provider quota-shape tag"),
            },
            uses_openai_client_format: plan.uses_openai_client_format,
            supports_runtime_rotation: plan.supports_runtime_rotation,
            supports_remote_compact_affinity: plan.supports_remote_compact_affinity,
            supports_websocket_reuse: plan.supports_websocket_reuse,
        }
    }

    pub fn supports_codex_runtime(&self) -> bool {
        matches!(
            self.capabilities().runtime_route_policy,
            RuntimeRoutePolicy::NativeCodex
        )
    }
}
