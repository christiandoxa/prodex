use serde_json::{Map, Value};

const CANONICAL_MOUNT_PATH: &str = "/v1";
const VERSION_PLACEHOLDER: &str = "__PRODEX_VERSION__";
const CHECKED_OPENAPI_DOCUMENT: &str = include_str!("local_rewrite_gateway_openapi.json");

pub(super) fn runtime_gateway_openapi_spec_for_mount(mount_path: &str) -> Value {
    let mount_path = mount_path.trim_end_matches('/');
    let mut document: Value = serde_json::from_str(CHECKED_OPENAPI_DOCUMENT)
        .expect("checked gateway OpenAPI document must be valid JSON");

    let version = document
        .pointer_mut("/info/version")
        .expect("checked gateway OpenAPI document must contain info.version");
    assert_eq!(
        version.as_str(),
        Some(VERSION_PLACEHOLDER),
        "checked gateway OpenAPI document must retain its version placeholder"
    );
    *version = Value::String(env!("CARGO_PKG_VERSION").to_owned());
    *document
        .pointer_mut("/servers/0/url")
        .expect("checked gateway OpenAPI document must contain servers[0].url") =
        Value::String(mount_path.to_owned());

    let paths = document
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .expect("checked gateway OpenAPI document must contain paths");
    *paths = remount_paths(std::mem::take(paths), mount_path);
    remount_local_path_refs(&mut document, mount_path);
    document
}

fn remount_paths(paths: Map<String, Value>, mount_path: &str) -> Map<String, Value> {
    paths
        .into_iter()
        .map(|(path, operation)| {
            let path = path
                .strip_prefix(CANONICAL_MOUNT_PATH)
                .filter(|suffix| suffix.starts_with('/'))
                .map_or(path.clone(), |suffix| format!("{mount_path}{suffix}"));
            (path, operation)
        })
        .collect()
}

