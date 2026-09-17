use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::provider_bridge::{RuntimeProviderRouteKind, runtime_provider_route_kind};
use crate::RuntimeProxyRequest;
use prodex_provider_core::{
    ProviderCapabilityStatus, ProviderEndpoint, ProviderErrorClass, ProviderId, provider_adapter,
};
use prodex_provider_spi::{
    ProviderRetryCause, ProviderRetryDecision, ProviderRetryPolicy, ProviderRetryStage,
    ProviderStreamMode, plan_provider_retry,
};
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeGatewayApplicationAdmission {
    endpoint: ProviderEndpoint,
    stream_mode: ProviderStreamMode,
}

impl RuntimeGatewayApplicationAdmission {
    pub(super) fn from_request(
        request: &RuntimeProxyRequest,
        shared: &RuntimeLocalRewriteProxyShared,
    ) -> Result<Self, &'static str> {
        let endpoint = runtime_gateway_provider_endpoint(&request.path_and_query)
            .ok_or("unsupported_provider_route")?;
        if provider_adapter(shared.provider.bridge_kind().provider_id()).capability_status(endpoint)
            == ProviderCapabilityStatus::Unsupported
        {
            return Err("unsupported_provider_route");
        }
        Ok(Self {
            endpoint,
            stream_mode: runtime_gateway_provider_stream_mode(request),
        })
    }
}

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayApplicationProviderDispatch<'a> {
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    stream_mode: ProviderStreamMode,
    _lifetime: PhantomData<&'a ()>,
}

impl RuntimeGatewayApplicationProviderDispatch<'_> {
    pub(in crate::runtime_launch::proxy_startup) fn provider(&self) -> ProviderId {
        self.provider
    }

    pub(in crate::runtime_launch::proxy_startup) fn endpoint(&self) -> ProviderEndpoint {
        self.endpoint
    }

    pub(in crate::runtime_launch::proxy_startup) fn stream_mode(&self) -> ProviderStreamMode {
        self.stream_mode
    }

    pub(in crate::runtime_launch::proxy_startup) fn selected_shared(
        &self,
        shared: &RuntimeLocalRewriteProxyShared,
    ) -> RuntimeLocalRewriteProxyShared {
        shared.clone()
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_provider_dispatch<
    'a,
>(
    admission: &'a RuntimeGatewayApplicationAdmission,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGatewayApplicationProviderDispatch<'a>, &'static str> {
    Ok(RuntimeGatewayApplicationProviderDispatch {
        provider: shared.provider.bridge_kind().provider_id(),
        endpoint: admission.endpoint,
        stream_mode: admission.stream_mode,
        _lifetime: PhantomData,
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_provider_retry_precommit(
    cause: ProviderRetryCause,
    error_class: ProviderErrorClass,
    attempt_index: usize,
    candidate_count: usize,
) -> bool {
    if candidate_count == 0 || attempt_index >= candidate_count {
        return false;
    }
    let policy = ProviderRetryPolicy::bounded(
        u8::try_from(candidate_count.saturating_sub(1)).unwrap_or(u8::MAX),
    );
    plan_provider_retry(
        policy,
        ProviderRetryStage::BeforeFirstByte,
        cause,
        error_class,
        u8::try_from(attempt_index).unwrap_or(u8::MAX),
    )
    .decision
        == ProviderRetryDecision::Allowed
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_stream_mode(
    request: &RuntimeProxyRequest,
) -> ProviderStreamMode {
    let streaming = serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|body| body.get("stream").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if streaming {
        ProviderStreamMode::Streaming
    } else {
        ProviderStreamMode::Unary
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_endpoint(
    path: &str,
) -> Option<ProviderEndpoint> {
    match runtime_provider_route_kind(path)? {
        RuntimeProviderRouteKind::Responses => Some(ProviderEndpoint::Responses),
        RuntimeProviderRouteKind::ResponsesCompact => Some(ProviderEndpoint::ResponsesCompact),
        RuntimeProviderRouteKind::ChatCompletions => Some(ProviderEndpoint::ChatCompletions),
        RuntimeProviderRouteKind::Messages => Some(ProviderEndpoint::Messages),
        RuntimeProviderRouteKind::Embeddings => Some(ProviderEndpoint::Embeddings),
        RuntimeProviderRouteKind::ModelsList | RuntimeProviderRouteKind::ModelsSingle(_) => {
            Some(ProviderEndpoint::Models)
        }
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_uses_compact_dispatch(
    path: &str,
) -> bool {
    matches!(
        runtime_provider_route_kind(path),
        Some(RuntimeProviderRouteKind::ResponsesCompact)
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_uses_models_dispatch(
    path: &str,
) -> bool {
    matches!(
        runtime_provider_route_kind(path),
        Some(RuntimeProviderRouteKind::ModelsList | RuntimeProviderRouteKind::ModelsSingle(_))
    )
}
