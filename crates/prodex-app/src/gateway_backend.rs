use crate::{RuntimeRotationProxy, app_commands};
use anyhow::Result;
use std::{net::SocketAddr, time::Duration};

pub struct GatewayBackend {
    proxy: RuntimeRotationProxy,
    provider_name: &'static str,
    auth_required: bool,
}

impl GatewayBackend {
    pub(crate) fn new(
        proxy: RuntimeRotationProxy,
        provider_name: &'static str,
        auth_required: bool,
    ) -> Self {
        Self {
            proxy,
            provider_name,
            auth_required,
        }
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.proxy.listen_addr
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider_name
    }

    pub fn auth_required(&self) -> bool {
        self.auth_required
    }

    #[cfg(test)]
    pub(crate) fn runtime_config(&self) -> &crate::RuntimeConfig {
        &self.proxy.runtime_config
    }

    pub fn shutdown_and_drain(&self, timeout: Duration) -> bool {
        self.proxy.shutdown_and_drain(timeout)
    }
}

impl std::fmt::Debug for GatewayBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayBackend")
            .field("listen_addr", &self.listen_addr())
            .field("provider_name", &self.provider_name)
            .field("auth_required", &self.auth_required)
            .finish_non_exhaustive()
    }
}

pub fn start_policy_gateway_backend(
    preferred_listen_addr: Option<String>,
) -> Result<GatewayBackend> {
    app_commands::start_policy_gateway_backend_inner(preferred_listen_addr)
}
