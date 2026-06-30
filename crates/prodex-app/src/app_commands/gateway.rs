use super::{
    GatewayArgs, GatewayCommands, GatewayProviderFilterArgs, GatewayProvidersArgs, Result,
};
use anyhow::{Context, anyhow};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderId, provider_adapter, provider_adapter_contract_matrix,
    provider_model_catalog_json,
};
use terminal_ui::print_stdout_line;

pub(crate) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    match &args.command {
        Some(GatewayCommands::Providers(command)) => handle_gateway_providers(command),
        Some(GatewayCommands::Capabilities(command)) => handle_gateway_capabilities(command),
        Some(GatewayCommands::Models(command)) => handle_gateway_models(command),
        None => super::runtime_launch::handle_gateway(args),
    }
}

fn handle_gateway_providers(args: &GatewayProvidersArgs) -> Result<()> {
    let providers = provider_adapter_contract_matrix();
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&providers)
                .context("failed to serialize provider contracts")?,
        );
        return Ok(());
    }
    for provider in providers {
        print_stdout_line(&format!(
            "{}: {}; models={}; endpoints={}",
            provider.provider,
            provider.transform_status,
            provider.model_count,
            provider.supported_endpoints.join(",")
        ));
    }
    Ok(())
}

fn handle_gateway_capabilities(args: &GatewayProviderFilterArgs) -> Result<()> {
    let provider = parse_gateway_provider(&args.provider)?;
    let spec = prodex_provider_core::provider_adapter_contract_spec(provider);
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&spec)
                .context("failed to serialize provider capabilities")?,
        );
        return Ok(());
    }
    print_stdout_line(&format!(
        "{}: {}; streaming={}; fallback={}",
        spec.provider, spec.transform_status, spec.supports_streaming, spec.supports_model_fallback
    ));
    for endpoint in spec.endpoint_status {
        print_stdout_line(&format!(
            "  {}: {}; streaming={}; tested={}",
            endpoint.endpoint, endpoint.status, endpoint.streaming, endpoint.tested
        ));
    }
    Ok(())
}

fn handle_gateway_models(args: &GatewayProviderFilterArgs) -> Result<()> {
    let provider = parse_gateway_provider(&args.provider)?;
    let models = provider_model_catalog_json(provider);
    if args.json {
        print_stdout_line(
            &serde_json::to_string_pretty(&models)
                .context("failed to serialize provider model catalog")?,
        );
        return Ok(());
    }
    let adapter = provider_adapter(provider);
    for model in adapter.model_catalog() {
        print_stdout_line(&format!(
            "{}: {} ({})",
            model.id,
            model.display_name,
            model
                .endpoints
                .iter()
                .map(|endpoint| endpoint.label())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    Ok(())
}

fn parse_gateway_provider(value: &str) -> Result<ProviderId> {
    ProviderId::parse(value).ok_or_else(|| {
        anyhow!(
            "unknown provider '{}'; expected openai, anthropic, copilot, deepseek, gemini, or local",
            value
        )
    })
}
