use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::*;

pub(super) fn runtime_gateway_admin_dashboard_response(
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    let html = runtime_gateway_admin_dashboard_html(shared);
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![
            (
                "content-type".to_string(),
                b"text/html; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
        ],
        body: html.into_bytes().into(),
    })
}

fn runtime_gateway_admin_dashboard_html(shared: &RuntimeLocalRewriteProxyShared) -> String {
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    let admin_prefix_json = serde_json::to_string(&admin_prefix)
        .unwrap_or_else(|_| "\"/v1/prodex/gateway\"".to_string());
    include_str!("gateway_admin_dashboard.html")
        .replace("__PRODEX_GATEWAY_ADMIN_PREFIX_JSON__", &admin_prefix_json)
}
