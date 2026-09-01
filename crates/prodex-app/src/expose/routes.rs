use super::http::{
    ExposeHttpRequest, expose_request_host_allowed, expose_single_header, handle_expose_input,
    handle_expose_session_exchange, handle_expose_session_revoke, handle_expose_session_rotate,
    handle_expose_stream,
};
use super::runtime::ExposeShared;
use super::ui::{expose_html_response, expose_js_response, expose_text_response};
use super::{EXPOSE_BASE_PATH, handle_mcp_route};
use std::sync::Arc;

pub(super) fn handle_expose_request(request: ExposeHttpRequest, shared: &Arc<ExposeShared>) {
    if !expose_request_host_allowed(&request, shared) {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    }
    let host = expose_single_header(&request, "Host")
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let mut request = request;
    if let Some(mcp) = shared.mcp.as_ref()
        && let Some(target) = mcp.openai_relay_target(request.target())
    {
        request.rewrite_target(target);
    }
    let path = request.target().to_string();
    if shared.is_mcp_only_host(&host) || request.target().starts_with("/pdx/v1/") {
        handle_mcp_route(request, shared, &host);
        return;
    }
    if path.contains('?') || path.contains('#') {
        let _ = request.respond(expose_text_response(404, "not found"));
        return;
    }
    match path.as_str() {
        EXPOSE_BASE_PATH if request.method() == "GET" => {
            let _ = request.respond(expose_html_response());
        }
        "/expose/app.js" if request.method() == "GET" => {
            let _ = request.respond(expose_js_response());
        }
        "/expose/session" => handle_expose_session_exchange(request, shared),
        "/expose/session/rotate" => handle_expose_session_rotate(request, shared),
        "/expose/session/revoke" => handle_expose_session_revoke(request, shared),
        "/expose/stream" => handle_expose_stream(request, shared),
        "/expose/input" => handle_expose_input(request, shared),
        _ => {
            let _ = request.respond(expose_text_response(404, "not found"));
        }
    }
}