fn remount_local_path_refs(value: &mut Value, mount_path: &str) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object
                .get("$ref")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                const CANONICAL_PREFIX: &str = "#/paths/~1v1~1";
                if let Some(suffix) = reference.strip_prefix(CANONICAL_PREFIX) {
                    let encoded_mount = mount_path.replace('~', "~0").replace('/', "~1");
                    object["$ref"] = Value::String(format!("#/paths/{encoded_mount}~1{suffix}"));
                }
            }
            for nested in object.values_mut() {
                remount_local_path_refs(nested, mount_path);
            }
        }
        Value::Array(array) => {
            for nested in array {
                remount_local_path_refs(nested, mount_path);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const COMPONENTS_DIGEST: &str =
        "0793d38857e524a3a14fa77974ebaf7cc56d5a4b8afc9243de5faddbbbdbc3c0";
    const DOCUMENT_DIGEST: &str =
        "11f3647119871fa3e1172f901f43dcbe0255b98c8afd52ddbfc595ba636418da";

    fn digest(value: &Value) -> String {
        Sha256::digest(serde_json::to_vec(value).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn assert_local_refs_resolve(value: &Value, spec: &Value) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
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
            Value::Array(array) => {
                for nested in array {
                    assert_local_refs_resolve(nested, spec);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn checked_openapi_document_is_exact_and_deterministic() {
        let mut expected: Value = serde_json::from_str(CHECKED_OPENAPI_DOCUMENT).unwrap();
        assert_eq!(digest(&expected), DOCUMENT_DIGEST);
        expected["info"]["version"] = Value::String(env!("CARGO_PKG_VERSION").to_owned());
        let first = runtime_gateway_openapi_spec_for_mount(CANONICAL_MOUNT_PATH);
        let second = runtime_gateway_openapi_spec_for_mount(CANONICAL_MOUNT_PATH);

        assert_eq!(first, expected);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert_eq!(first["info"]["version"], env!("CARGO_PKG_VERSION"));
        assert_local_refs_resolve(&first, &first);
    }

    #[test]
    fn openapi_components_contract_is_stable() {
        let spec = runtime_gateway_openapi_spec_for_mount(CANONICAL_MOUNT_PATH);
        assert_eq!(digest(&spec["components"]), COMPONENTS_DIGEST);
    }

    #[test]
    fn openapi_spec_uses_supplied_mount_path() {
        let canonical = runtime_gateway_openapi_spec_for_mount(CANONICAL_MOUNT_PATH);
        let remounted = runtime_gateway_openapi_spec_for_mount("/gateway/");

        assert_eq!(remounted["servers"][0]["url"], "/gateway");
        assert_eq!(
            canonical["paths"].as_object().unwrap().len(),
            remounted["paths"].as_object().unwrap().len()
        );
        for (path, operation) in canonical["paths"].as_object().unwrap() {
            let remounted_path = path
                .strip_prefix(CANONICAL_MOUNT_PATH)
                .filter(|suffix| suffix.starts_with('/'))
                .map_or(path.clone(), |suffix| format!("/gateway{suffix}"));
            let mut expected = operation.clone();
            remount_local_path_refs(&mut expected, "/gateway");
            assert_eq!(&remounted["paths"][remounted_path], &expected);
        }
        assert!(
            remounted["paths"]
                .as_object()
                .unwrap()
                .keys()
                .all(|path| !path.starts_with("/v1/"))
        );
        assert_local_refs_resolve(&remounted, &remounted);

        let root = runtime_gateway_openapi_spec_for_mount("/");
        assert_eq!(root["servers"][0]["url"], "");
        assert!(root["paths"]["/responses"].is_object());
        assert!(root["paths"]["/prodex/gateway/openapi.json"].is_object());
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
    }

    #[test]
    fn openapi_documents_policy_lifecycle_preconditions() {
        let spec = runtime_gateway_openapi_spec_for_mount("/v1/");
        let paths = &spec["paths"];
        for path in [
            "/v1/prodex/gateway/policies",
            "/v1/prodex/gateway/policies/validate",
            "/v1/prodex/gateway/policies/status",
            "/v1/prodex/gateway/policies/{revision_id}",
            "/v1/prodex/gateway/policies/{revision_id}/submit",
            "/v1/prodex/gateway/policies/{revision_id}/approvals/{approval_id}/votes",
            "/v1/prodex/gateway/policies/{revision_id}/activate",
            "/v1/prodex/gateway/policies/{revision_id}/rollback",
            "/v1/prodex/gateway/governance/outbox",
            "/v1/prodex/gateway/governance/outbox/claim",
            "/v1/prodex/gateway/governance/audit/integrity",
        ] {
            assert!(
                paths[path].is_object(),
                "missing policy lifecycle path {path}"
            );
        }
        for action in ["activate", "rollback"] {
            let post =
                &paths[format!("/v1/prodex/gateway/policies/{{revision_id}}/{action}")]["post"];
            let names = post["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|parameter| parameter["name"].as_str())
                .collect::<Vec<_>>();
            assert!(names.contains(&"Idempotency-Key"));
            assert!(names.contains(&"If-Match"));
            assert!(post["responses"]["412"].is_object());
            assert!(post["responses"]["428"].is_object());
        }
    }

    #[test]
    fn openapi_documents_every_governance_artifact_lifecycle() {
        let spec = runtime_gateway_openapi_spec_for_mount("/v1/");
        let paths = &spec["paths"];
        for resource in [
            "classification-rules",
            "provider-registries",
            "routing-scores",
        ] {
            for suffix in [
                "",
                "/validate",
                "/status",
                "/{revision_id}",
                "/{revision_id}/submit",
                "/{revision_id}/approvals/{approval_id}/votes",
                "/{revision_id}/activate",
                "/{revision_id}/rollback",
            ] {
                let path = format!("/v1/prodex/gateway/{resource}{suffix}");
                assert!(paths[&path].is_object(), "missing governance path {path}");
            }
        }
    }

    #[test]
    fn checked_artifact_uses_expected_template_markers() {
        let artifact: Value = serde_json::from_str(CHECKED_OPENAPI_DOCUMENT).unwrap();
        assert_eq!(artifact["info"]["version"], VERSION_PLACEHOLDER);
        assert_eq!(artifact["servers"][0]["url"], CANONICAL_MOUNT_PATH);
        assert!(artifact["paths"].as_object().unwrap().keys().all(|path| {
            matches!(path.as_str(), "/livez" | "/readyz" | "/startupz") || path.starts_with("/v1/")
        }));
    }
}
