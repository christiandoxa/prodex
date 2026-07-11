use crate::RuntimeRotationProxy;
use anyhow::{Context as _, Result, bail};
use std::time::Duration;

pub(super) fn wait_for_signal_and_drain(proxy: &RuntimeRotationProxy) -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to initialize gateway shutdown signal runtime")?
        .block_on(wait_for_signal())?;
    let timeout = Duration::from_millis(
        prodex_gateway_http::GatewayHttpPolicy::production_default().connection_drain_timeout_ms,
    );
    if !proxy.shutdown_and_drain(timeout) {
        bail!("gateway shutdown timed out while draining active requests");
    }
    Ok(())
}

#[cfg(unix)]
async fn wait_for_signal() -> Result<()> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("failed to register gateway SIGTERM handler")?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.context("failed to wait for gateway SIGINT"),
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() -> Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("failed to wait for gateway shutdown signal")
}
