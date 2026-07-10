use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_label, runtime_provider_model_from_body,
    runtime_provider_request_body_with_model,
};
use crate::RuntimeProxyRequest;
use std::collections::BTreeMap;

pub(super) const RUNTIME_LOCAL_REWRITE_MODEL_MEMORY_LIMIT: usize = 4096;

#[derive(Debug, Default)]
pub(super) struct RuntimeLocalRewriteModelMemoryState {
    selected_models: BTreeMap<String, String>,
}

pub(super) struct RuntimeLocalRewriteModelSelection {
    pub(super) model: String,
    pub(super) body: Vec<u8>,
}

pub(super) fn runtime_local_rewrite_model_selection(
    shared: &RuntimeLocalRewriteProxyShared,
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    body: &[u8],
    default_model: &str,
) -> RuntimeLocalRewriteModelSelection {
    let body_model = runtime_provider_model_from_body(body);
    let original_model = body_model
        .clone()
        .unwrap_or_else(|| default_model.to_string());
    let scope = runtime_local_rewrite_model_scope(provider_kind, request, body);
    let remembered_model = scope
        .as_deref()
        .and_then(|scope| shared.model_memory.lock().ok()?.selected_model(scope));
    let model = remembered_model
        .filter(|_| runtime_local_rewrite_model_allows_session_memory(&original_model))
        .unwrap_or_else(|| original_model.clone());
    if let (Some(scope), Some(explicit_model)) = (scope.as_deref(), body_model.as_deref())
        && !runtime_local_rewrite_model_allows_session_memory(explicit_model)
        && let Ok(mut memory) = shared.model_memory.lock()
    {
        memory.remember_selected_model(scope, explicit_model);
    }
    let body = if model != original_model {
        runtime_provider_request_body_with_model(body, &model)
    } else {
        body.to_vec()
    };
    RuntimeLocalRewriteModelSelection { model, body }
}

pub(super) fn runtime_local_rewrite_model_allows_session_memory(model: &str) -> bool {
    matches!(
        model.trim().to_ascii_lowercase().as_str(),
        "" | "auto" | "default"
    )
}

pub(super) fn runtime_local_rewrite_model_scope(
    provider_kind: RuntimeProviderBridgeKind,
    request: &RuntimeProxyRequest,
    body: &[u8],
) -> Option<String> {
    runtime_proxy_crate::runtime_request_explicit_session_id(request)
        .map(runtime_proxy_crate::RuntimeExplicitSessionId::into_string)
        .or_else(|| runtime_proxy_crate::runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(body)
                .ok()
                .and_then(|value| {
                    runtime_proxy_crate::runtime_request_session_id_from_value(&value)
                })
        })
        .map(|session_id| {
            format!(
                "{}:session:{session_id}",
                runtime_provider_label(provider_kind)
            )
        })
}

impl RuntimeLocalRewriteModelMemoryState {
    pub(super) fn remember_selected_model(&mut self, scope: &str, model: &str) {
        self.selected_models
            .insert(scope.to_string(), model.trim().to_string());
        while self.selected_models.len() > RUNTIME_LOCAL_REWRITE_MODEL_MEMORY_LIMIT {
            let Some(stale_scope) = self
                .selected_models
                .keys()
                .find(|candidate| candidate.as_str() != scope)
                .cloned()
            else {
                break;
            };
            self.selected_models.remove(&stale_scope);
        }
    }

    pub(super) fn selected_model(&self, scope: &str) -> Option<String> {
        self.selected_models.get(scope).cloned()
    }
}
