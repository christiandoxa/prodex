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
    use sha2::{Digest, Sha256};

    fn assert_local_refs_resolve(value: &serde_json::Value, spec: &serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str) {
                    let pointer = reference
                        .strip_prefix('#')
                        .expect("gateway OpenAPI refs must be local");
                    assert!(
                        spec.pointer(pointer).is_some(),
                        "unresolved OpenAPI ref {reference}"
                    );
                }
                for nested in object.values() {
                    assert_local_refs_resolve(nested, spec);
                }
            }
            serde_json::Value::Array(array) => {
                for nested in array {
                    assert_local_refs_resolve(nested, spec);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn openapi_components_contract_is_stable() {
        let serialized = serde_json::to_vec(&runtime_gateway_openapi_components()).unwrap();
        let digest = Sha256::digest(serialized);
        let digest = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            digest,
            "0793d38857e524a3a14fa77974ebaf7cc56d5a4b8afc9243de5faddbbbdbc3c0"
        );
    }

    #[test]
    fn openapi_spec_uses_supplied_mount_path() {
        let spec = runtime_gateway_openapi_spec_for_mount("/v1/");
        assert_eq!(spec["openapi"], "3.1.0");
        assert!(spec["paths"]["/v1/responses"].is_object());
        assert!(spec["paths"]["/v1/prodex/gateway/keys"].is_object());
        assert!(spec["paths"]["/v1/prodex/gateway/openapi.json"].is_object());
        assert!(spec["paths"]["/v1/prodex/gateway/routes/explain"].is_object());
    }

    #[test]
    fn openapi_documents_exact_gateway_identifier_boundaries() {
        let spec = runtime_gateway_openapi_spec_for_mount("/v1/");
        let schemas = &spec["components"]["schemas"];

        assert_eq!(schemas["GatewayExactIdentifier"]["minLength"], 1);
        assert_eq!(schemas["GatewayExactIdentifier"]["pattern"], "^\\S+$");
        assert_eq!(
            schemas["GatewayKeyCreateRequest"]["properties"]["name"]["$ref"],
            "#/components/schemas/GatewayExactIdentifier"
        );
        assert_eq!(
            schemas["GatewayKeyCreateRequest"]["properties"]["tenant_id"]["$ref"],
            "#/components/schemas/GatewayNullableExactIdentifier"
        );
        assert_eq!(
            schemas["GatewayKeyCreateRequest"]["properties"]["allowed_models"]["items"]["$ref"],
            "#/components/schemas/GatewayExactIdentifier"
        );
        assert_eq!(
            schemas["GatewayScimUserWrite"]["properties"]["userName"]["$ref"],
            "#/components/schemas/GatewayExactIdentifier"
        );
        assert_eq!(
            schemas["GatewayScimUserWrite"]["properties"]["allowed_key_prefixes"]["items"]["$ref"],
            "#/components/schemas/GatewayExactIdentifier"
        );
    }

    #[test]
    fn openapi_documents_route_explain_auth_schemas_and_errors() {
        let spec = runtime_gateway_openapi_spec_for_mount("/v1/");
        let post = &spec["paths"]["/v1/prodex/gateway/routes/explain"]["post"];
        let schemas = &spec["components"]["schemas"];

        assert_eq!(
            post["security"][0]["GatewayBearerAuth"],
            serde_json::json!([])
        );
        assert_eq!(
            post["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/GatewayRouteExplainRequest"
        );
        assert_eq!(
            post["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/GatewayRouteExplainResponse"
        );
        for status in ["400", "401", "403", "405", "413", "422", "503"] {
            assert_eq!(
                post["responses"][status]["$ref"],
                "#/components/responses/GatewayError"
            );
        }
        for schema in [
            "GatewayRouteExplainRequest",
            "GatewayRouteExplainResponse",
            "GatewayRouteConstraintPolicy",
            "GatewayRouteDecisionTrace",
            "GatewayRouteCandidateDecision",
            "GatewayRouteDecisionStage",
            "GatewayRouteDecisionReason",
        ] {
            assert!(schemas[schema].is_object(), "missing schema {schema}");
        }
        assert_eq!(
            schemas["GatewayRouteExplainResponse"]["properties"]["trace"]["$ref"],
            "#/components/schemas/GatewayRouteDecisionTrace"
        );
        let required = schemas["GatewayRouteExplainResponse"]["required"]
            .as_array()
            .unwrap();
        for field in [
            "diagnostic_seed",
            "hard_affinity_required",
            "hard_affinity_applied",
            "owner_model",
            "current_load_included",
            "health_quota_included",
            "omitted_candidates",
        ] {
            assert!(required.contains(&serde_json::json!(field)));
        }
        assert_eq!(
            schemas["GatewayRouteExplainResponse"]["properties"]["policy_adjustments"]["items"]["$ref"],
            "#/components/schemas/GatewayRouteOutputAdjustment"
        );
        assert_local_refs_resolve(&spec, &spec);
    }
}
