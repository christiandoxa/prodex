use super::gateway_config::gateway_api_keys_from_list;
use super::selection::RuntimeLaunchSelection;
use super::*;

pub(crate) fn start_mem0_memory_gateway_for_runtime_request(
    paths: &AppPaths,
    request: &RuntimeLaunchRequest<'_>,
    gateway_token: &str,
) -> Result<RuntimeRotationProxy> {
    let state = AppState::load(paths)?;
    let selection = RuntimeLaunchSelection::resolve(
        paths,
        &state,
        request.profile,
        request.model_provider_override,
        request.profile_v2_name,
        request.external_provider,
        request.external_provider_api_key,
    )?;
    let provider = runtime_mem0_gateway_provider_options(&state, &selection, request)?;
    let upstream_base_url = local_rewrite_proxy_upstream_base_url(&selection, request)
        .or_else(|| request.base_url.map(str::to_string))
        .unwrap_or_else(|| quota_base_url(request.base_url));
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state: &state,
        upstream_base_url,
        provider,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: request.model_context_window_tokens,
        preferred_listen_addr: Some("0.0.0.0:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
}

fn runtime_mem0_gateway_provider_options(
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeLocalRewriteProviderOptions> {
    if selection.non_openai_model_provider.is_some() {
        return runtime_local_rewrite_provider_options(state, selection, request);
    }

    let api_keys = runtime_mem0_openai_api_keys(&selection.codex_home)?;
    if api_keys.is_empty() {
        return Ok(RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly {
            embedding_model: "text-embedding-3-small".to_string(),
        });
    }
    Ok(RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys })
}

fn runtime_mem0_openai_api_keys(codex_home: &Path) -> Result<Vec<String>> {
    if let Some(keys) = env::var("OPENAI_API_KEYS")
        .ok()
        .and_then(|value| gateway_api_keys_from_list(&value))
    {
        return Ok(keys);
    }
    if let Some(key) = env::var("OPENAI_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(vec![key]);
    }
    let Some(auth_json) = read_auth_json_text(codex_home)? else {
        return Ok(Vec::new());
    };
    let value: serde_json::Value =
        serde_json::from_str(&auth_json).context("failed to parse selected profile auth.json")?;
    let key = value
        .get("OPENAI_API_KEY")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(key.into_iter().collect())
}
