use super::*;

mod input;
mod models;
mod output;
mod tools;

pub(super) use input::*;
pub(super) use models::*;
pub(super) use output::*;
pub(super) use tools::*;

pub(super) fn handle_runtime_proxy_anthropic_compat_request(
    request: &tiny_http::Request,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    let method = request.method().as_str();
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }

    match path {
        "/" => Some(build_runtime_proxy_json_response(
            200,
            serde_json::json!({
                "service": "prodex",
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            })
            .to_string(),
        )),
        RUNTIME_PROXY_ANTHROPIC_HEALTH_PATH => Some(build_runtime_proxy_json_response(
            200,
            serde_json::json!({
                "status": "ok",
            })
            .to_string(),
        )),
        RUNTIME_PROXY_ANTHROPIC_MODELS_PATH => Some(build_runtime_proxy_json_response(
            200,
            runtime_proxy_anthropic_models_list().to_string(),
        )),
        _ => runtime_proxy_anthropic_model_id_from_path(path).map(|model_id| {
            build_runtime_proxy_json_response(
                200,
                runtime_proxy_anthropic_model_descriptor(model_id).to_string(),
            )
        }),
    }
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeAnthropicMessagesRequest {
    pub(super) translated_request: RuntimeProxyRequest,
    pub(super) requested_model: String,
    pub(super) stream: bool,
    pub(super) want_thinking: bool,
    pub(super) server_tools: RuntimeAnthropicServerTools,
    pub(super) carried_web_search_requests: u64,
    pub(super) carried_web_fetch_requests: u64,
    pub(super) carried_code_execution_requests: u64,
    pub(super) carried_tool_search_requests: u64,
}
