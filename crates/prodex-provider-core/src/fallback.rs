use crate::{PRODEX_ANTHROPIC_DEFAULT_MODEL, PRODEX_COPILOT_DEFAULT_MODEL, ProviderId};

pub fn provider_model_fallback_chain(provider: ProviderId, model: &str) -> Vec<String> {
    let model = model.trim();
    if let Some(chain) = combo_chain(model) {
        return chain;
    }
    let lower = model.to_ascii_lowercase();
    let chain: &[&str] = match provider {
        ProviderId::Anthropic => match lower.as_str() {
            "" | "auto" | "default" => &[
                PRODEX_ANTHROPIC_DEFAULT_MODEL,
                "claude-opus-4-8",
                "claude-haiku-4-5",
            ],
            "opus" | "best" => &["claude-opus-4-8", "claude-sonnet-4-6"],
            "sonnet" | "pro" => &["claude-sonnet-4-6", "claude-opus-4-8"],
            "haiku" | "flash" => &["claude-haiku-4-5", "claude-sonnet-4-6"],
            _ => return non_empty_single(model),
        },
        ProviderId::Copilot => match lower.as_str() {
            "" | "auto" | "default" => &[PRODEX_COPILOT_DEFAULT_MODEL, "gpt-5.1-codex", "gpt-4o"],
            "codex" | "pro" => &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "gpt-5.5" => &["gpt-5.5", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "gpt-5.4" => &["gpt-5.4", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "gpt-5.3-codex" => &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "claude" | "sonnet" => &["claude-sonnet-4-6", "gpt-5.3-codex", "gpt-5.1-codex"],
            "gemini" => &["gemini-3.1-pro-preview", "gpt-5.3-codex", "gpt-5.1-codex"],
            _ => return non_empty_single(model),
        },
        ProviderId::Gemini => match lower.as_str() {
            "chat-compression-default" => &[
                "gemini-3-pro-preview",
                "gemini-3-flash-preview",
                "gemini-2.5-pro",
                "gemini-2.5-flash",
            ],
            "" | "auto" | "auto-gemini-3" => &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "auto-gemini-2.5" => &["gemini-2.5-pro", "gemini-2.5-flash"],
            "pro" => &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
            ],
            "gemini-3.1-pro-preview-customtools" => &[
                "gemini-3.1-pro-preview-customtools",
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3.1-pro-preview" => &[
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3-pro-preview" => &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3.5-flash" => &[
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-3-flash-preview",
                "gemini-2.5-flash",
            ],
            "gemini-3-flash-preview" => &[
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "gemini-3-flash" => &["gemini-3-flash", "gemini-3.5-flash", "gemini-2.5-flash"],
            "gemini-3.1-flash-lite" => &[
                "gemini-3.1-flash-lite",
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
            ],
            "flash" => &[
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
            "flash-lite" => &["gemini-3.1-flash-lite", "gemini-2.5-flash-lite"],
            _ => return non_empty_single(model),
        },
        ProviderId::DeepSeek => match lower.as_str() {
            "" | "auto" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "pro" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "flash" => &["deepseek-v4-flash", "deepseek-v4-pro"],
            _ => return non_empty_single(model),
        },
        ProviderId::OpenAi | ProviderId::Local => return non_empty_single(model),
    };
    dedup_chain(chain.iter().map(|value| (*value).to_string()).collect())
}

fn non_empty_single(model: &str) -> Vec<String> {
    if model.is_empty() {
        Vec::new()
    } else {
        vec![model.to_string()]
    }
}

fn combo_chain(model: &str) -> Option<Vec<String>> {
    let chain = model.trim().strip_prefix("combo:")?;
    let models = chain
        .split([',', ';', '|', '>'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!models.is_empty()).then(|| dedup_chain(models))
}

fn dedup_chain(models: Vec<String>) -> Vec<String> {
    let mut seen = Vec::<String>::new();
    let mut deduped = Vec::new();
    for model in models {
        let key = model.to_ascii_lowercase();
        if seen.iter().any(|value| value == &key) {
            continue;
        }
        seen.push(key);
        deduped.push(model);
    }
    deduped
}
