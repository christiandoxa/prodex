use std::time::Duration;

use anyhow::Result;
use prodex_gateway_server::{GatewayHandlerRequest, GatewayHandlerResult};

use crate::{RuntimeGatewayApplication, app_commands};

pub struct GatewayApplication {
    runtime: RuntimeGatewayApplication,
    provider_name: &'static str,
    auth_required: bool,
}

impl GatewayApplication {
    pub(crate) fn new(
        runtime: RuntimeGatewayApplication,
        provider_name: &'static str,
        auth_required: bool,
    ) -> Self {
        Self {
            runtime,
            provider_name,
            auth_required,
        }
    }

    pub async fn handle(&self, request: GatewayHandlerRequest) -> GatewayHandlerResult {
        self.runtime.handle(request).await
    }

    pub const fn provider_name(&self) -> &'static str {
        self.provider_name
    }

    pub const fn auth_required(&self) -> bool {
        self.auth_required
    }

    pub fn shutdown_and_drain(&self, timeout: Duration) -> bool {
        self.runtime.shutdown_and_drain(timeout)
    }
}

impl std::fmt::Debug for GatewayApplication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayApplication")
            .field("provider_name", &self.provider_name)
            .field("auth_required", &self.auth_required)
            .finish_non_exhaustive()
    }
}

pub fn start_policy_gateway_application() -> Result<GatewayApplication> {
    start_policy_gateway_application_for_mode(
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
    )
}

pub fn start_policy_gateway_application_for_mode(
    service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
) -> Result<GatewayApplication> {
    prodex_runtime_policy::ensure_runtime_policy_service_mode(service_mode)?;
    app_commands::start_policy_gateway_application_inner(service_mode)
}
