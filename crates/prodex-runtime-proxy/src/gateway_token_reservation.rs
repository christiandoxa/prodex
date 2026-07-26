pub fn runtime_gateway_hard_limit_reserved_tokens(
    provider: prodex_provider_core::ProviderId,
    model: &str,
    body: &[u8],
) -> u64 {
    let input_tokens = prodex_provider_core::estimate_request_input_tokens(body);
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(output_tokens) =
            prodex_provider_core::provider_requested_output_tokens_compat(&value)
    {
        return input_tokens.saturating_add(output_tokens);
    }

    let model_ceiling = |model: &str| {
        let entry = prodex_provider_core::provider_catalog_entry(provider, model)?;
        let context_ceiling = entry
            .context_window_tokens
            .map(|tokens| tokens.max(input_tokens));
        let output_ceiling = entry
            .max_output_tokens
            .map(|tokens| input_tokens.saturating_add(tokens));
        match (context_ceiling, output_ceiling) {
            (Some(context), Some(output)) => Some(context.min(output)),
            (Some(tokens), None) | (None, Some(tokens)) => Some(tokens),
            (None, None) => None,
        }
    };

    if let Some(models) = model.trim().strip_prefix("combo:") {
        return models
            .split(',')
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(model_ceiling)
            .collect::<Option<Vec<_>>>()
            .and_then(|ceilings| ceilings.into_iter().max())
            .unwrap_or(u64::MAX);
    }
    model_ceiling(model).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserves_explicit_output_or_model_ceiling() {
        let reserve = |provider, model, body: &[u8]| {
            runtime_gateway_hard_limit_reserved_tokens(provider, model, body)
        };
        assert_eq!(
            reserve(
                prodex_provider_core::ProviderId::OpenAi,
                "gpt-5.4",
                br#"{"model":"gpt-5.4","input":"hello from prodex","max_output_tokens":17}"#,
            ),
            22
        );
        assert_eq!(
            reserve(
                prodex_provider_core::ProviderId::OpenAi,
                "gpt-5.4",
                br#"{"model":"gpt-5.4","input":"hello from prodex"}"#,
            ),
            400_000
        );
        assert_eq!(
            reserve(
                prodex_provider_core::ProviderId::Copilot,
                "combo:gpt-5.3-codex,gemini-2.5-pro",
                br#"{"model":"prodex-fallback","input":"hello from prodex"}"#,
            ),
            1_048_576
        );
        assert_eq!(
            reserve(
                prodex_provider_core::ProviderId::OpenAi,
                "unknown-model",
                br#"{"model":"unknown-model","input":"hello from prodex"}"#,
            ),
            u64::MAX
        );
    }
}
