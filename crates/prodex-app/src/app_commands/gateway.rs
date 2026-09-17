use super::*;
use anyhow::bail;

pub(crate) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    let provider = args.provider.map(SuperExternalProvider::as_str);
    let presidio = args.presidio && !args.no_presidio;
    let request = RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: args.base_url.as_deref(),
        upstream_no_proxy: false,
        include_code_review: false,
        requested_model: None,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: presidio,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: true,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: provider,
        external_provider_api_key: args.api_key.as_deref(),
    };
    let resolved_harness = prodex_provider_core::resolve_harness_mode(args.harness, None);
    let prepared =
        runtime_launch::prepare_gateway_runtime(request, resolved_harness, args.listen.as_deref())?;
    let Some(endpoint) = prepared.runtime_proxy.as_ref() else {
        bail!("gateway provider does not expose an OpenAI-compatible proxy");
    };
    println!(
        "Prodex gateway listening on http://{}{}",
        endpoint.listen_addr, endpoint.openai_mount_path
    );
    wait_for_gateway_signal()?;
    drop(prepared);
    Ok(())
}

fn wait_for_gateway_signal() -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to initialize gateway signal runtime")?
        .block_on(wait_for_signal())
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
