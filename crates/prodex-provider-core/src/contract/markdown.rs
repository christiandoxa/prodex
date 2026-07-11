//! Provider capability Markdown rendering.

use std::{collections::BTreeMap, fmt::Write as _};

use crate::{
    ProviderConformanceOperation, provider_adapter_contract_matrix, provider_conformance_cases,
};

pub fn provider_capabilities_markdown() -> String {
    let matrix = provider_adapter_contract_matrix();
    let mut fixture_counts: BTreeMap<&'static str, (usize, usize, usize)> = BTreeMap::new();
    for case in provider_conformance_cases() {
        let entry = fixture_counts.entry(case.provider.label()).or_default();
        match case.operation {
            ProviderConformanceOperation::Request => entry.0 += 1,
            ProviderConformanceOperation::Response => entry.1 += 1,
            ProviderConformanceOperation::StreamEvent => entry.2 += 1,
        }
    }

    let mut markdown = String::new();
    markdown.push_str("# Provider Capabilities\n\n");
    markdown.push_str("Generated from `prodex_provider_core::provider_adapter_contract_matrix()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.\n\n");
    markdown.push_str("| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | responses | responses/compact | chat-completions | messages | models | embeddings | images | audio | batches | rerank | a2a |\n");
    markdown.push_str("|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for contract in &matrix {
        let counts = fixture_counts
            .get(contract.provider)
            .copied()
            .unwrap_or_default();
        let mut endpoint_status = BTreeMap::new();
        for endpoint in &contract.endpoint_status {
            endpoint_status.insert(endpoint.endpoint, endpoint.status);
        }
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {}/{}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            contract.provider,
            contract.model_count,
            contract.transform_status,
            contract.supports_streaming,
            contract.supports_model_fallback,
            counts.0,
            counts.1,
            counts.2,
            endpoint_status
                .get("responses")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("responses/compact")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("chat-completions")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("messages")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("models")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("embeddings")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("images")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("audio")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("batches")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status
                .get("rerank")
                .copied()
                .unwrap_or("unsupported"),
            endpoint_status.get("a2a").copied().unwrap_or("unsupported"),
        );
    }
    markdown.push_str("\nStatus values: `native`, `translated`, `passthrough`, `emulated`, `partial`, `untested`, `unsupported`.\n\n");
    markdown.push_str("Fixture summary counts are `request/response/stream-event` conformance cases per provider.\n\n");
    markdown.push_str("## Declared Responses parameter limitations\n\n");
    let mut wrote_limit = false;
    for contract in &matrix {
        let Some(responses) = contract
            .endpoint_status
            .iter()
            .find(|endpoint| endpoint.endpoint == "responses")
        else {
            continue;
        };
        if responses.unsupported_params.is_empty() {
            continue;
        }
        wrote_limit = true;
        let _ = writeln!(
            markdown,
            "- `{}`: `{}`",
            contract.provider,
            responses.unsupported_params.join("`, `")
        );
    }
    if !wrote_limit {
        markdown.push_str("- none\n");
    }
    markdown
}
