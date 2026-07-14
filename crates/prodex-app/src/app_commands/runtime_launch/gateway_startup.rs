use super::gateway_config;
use crate::app_state::AppStateIoExt;
use crate::{
    AppPaths, AppState, GatewayApplication, GatewayArgs, GatewayBackend, RuntimeConfig,
    RuntimeGatewayCredentialRefreshPlan, RuntimeLocalRewriteProxyStartOptions,
    start_runtime_gateway_application_with_runtime_config,
    start_runtime_gateway_rewrite_proxy_with_runtime_config,
};
use anyhow::Result;
use std::sync::Arc;

#[cfg(test)]
#[path = "gateway_startup/tests.rs"]
mod tests;

pub(crate) fn start_policy_gateway_backend(
    preferred_listen_addr: Option<String>,
) -> Result<GatewayBackend> {
    let service_mode = prodex_runtime_policy::runtime_policy_service_mode()?;
    start_gateway_backend_for_service_mode(
        GatewayArgs {
            command: None,
            listen: preferred_listen_addr,
            provider: None,
            harness: None,
            base_url: None,
            api_key: None,
            auth_token: None,
            smart_context: false,
            presidio: false,
            no_presidio: false,
        },
        service_mode,
    )
}

pub(crate) fn start_gateway_backend(args: GatewayArgs) -> Result<GatewayBackend> {
    prodex_runtime_policy::ensure_runtime_policy_service_mode(
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
    )?;
    start_gateway_backend_for_service_mode(
        args,
        prodex_runtime_policy::RuntimePolicyServiceMode::Gateway,
    )
}

pub(crate) fn start_policy_gateway_application(
    service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
) -> Result<GatewayApplication> {
    let (runtime, provider_name, auth_required) = start_gateway_runtime_for_service_mode(
        GatewayArgs {
            command: None,
            listen: None,
            provider: None,
            harness: None,
            base_url: None,
            api_key: None,
            auth_token: None,
            smart_context: false,
            presidio: false,
            no_presidio: false,
        },
        service_mode,
        start_runtime_gateway_application_with_runtime_config,
    )?;
    Ok(GatewayApplication::new(
        runtime,
        provider_name,
        auth_required,
    ))
}

fn start_gateway_backend_for_service_mode(
    args: GatewayArgs,
    service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
) -> Result<GatewayBackend> {
    let (proxy, provider_name, auth_required) = start_gateway_runtime_for_service_mode(
        args,
        service_mode,
        start_runtime_gateway_rewrite_proxy_with_runtime_config,
    )?;
    Ok(GatewayBackend::new(proxy, provider_name, auth_required))
}

fn start_gateway_runtime_for_service_mode<T>(
    args: GatewayArgs,
    service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
    start: impl FnOnce(
        RuntimeLocalRewriteProxyStartOptions<'_>,
        Arc<RuntimeConfig>,
        Option<RuntimeGatewayCredentialRefreshPlan>,
        prodex_provider_core::ProviderRequestConstraintPolicy,
        prodex_provider_core::ResolvedHarnessMode,
    ) -> Result<T>,
) -> Result<(T, &'static str, bool)> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let runtime_config = Arc::new(RuntimeConfig::from_gateway_env_policy_and_cli(
        &paths,
        service_mode,
        &args,
    )?);
    let policy = prodex_runtime_policy::runtime_policy_gateway().unwrap_or_default();
    let secrets = prodex_runtime_policy::runtime_policy_secrets().unwrap_or_default();
    let gateway = gateway_config::resolve_gateway_launch_config_for_service_mode(
        &paths,
        &state,
        &args,
        &policy,
        &secrets,
        &runtime_config,
        service_mode,
    )?;
    let refresh_template = gateway_config::gateway_credential_refresh_template(&gateway);
    let secret_refresh = secrets.projected_root.is_some().then(|| {
        let refresh_state = state.clone();
        let refresh_args = gateway_refresh_args(&args);
        let refresh_policy = policy.clone();
        let refresh_secrets = secrets.clone();
        let refresh_runtime_config = Arc::clone(&runtime_config);
        RuntimeGatewayCredentialRefreshPlan::new(
            gateway.credential_fingerprint,
            Arc::new(move || {
                gateway_config::resolve_gateway_refresh_candidate_for_service_mode(
                    &refresh_state,
                    &refresh_args,
                    &refresh_policy,
                    &refresh_secrets,
                    &refresh_runtime_config,
                    service_mode,
                    &refresh_template,
                )
            }),
        )
    });
    let request_constraints = gateway.request_constraints;
    let resolved_harness = gateway.resolved_harness;
    let provider_name = gateway.provider_name.unwrap_or("openai-compatible");
    let auth_required = gateway.auth_required;
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
    let runtime = start(
        options,
        Arc::clone(&runtime_config),
        secret_refresh,
        request_constraints,
        resolved_harness,
    )?;
    Ok((runtime, provider_name, auth_required))
}

pub(super) fn gateway_refresh_args(args: &GatewayArgs) -> GatewayArgs {
    GatewayArgs {
        command: None,
        listen: None,
        provider: args.provider,
        harness: args.harness,
        base_url: None,
        api_key: args.api_key.clone(),
        auth_token: args.auth_token.clone(),
        smart_context: false,
        presidio: false,
        no_presidio: false,
    }
}
