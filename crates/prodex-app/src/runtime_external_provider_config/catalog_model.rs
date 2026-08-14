use super::ExternalCatalogProvider;
use prodex_provider_core::{ProviderReasoningEffort, provider_catalog_entry};
use serde_json::json;

const DEFAULT_REASONING_EFFORTS: [ProviderReasoningEffort; 4] = [
    ProviderReasoningEffort::Low,
    ProviderReasoningEffort::Medium,
    ProviderReasoningEffort::High,
    ProviderReasoningEffort::XHigh,
];

pub(super) fn external_catalog_model(
    provider: ExternalCatalogProvider,
    slug: &str,
    display_name: &str,
    description: &str,
    priority: usize,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> serde_json::Value {
    let kiro = matches!(provider, ExternalCatalogProvider::Kiro);
    let catalog_entry = provider_catalog_entry(provider.provider_id(), slug);
    let reasoning_efforts = catalog_entry
        .and_then(|entry| entry.supported_reasoning_efforts.as_deref())
        .unwrap_or(&DEFAULT_REASONING_EFFORTS);
    let supported_reasoning_levels = reasoning_efforts
        .iter()
        .filter_map(|effort| {
            let description = match effort {
                ProviderReasoningEffort::None => "No reasoning effort",
                ProviderReasoningEffort::Minimal => "Minimal reasoning effort",
                ProviderReasoningEffort::Low => "Low reasoning effort",
                ProviderReasoningEffort::Medium => "Medium reasoning effort",
                ProviderReasoningEffort::High => "High reasoning effort",
                ProviderReasoningEffort::XHigh => "Extra-high reasoning effort",
                ProviderReasoningEffort::Max => "Max reasoning effort",
                ProviderReasoningEffort::Unknown => return None,
            };
            Some(json!({ "effort": effort, "description": description }))
        })
        .collect::<Vec<_>>();
    let default_reasoning_level = catalog_entry
        .and_then(|entry| entry.default_reasoning_effort)
        .filter(|effort| *effort != ProviderReasoningEffort::Unknown)
        .unwrap_or(ProviderReasoningEffort::High);
    json!({
        "slug": slug,
        "display_name": display_name,
        "description": description,
        "default_reasoning_level": default_reasoning_level,
        "supported_reasoning_levels": supported_reasoning_levels,
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": priority,
        "additional_speed_tiers": [],
        "service_tiers": [],
        "default_service_tier": null,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": "",
        "supports_reasoning_summaries": !kiro,
        "supports_reasoning_summary_parameter": !kiro,
        "default_reasoning_summary": "none",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": "freeform",
        "web_search_tool_type": "text",
        "truncation_policy": {
            "mode": "tokens",
            "limit": 10000
        },
        "supports_parallel_tool_calls": true,
        "supports_image_detail_original": false,
        "context_window": context_window,
        "max_context_window": context_window,
        "auto_compact_token_limit": auto_compact_token_limit,
        "effective_context_window_percent": 95,
        "experimental_supported_tools": [],
        "input_modalities": (if kiro { json!(["text"]) } else { json!(["text", "image"]) }),
        "supports_search_tool": !kiro
    })
}
