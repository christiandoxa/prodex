use super::smart_context_estimate_tokens_from_body;
use tiktoken_rs::tokenizer::{Tokenizer, get_tokenizer};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SmartContextTokenCountSource {
    TokenizerCounted,
    #[default]
    Estimated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTokenCount {
    pub tokens: u64,
    pub source: SmartContextTokenCountSource,
    pub tokenizer_family: Option<&'static str>,
    pub confidence_basis_points: u16,
    pub error_bound_tokens: u64,
}

impl SmartContextTokenCount {
    pub fn is_proven(&self) -> bool {
        self.source == SmartContextTokenCountSource::TokenizerCounted
    }
}

impl SmartContextTokenCountSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TokenizerCounted => "tokenizer_counted",
            Self::Estimated => "estimated",
        }
    }
}

pub fn smart_context_count_serialized_request(
    body: &[u8],
    model: Option<&str>,
) -> SmartContextTokenCount {
    let Some(text) = std::str::from_utf8(body).ok() else {
        return estimated_count(body);
    };
    let Some(tokenizer) = model.and_then(get_tokenizer) else {
        return estimated_count(body);
    };
    let family = tokenizer_family(tokenizer);
    let Ok(bpe) = tiktoken_rs::bpe_for_tokenizer(tokenizer) else {
        return estimated_count(body);
    };
    SmartContextTokenCount {
        tokens: bpe.encode_with_special_tokens(text).len() as u64,
        source: SmartContextTokenCountSource::TokenizerCounted,
        tokenizer_family: Some(family),
        confidence_basis_points: 10_000,
        error_bound_tokens: 0,
    }
}

fn estimated_count(body: &[u8]) -> SmartContextTokenCount {
    let tokens = smart_context_estimate_tokens_from_body(body);
    SmartContextTokenCount {
        tokens,
        source: SmartContextTokenCountSource::Estimated,
        tokenizer_family: None,
        confidence_basis_points: 5_000,
        error_bound_tokens: tokens.saturating_add(3) / 4,
    }
}

fn tokenizer_family(tokenizer: Tokenizer) -> &'static str {
    match tokenizer {
        Tokenizer::O200kHarmony => "o200k_harmony",
        Tokenizer::O200kBase => "o200k_base",
        Tokenizer::Cl100kBase => "cl100k_base",
        Tokenizer::P50kBase => "p50k_base",
        Tokenizer::R50kBase => "r50k_base",
        Tokenizer::P50kEdit => "p50k_edit",
        Tokenizer::Gpt2 => "gpt2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_model_uses_real_tokenizer_and_unknown_model_is_labeled_estimated() {
        let counted =
            smart_context_count_serialized_request(br#"{"input":"hello world"}"#, Some("gpt-5.4"));
        assert!(counted.is_proven());
        assert_eq!(counted.tokenizer_family, Some("o200k_base"));
        assert_eq!(counted.error_bound_tokens, 0);

        let estimated =
            smart_context_count_serialized_request(b"hello world", Some("unknown-model"));
        assert!(!estimated.is_proven());
        assert!(estimated.error_bound_tokens > 0);
    }
}
