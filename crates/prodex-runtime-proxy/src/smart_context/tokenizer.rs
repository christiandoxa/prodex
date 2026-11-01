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
    let (bpe, family) = match model.and_then(get_tokenizer) {
        Some(Tokenizer::O200kBase) => (tiktoken_rs::o200k_base_singleton(), "o200k_base"),
        Some(Tokenizer::Cl100kBase) => (tiktoken_rs::cl100k_base_singleton(), "cl100k_base"),
        Some(Tokenizer::O200kHarmony) => (tiktoken_rs::o200k_harmony_singleton(), "o200k_harmony"),
        _ => return estimated_count(body),
    };
    SmartContextTokenCount {
        // Literal special-token spellings inside JSON are data, not control tokens.
        tokens: bpe.count_ordinary(text) as u64,
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

        for (model, family) in [("gpt-4", "cl100k_base"), ("gpt-oss-120b", "o200k_harmony")] {
            let counted = smart_context_count_serialized_request(b"hello world", Some(model));
            assert!(counted.is_proven());
            assert_eq!(counted.tokenizer_family, Some(family));
        }

        let estimated =
            smart_context_count_serialized_request(b"hello world", Some("unknown-model"));
        assert!(!estimated.is_proven());
        assert!(estimated.error_bound_tokens > 0);
    }

    #[test]
    fn smart_context_audit_literal_special_tokens_are_ordinary_text() {
        let text = r#"{"input":"literal <|endoftext|> and <|fim_prefix|> are user data"}"#;
        let actual = smart_context_count_serialized_request(text.as_bytes(), Some("gpt-5.4"));
        let expected = tiktoken_rs::o200k_base_singleton().count_ordinary(text) as u64;
        assert_eq!(actual.tokens, expected);
    }
}
