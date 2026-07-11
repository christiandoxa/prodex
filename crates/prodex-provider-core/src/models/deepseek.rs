//! Deepseek provider model catalog.

use super::DEEPSEEK_CONTEXT_WINDOW_TOKENS;
use super::model;
use crate::CORE_TEXT_ENDPOINTS;
use crate::{ProviderId, ProviderModelSpec};

pub(super) const MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-v4-pro",
        "DeepSeek V4 Pro",
        "DeepSeek Pro alias routed through provider fallback.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro", "auto"]
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-v4-flash",
        "DeepSeek V4 Flash",
        "DeepSeek Flash alias routed through provider fallback.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-chat",
        "DeepSeek Chat",
        "DeepSeek chat-completions model.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-reasoner",
        "DeepSeek Reasoner",
        "DeepSeek reasoning model.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
];
