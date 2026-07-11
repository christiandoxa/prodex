use super::gateway_config;
use crate::app_state::AppStateIoExt;
use crate::{
    AppPaths, AppState, GatewayArgs, GatewayBackend, RuntimeGatewayCredentialRefreshCandidate,
    RuntimeGatewayCredentialRefreshPlan, RuntimeLocalRewriteProxyStartOptions,
    start_runtime_gateway_rewrite_proxy, start_runtime_gateway_rewrite_proxy_with_secret_refresh,
};
use anyhow::Result;
use std::sync::Arc;

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
    let policy = prodex_runtime_policy::runtime_policy_gateway().unwrap_or_default();
    let secrets = prodex_runtime_policy::runtime_policy_secrets().unwrap_or_default();
    let gateway = gateway_config::resolve_gateway_launch_config_with_secrets(
        &paths, &state, &args, &policy, &secrets,
    )?;
    let secret_refresh = secrets.projected_root.is_some().then(|| {
        let refresh_paths = paths.clone();
        let refresh_state = state.clone();
        let refresh_args = gateway_refresh_args(&args);
        let refresh_policy = policy.clone();
        let refresh_secrets = secrets.clone();
        RuntimeGatewayCredentialRefreshPlan::new(
            gateway.credential_fingerprint,
            Arc::new(move || {
                let refreshed = gateway_config::resolve_gateway_launch_config_with_secrets(
                    &refresh_paths,
                    &refresh_state,
                    &refresh_args,
                    &refresh_policy,
                    &refresh_secrets,
                )?;
                Ok(gateway_refresh_candidate(&refreshed))
            }),
        )
    });
    let request_constraints = gateway.request_constraints;
    let options = RuntimeLocalRewriteProxyStartOptions {
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
    };
    let proxy = match secret_refresh {
        Some(secret_refresh) => start_runtime_gateway_rewrite_proxy_with_secret_refresh(
            options,
            secret_refresh,
            request_constraints,
        )?,
        None => start_runtime_gateway_rewrite_proxy(options, request_constraints)?,
    };
    Ok(GatewayBackend::new(
        proxy,
        gateway.provider_name.unwrap_or("openai-compatible"),
        gateway.auth_required,
    ))
}

fn gateway_refresh_args(args: &GatewayArgs) -> GatewayArgs {
    GatewayArgs {
        command: None,
        listen: args.listen.clone(),
        provider: args.provider,
        base_url: args.base_url.clone(),
        api_key: args.api_key.clone(),
        auth_token: args.auth_token.clone(),
        smart_context: args.smart_context,
        presidio: args.presidio,
        no_presidio: args.no_presidio,
    }
}

fn gateway_refresh_candidate(
    gateway: &gateway_config::ResolvedGatewayLaunchConfig,
) -> RuntimeGatewayCredentialRefreshCandidate {
    let (provider, provider_credential) = gateway.provider_options.clone().into_runtime_parts();
    RuntimeGatewayCredentialRefreshCandidate {
        fingerprint: gateway.credential_fingerprint,
        provider,
        provider_credential,
        auth_token_hash: gateway.auth_token_hash.clone(),
        admin_tokens: gateway.admin_tokens.clone(),
        sso: gateway.sso.clone(),
        virtual_keys: gateway.virtual_keys.clone(),
        guardrail_webhook: gateway.guardrail_webhook.clone(),
        observability: gateway.observability.clone(),
    }
}
