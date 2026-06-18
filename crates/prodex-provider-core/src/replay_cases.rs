use crate::ProviderId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderReplayCase {
    pub provider: ProviderId,
    pub model: &'static str,
    pub request_body: &'static [u8],
    pub response_body: &'static [u8],
    pub expected_input_tokens: Option<u64>,
    pub expected_output_tokens: Option<u64>,
}

const PROVIDER_REPLAY_CASES: &[ProviderReplayCase] = &[
    ProviderReplayCase {
        provider: ProviderId::OpenAi,
        model: "gpt-5.4",
        request_body: br#"{"model":"gpt-5.4","input":"hello openai","stream":false}"#,
        response_body: br#"{"usage":{"input_tokens":11,"output_tokens":7,"total_tokens":18}}"#,
        expected_input_tokens: Some(11),
        expected_output_tokens: Some(7),
    },
    ProviderReplayCase {
        provider: ProviderId::Anthropic,
        model: "auto",
        request_body: br#"{"model":"auto","input":"hello anthropic","stream":true}"#,
        response_body: br#"{"usage":{"input_tokens":13,"output_tokens":8}}"#,
        expected_input_tokens: Some(13),
        expected_output_tokens: Some(8),
    },
    ProviderReplayCase {
        provider: ProviderId::Copilot,
        model: "codex",
        request_body: br#"{"model":"codex","input":"hello copilot"}"#,
        response_body: br#"{"usage":{"prompt_tokens":17,"completion_tokens":9,"total_tokens":26}}"#,
        expected_input_tokens: Some(17),
        expected_output_tokens: Some(9),
    },
    ProviderReplayCase {
        provider: ProviderId::DeepSeek,
        model: "flash",
        request_body: br#"{"model":"flash","messages":[{"role":"user","content":"hello deepseek"}]}"#,
        response_body: br#"{"usage":{"prompt_tokens":19,"completion_tokens":10}}"#,
        expected_input_tokens: Some(19),
        expected_output_tokens: Some(10),
    },
    ProviderReplayCase {
        provider: ProviderId::Gemini,
        model: "flash",
        request_body: br#"{"model":"flash","input":"hello gemini"}"#,
        response_body: br#"{"usageMetadata":{"promptTokenCount":23,"candidatesTokenCount":12,"totalTokenCount":35}}"#,
        expected_input_tokens: Some(23),
        expected_output_tokens: Some(12),
    },
    ProviderReplayCase {
        provider: ProviderId::Local,
        model: "local",
        request_body: br#"{"model":"local","input":"hello local"}"#,
        response_body: br#"{"usage":{"input_tokens":5,"output_tokens":3}}"#,
        expected_input_tokens: Some(5),
        expected_output_tokens: Some(3),
    },
];

pub fn provider_replay_cases() -> &'static [ProviderReplayCase] {
    PROVIDER_REPLAY_CASES
}

pub(crate) fn provider_replay_case_count(provider: ProviderId) -> usize {
    PROVIDER_REPLAY_CASES
        .iter()
        .filter(|case| case.provider == provider)
        .count()
}
