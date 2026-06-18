use super::local_rewrite_gateway_openapi_components::runtime_gateway_openapi_components;
use super::local_rewrite_gateway_openapi_paths::runtime_gateway_openapi_paths_for_mount;

pub(super) fn runtime_gateway_openapi_spec_for_mount(mount_path: &str) -> serde_json::Value {
    let mount_path = mount_path.trim_end_matches('/');
    let paths = runtime_gateway_openapi_paths_for_mount(mount_path);

    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Prodex Gateway API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Local Prodex gateway admin and OpenAI-compatible response surface."
        },
        "servers": [{"url": mount_path}],
        "security": [{"GatewayBearerAuth": []}],
        "paths": paths,
        "components": runtime_gateway_openapi_components()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_uses_supplied_mount_path() {
        let spec = runtime_gateway_openapi_spec_for_mount("/v1/");
        assert_eq!(spec["openapi"], "3.1.0");
        assert!(spec["paths"]["/v1/responses"].is_object());
        assert!(spec["paths"]["/v1/prodex/gateway/keys"].is_object());
        assert!(spec["paths"]["/v1/prodex/gateway/openapi.json"].is_object());
    }
}
