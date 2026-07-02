mod deepseek;
mod gemini;
mod kiro;
mod passthrough;

use crate::ProviderId;
use crate::translator::{ProviderConformanceCase, ProviderTranslator};

pub use deepseek::DeepSeekTranslator;
pub use gemini::GeminiTranslator;
pub use kiro::KiroTranslator;
pub use passthrough::PassthroughTranslator;

static OPENAI_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::OpenAi);
static ANTHROPIC_TRANSLATOR: PassthroughTranslator =
    PassthroughTranslator::new(ProviderId::Anthropic);
static COPILOT_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::Copilot);
static DEEPSEEK_TRANSLATOR: DeepSeekTranslator = DeepSeekTranslator;
static GEMINI_TRANSLATOR: GeminiTranslator = GeminiTranslator;
static KIRO_TRANSLATOR: KiroTranslator = KiroTranslator;
static LOCAL_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::Local);

pub fn provider_translator(provider: ProviderId) -> &'static dyn ProviderTranslator {
    match provider {
        ProviderId::OpenAi => &OPENAI_TRANSLATOR,
        ProviderId::Anthropic => &ANTHROPIC_TRANSLATOR,
        ProviderId::Copilot => &COPILOT_TRANSLATOR,
        ProviderId::DeepSeek => &DEEPSEEK_TRANSLATOR,
        ProviderId::Gemini => &GEMINI_TRANSLATOR,
        ProviderId::Kiro => &KIRO_TRANSLATOR,
        ProviderId::Local => &LOCAL_TRANSLATOR,
    }
}

pub fn provider_conformance_cases() -> &'static [ProviderConformanceCase] {
    static CASES: std::sync::OnceLock<Vec<ProviderConformanceCase>> = std::sync::OnceLock::new();
    CASES
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../tests/fixtures/provider_conformance_cases.json"
            ))
            .expect("provider conformance fixtures should parse")
        })
        .as_slice()
}
