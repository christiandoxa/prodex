//! Provider capability Markdown rendering.

use std::{collections::BTreeMap, fmt::Write as _};

use crate::{
    EffectiveHarnessMode, ProviderConformanceOperation, provider_conformance_cases,
    provider_contract_catalog,
};

pub fn provider_capabilities_markdown() -> String {
    let catalog = provider_contract_catalog(EffectiveHarnessMode::Native);
    let matrix = &catalog.providers;
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
    markdown.push_str("Generated from `prodex_provider_core::provider_contract_catalog()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.\n\n");
    markdown.push_str("| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | responses | responses/compact | chat-completions | messages | models | embeddings | images | audio | batches | rerank | a2a |\n");
    markdown.push_str("|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for contract in matrix {
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
    markdown.push_str("Model counts cover deterministic offline built-ins. Imported or provider-discovered runtime routes may augment them, and Super accepts an explicit non-empty custom child model ID without requiring live discovery.\n\n");
    markdown.push_str("## Harness modes\n\n");
    let _ = writeln!(
        markdown,
        "Default mode: `{}`. Resolved mode for this catalog: `{}`.\n",
        catalog.default_harness_mode, catalog.resolved_harness_mode
    );
    markdown.push_str("| Mode | Label | Selectable | Default effective | Canonical request routes | Request shaping | Response shaping | Stream shaping | Description |\n");
    markdown.push_str("|---|---|---|---|---|---|---|---|---|\n");
    for mode in catalog.harness_modes {
        let routes = mode
            .supported_canonical_request_routes
            .iter()
            .map(|route| route.label())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            mode.id,
            mode.display_label,
            mode.selectable,
            mode.default_effective_mode,
            routes,
            mode.request_shaping,
            mode.response_shaping,
            mode.stream_shaping,
            mode.description,
        );
    }
    markdown.push('\n');
    markdown.push_str("## Declared Responses parameter limitations\n\n");
    let mut wrote_limit = false;
    for contract in matrix {
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
    markdown.push_str("\n## Semantic compact observability\n\n");
    markdown.push_str("Gemini and Kiro semantic compact responses expose `x-prodex-compact-mode` (`semantic` or `local-fallback`) and `x-prodex-compact-provider`. Lossy fallback also exposes `x-prodex-compact-degraded: true` plus a bounded `x-prodex-compact-reason` code: `timeout`, `unsupported`, `unavailable`, `invalid-response`, `provider-error`, or `local-policy`. Raw upstream errors are never copied into headers.\n\n");
    markdown.push_str("Prometheus output includes `prodex_semantic_compact_total{provider,mode}` and `prodex_semantic_compact_fallback_total{provider,reason}` with fixed-cardinality labels. Local fallback preserves HTTP 200 for continuation compatibility but is not semantic success. It is intentionally lossy and retains at most 24 recent snippets, 768 bytes per snippet, and 24 KiB total.\n\n");
    markdown.push_str("## Transport limits\n\n");
    markdown.push_str("Capability labels describe documented HTTP/text transformations, not lossless equivalence. Translated or emulated shapes may reject unsupported fields as listed above. Gemini Live rejects unexpected upstream binary WebSocket frames predictably; it does not reinterpret them as text.\n");
    markdown
}
