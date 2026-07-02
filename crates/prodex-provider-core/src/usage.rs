use crate::ProviderModelCost;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderTokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

impl ProviderTokenUsage {
    pub fn merged_total(self) -> Option<u64> {
        self.total_tokens.or_else(|| {
            Some(
                self.input_tokens?
                    .saturating_add(self.output_tokens.unwrap_or_default()),
            )
        })
    }
}

pub fn estimate_request_input_tokens(body: &[u8]) -> u64 {
    if body.is_empty() {
        return 0;
    }
    let parsed = serde_json::from_slice::<serde_json::Value>(body).ok();
    let estimated = parsed
        .as_ref()
        .and_then(estimate_semantic_request_tokens)
        .unwrap_or_else(|| estimate_text_tokens(&String::from_utf8_lossy(body)));
    estimated.max(1)
}

fn estimate_semantic_request_tokens(value: &serde_json::Value) -> Option<u64> {
    let mut total = 0_u64;
    if let Some(object) = value.as_object() {
        for key in [
            "input",
            "messages",
            "contents",
            "content",
            "parts",
            "prompt",
            "system",
            "instructions",
            "tools",
        ] {
            if let Some(value) = object.get(key) {
                total = total.saturating_add(estimate_value_tokens(value));
            }
        }
    }
    (total > 0).then_some(total)
}

fn estimate_value_tokens(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::String(text) => estimate_text_tokens(text),
        serde_json::Value::Array(values) => values.iter().fold(0_u64, |sum, value| {
            sum.saturating_add(estimate_value_tokens(value))
        }),
        serde_json::Value::Object(object) => object.iter().fold(0_u64, |sum, (key, value)| {
            if matches!(
                key.as_str(),
                "model"
                    | "role"
                    | "type"
                    | "id"
                    | "name"
                    | "metadata"
                    | "temperature"
                    | "top_p"
                    | "stream"
            ) {
                sum
            } else {
                sum.saturating_add(estimate_value_tokens(value))
            }
        }),
        _ => 0,
    }
}

pub fn estimate_text_tokens(text: &str) -> u64 {
    let significant = text.chars().filter(|ch| !ch.is_control()).count() as u64;
    significant.saturating_add(3) / 4
}

pub fn extract_usage_tokens(body: &[u8]) -> ProviderTokenUsage {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return ProviderTokenUsage::default();
    };
    extract_usage_from_value(&value)
}

fn extract_usage_from_value(value: &serde_json::Value) -> ProviderTokenUsage {
    let usage = value.get("usage").unwrap_or(value);
    let input_tokens = first_u64(
        usage,
        &[
            "input_tokens",
            "prompt_tokens",
            "promptTokens",
            "inputTokens",
            "cache_creation_input_tokens",
        ],
    )
    .or_else(|| {
        value
            .get("usageMetadata")
            .and_then(|usage| first_u64(usage, &["promptTokenCount"]))
    });
    let output_tokens = first_u64(
        usage,
        &[
            "output_tokens",
            "completion_tokens",
            "completionTokens",
            "outputTokens",
        ],
    )
    .or_else(|| {
        value
            .get("usageMetadata")
            .and_then(|usage| first_u64(usage, &["candidatesTokenCount"]))
    });
    let total_tokens = first_u64(usage, &["total_tokens", "totalTokens"]).or_else(|| {
        value
            .get("usageMetadata")
            .and_then(|usage| first_u64(usage, &["totalTokenCount"]))
    });
    ProviderTokenUsage {
        input_tokens,
        output_tokens,
        total_tokens,
    }
}

fn first_u64(value: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_u64))
}

pub fn calculate_cost_microusd(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost: ProviderModelCost,
) -> Option<u64> {
    let mut total = 0_u64;
    let mut known = false;
    if let (Some(tokens), Some(rate)) = (input_tokens, cost.input_cost_per_million_microusd) {
        total = total.saturating_add(tokens.saturating_mul(rate) / 1_000_000);
        known = true;
    }
    if let (Some(tokens), Some(rate)) = (output_tokens, cost.output_cost_per_million_microusd) {
        total = total.saturating_add(tokens.saturating_mul(rate) / 1_000_000);
        known = true;
    }
    known.then_some(total)
}

pub fn microusd_to_usd(value: u64) -> f64 {
    value as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_token_estimator_ignores_json_scaffolding() {
        let body = br#"{"model":"x","messages":[{"role":"user","content":"hello world from prodex"}],"stream":true}"#;
        let semantic = estimate_request_input_tokens(body);
        let raw = estimate_text_tokens(&String::from_utf8_lossy(body));
        assert!(semantic < raw);
        assert!(semantic > 0);
    }

    #[test]
    fn usage_parser_reads_openai_and_gemini_shapes() {
        let openai = extract_usage_tokens(
            br#"{"usage":{"input_tokens":10,"output_tokens":20,"total_tokens":30}}"#,
        );
        assert_eq!(openai.input_tokens, Some(10));
        assert_eq!(openai.output_tokens, Some(20));
        assert_eq!(openai.total_tokens, Some(30));

        let gemini = extract_usage_tokens(
            br#"{"usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":22,"totalTokenCount":33}}"#,
        );
        assert_eq!(gemini.input_tokens, Some(11));
        assert_eq!(gemini.output_tokens, Some(22));
        assert_eq!(gemini.total_tokens, Some(33));
    }

    #[test]
    fn cost_calc_uses_micro_usd_rates() {
        let cost = ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        };
        assert_eq!(
            calculate_cost_microusd(Some(1_000), Some(2_000), cost),
            Some(5_000)
        );
        assert_eq!(microusd_to_usd(5_000), 0.005);
    }
}
