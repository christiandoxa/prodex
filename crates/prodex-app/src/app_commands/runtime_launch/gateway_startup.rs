use super::gateway_config;
use crate::app_state::AppStateIoExt;
use crate::{
    AppPaths, AppState, GatewayArgs, GatewayBackend, RuntimeLocalRewriteProxyStartOptions,
    start_runtime_gateway_rewrite_proxy,
};
use anyhow::Result;

pub(super) fn start_policy_gateway_backend(
    preferred_listen_addr: Option<String>,
) -> Result<GatewayBackend> {
    start_gateway_backend(GatewayArgs {
        command: None,
        listen: preferred_listen_addr,
        provider: None,
        base_url: None,
        api_key: None,
        auth_token: None,
        smart_context: false,
        presidio: false,
        no_presidio: false,
    })
}

pub(super) fn start_gateway_backend(args: GatewayArgs) -> Result<GatewayBackend> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let gateway = gateway_config::resolve_current_gateway_launch_config(&paths, &state, &args)?;
    let proxy = start_runtime_gateway_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &state,
        upstream_base_url: gateway.upstream_base_url,
        provider: gateway.provider_options,
        upstream_no_proxy: false,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: gateway.presidio_redaction_enabled,
        model_context_window_tokens: None,
        preferred_listen_addr: Some(&gateway.listen_addr),
        gateway_auth_token_hash: gateway.auth_token_hash,
        gateway_admin_tokens: gateway.admin_tokens,
        gateway_sso: gateway.sso,
        gateway_state_store: gateway.state_store,
        gateway_virtual_keys: gateway.virtual_keys,
        gateway_route_aliases: gateway.route_aliases,
        gateway_guardrails: gateway.guardrails,
        gateway_guardrail_webhook: gateway.guardrail_webhook,
        gateway_call_id_header: Some(gateway.call_id_header),
        gateway_observability: gateway.observability,
    })?;
    Ok(GatewayBackend::new(
        proxy,
        gateway.provider_name.unwrap_or("openai-compatible"),
        gateway.auth_required,
    ))
}
