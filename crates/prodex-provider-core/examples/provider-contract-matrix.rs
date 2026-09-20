use prodex_provider_core::{
    ALL_PROVIDER_ENDPOINTS, PROVIDER_IMPLEMENTATION_ORDER, provider_adapter, provider_translator,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn main() {
    let providers = PROVIDER_IMPLEMENTATION_ORDER
        .iter()
        .copied()
        .map(|provider| {
            let adapter = provider_adapter(provider);
            let translator = provider_translator(provider);
            let endpoint_status = ALL_PROVIDER_ENDPOINTS
                .iter()
                .copied()
                .map(|endpoint| {
                    let support = translator.supported_params(endpoint, "");
                    let unsupported_params = support
                        .unsupported
                        .into_iter()
                        .map(|item| item.field)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    json!({
                        "endpoint": endpoint.label(),
                        "status": adapter.capability_status(endpoint).label(),
                        "unsupported_params": unsupported_params,
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "provider": provider.label(),
                "client_request_format": adapter.client_request_format().label(),
                "upstream_request_format": adapter.upstream_request_format().label(),
                "response_format": adapter.response_format().label(),
                "canonical_client_endpoint": adapter.canonical_client_endpoint(),
                "model_list_endpoint": adapter.model_list_endpoint(),
                "supports_streaming": adapter.supports_streaming(),
                "supports_model_fallback": adapter.supports_model_fallback(),
                "transform_status": adapter.transform_status().label(),
                "endpoint_status": endpoint_status,
            })
        })
        .collect::<Vec<Value>>();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "providers": providers }))
            .expect("provider contract matrix serializes")
    );
}
