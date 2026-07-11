use super::super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use runtime_proxy_crate::path_without_query;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct RuntimeKiroProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) model_catalog: Vec<serde_json::Value>,
    pub(crate) command: Option<PathBuf>,
}

pub(crate) fn runtime_kiro_model_catalog_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Vec<serde_json::Value> {
    let RuntimeLocalRewriteProviderOptions::Kiro { auth } = provider else {
        return Vec::new();
    };
    auth.model_catalog.clone()
}

pub(crate) fn runtime_kiro_models_buffered_response(
    auth: &RuntimeKiroProfileAuth,
    method: &str,
    path_and_query: &str,
) -> Option<RuntimeHeapTrimmedBufferedResponseParts> {
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    let path = path_without_query(path_and_query);
    if path.ends_with("/models") {
        return Some(runtime_kiro_json_parts(
            200,
            prodex_provider_core::kiro_provider_core_model_list_value(&auth.model_catalog),
        ));
    }
    let model_id = path.split("/models/").nth(1)?.trim();
    if model_id.is_empty() {
        return None;
    }
    let (status, body) = prodex_provider_core::kiro_provider_core_model_value_or_not_found(
        &auth.model_catalog,
        model_id,
    );
    Some(runtime_kiro_json_parts(status, body))
}

pub(crate) fn runtime_kiro_json_parts(
    status: u16,
    body: Value,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}
