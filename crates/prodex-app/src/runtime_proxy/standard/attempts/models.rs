//! Standard-route model metadata response helpers.

use super::*;

pub(super) fn forward_runtime_standard_success_response(
    shared: &RuntimeRotationProxyShared,
    request: &RuntimeProxyRequest,
    response: reqwest::Response,
) -> Result<tiny_http::ResponseBox> {
    if !runtime_openai_models_metadata_path(&request.path_and_query) {
        return await_runtime_proxy_async_task(
            shared,
            "standard_forward_response",
            forward_runtime_proxy_response(response, Vec::new()),
        );
    }

    let parts = await_runtime_proxy_async_task(
        shared,
        "standard_buffer_models_response",
        buffer_runtime_proxy_async_response_parts(response, Vec::new()),
    )?;
    Ok(build_runtime_proxy_response_from_parts(
        runtime_patch_openai_spark_models_response(parts),
    ))
}

fn runtime_openai_models_metadata_path(path_and_query: &str) -> bool {
    let normalized = runtime_proxy_normalize_openai_path(path_and_query);
    let path = path_without_query(normalized.as_ref()).trim_end_matches('/');
    path == "/models" || path == "/v1/models" || path.ends_with("/codex/models")
}

fn runtime_patch_openai_spark_models_response(
    mut parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&parts.body) else {
        return parts;
    };
    let Some(models) = value
        .get_mut("models")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return parts;
    };

    if let Some(model) = models
        .iter_mut()
        .find(|model| runtime_model_catalog_entry_matches_slug(model, "gpt-5.3-codex-spark"))
    {
        runtime_patch_openai_spark_model_entry(model);
    } else if let Some(mut model) = ["gpt-5.3-codex", "gpt-5.4", "gpt-5.5"]
        .iter()
        .find_map(|slug| {
            models
                .iter()
                .find(|model| runtime_model_catalog_entry_matches_slug(model, slug))
                .cloned()
        })
        .or_else(|| models.first().cloned())
    {
        runtime_patch_openai_spark_model_entry(&mut model);
        models.push(model);
    }

    if let Ok(body) = serde_json::to_vec(&value) {
        parts.body = body.into();
    }
    parts
}

fn runtime_model_catalog_entry_matches_slug(model: &serde_json::Value, slug: &str) -> bool {
    model
        .get("slug")
        .or_else(|| model.get("id"))
        .or_else(|| model.get("model"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case(slug))
}

fn runtime_patch_openai_spark_model_entry(model: &mut serde_json::Value) {
    let Some(object) = model.as_object_mut() else {
        return;
    };
    object.insert(
        "slug".to_string(),
        serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
    );
    if object.contains_key("id") {
        object.insert(
            "id".to_string(),
            serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
        );
    }
    if object.contains_key("model") {
        object.insert(
            "model".to_string(),
            serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
        );
    }
    object.insert(
        "display_name".to_string(),
        serde_json::Value::String("gpt-5.3-codex-spark".to_string()),
    );
    object.insert("context_window".to_string(), serde_json::json!(128_000));
    object.insert("max_context_window".to_string(), serde_json::json!(128_000));
    object.insert(
        "auto_compact_token_limit".to_string(),
        serde_json::json!(115_200),
    );
    object.insert(
        "effective_context_window_percent".to_string(),
        serde_json::json!(95),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response_parts(body: serde_json::Value) -> RuntimeHeapTrimmedBufferedResponseParts {
        RuntimeHeapTrimmedBufferedResponseParts {
            status: 200,
            headers: Vec::new(),
            body: serde_json::to_vec(&body).unwrap().into(),
        }
    }

    #[test]
    fn openai_models_response_adds_spark_context_from_codex_metadata() {
        let parts = runtime_patch_openai_spark_models_response(response_parts(json!({
            "models": [{
                "slug": "gpt-5.3-codex",
                "display_name": "gpt-5.3-codex",
                "context_window": 272000,
                "max_context_window": 272000,
                "auto_compact_token_limit": null,
                "effective_context_window_percent": 95
            }]
        })));
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let spark = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["slug"] == "gpt-5.3-codex-spark")
            .unwrap();

        assert_eq!(spark["context_window"], 128_000);
        assert_eq!(spark["max_context_window"], 128_000);
        assert_eq!(spark["auto_compact_token_limit"], 115_200);
    }

    #[test]
    fn openai_models_response_adds_spark_when_codex_metadata_is_absent() {
        let parts = runtime_patch_openai_spark_models_response(response_parts(json!({
            "models": [{
                "slug": "gpt-5.4",
                "display_name": "gpt-5.4",
                "context_window": 272000,
                "max_context_window": 1000000,
                "auto_compact_token_limit": null,
                "effective_context_window_percent": 95
            }]
        })));
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let spark = value["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["slug"] == "gpt-5.3-codex-spark")
            .unwrap();

        assert_eq!(spark["display_name"], "gpt-5.3-codex-spark");
        assert_eq!(spark["context_window"], 128_000);
        assert_eq!(spark["max_context_window"], 128_000);
        assert_eq!(spark["auto_compact_token_limit"], 115_200);
    }

    #[test]
    fn openai_models_response_corrects_existing_spark_context() {
        let parts = runtime_patch_openai_spark_models_response(response_parts(json!({
            "models": [{
                "slug": "gpt-5.3-codex-spark",
                "display_name": "spark",
                "context_window": 272000,
                "max_context_window": 272000,
                "auto_compact_token_limit": 258400,
                "effective_context_window_percent": 95
            }]
        })));
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let spark = &value["models"][0];

        assert_eq!(spark["display_name"], "gpt-5.3-codex-spark");
        assert_eq!(spark["context_window"], 128_000);
        assert_eq!(spark["auto_compact_token_limit"], 115_200);
    }

    #[test]
    fn openai_models_metadata_path_matches_codex_and_openai_routes() {
        assert!(runtime_openai_models_metadata_path(
            "/backend-api/codex/models?client_version=0.124.0"
        ));
        assert!(runtime_openai_models_metadata_path(
            "/backend-api/prodex/models?client_version=0.142.5"
        ));
        assert!(runtime_openai_models_metadata_path(
            "/v1/models?client_version=0.124.0"
        ));
        assert!(!runtime_openai_models_metadata_path("/v1/responses"));
    }
}
