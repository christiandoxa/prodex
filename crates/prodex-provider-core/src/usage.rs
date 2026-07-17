use crate::ProviderModelCost;

mod estimate;

pub use self::estimate::{
    estimate_request_input_tokens, estimate_request_input_tokens_value, estimate_text_tokens,
};

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

pub fn extract_usage_tokens(body: &[u8]) -> ProviderTokenUsage {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return extract_usage_from_value(&value);
    }
    extract_usage_from_sse(body)
}

fn extract_usage_from_value(value: &serde_json::Value) -> ProviderTokenUsage {
    let usage = value
        .get("usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("usage"))
        })
        .or_else(|| {
            value
                .get("message")
                .and_then(|message| message.get("usage"))
        })
        .unwrap_or(value);
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

fn extract_usage_from_sse(body: &[u8]) -> ProviderTokenUsage {
    let Ok(text) = std::str::from_utf8(body) else {
        return ProviderTokenUsage::default();
    };
    let mut data_lines = Vec::new();
    let mut merged = ProviderTokenUsage::default();
    let flush = |data_lines: &mut Vec<&str>, merged: &mut ProviderTokenUsage| {
        if data_lines.is_empty() {
            return;
        }
        let data = data_lines.join("\n");
        data_lines.clear();
        if data.trim() == "[DONE]" {
            return;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        let usage = extract_usage_from_value(&value);
        if usage.input_tokens.is_some() {
            merged.input_tokens = usage.input_tokens;
        }
        if usage.output_tokens.is_some() {
            merged.output_tokens = usage.output_tokens;
        }
        if usage.total_tokens.is_some() {
            merged.total_tokens = usage.total_tokens;
        }
    };

    for line in text.lines() {
        if line.is_empty() || line == "\r" {
            flush(&mut data_lines, &mut merged);
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(
                data.strip_prefix(' ')
                    .unwrap_or(data)
                    .trim_end_matches('\r'),
            );
        }
    }
    flush(&mut data_lines, &mut merged);
    merged
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
    fn usage_parser_reads_final_sse_usage_without_counting_framing() {
        let usage = extract_usage_tokens(
            concat!(
                "event: response.in_progress\n",
                "data: {\"type\":\"response.in_progress\"}\n\n",
                "event: response.completed\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{",
                "\"input_tokens\":12,\"output_tokens\":7,\"total_tokens\":19}}}\n\n",
                "data: [DONE]\n\n"
            )
            .as_bytes(),
        );

        assert_eq!(
            usage,
            ProviderTokenUsage {
                input_tokens: Some(12),
                output_tokens: Some(7),
                total_tokens: Some(19),
            }
        );
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
