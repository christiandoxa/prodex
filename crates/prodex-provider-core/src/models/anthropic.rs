//! Anthropic provider model catalog.

use super::ANTHROPIC_CONTEXT_WINDOW_TOKENS;
use super::model;
use crate::CORE_TEXT_ENDPOINTS;
use crate::{ProviderId, ProviderModelSpec};

pub(super) const MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "auto",
        "Anthropic Auto",
        "Anthropic alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "opus",
        "Claude Opus",
        "Anthropic Opus alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["best"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "sonnet",
        "Claude Sonnet",
        "Anthropic Sonnet alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "haiku",
        "Claude Haiku",
        "Anthropic Haiku alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-8",
        "Claude Opus 4.8",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-sonnet-4-6",
        "Claude Sonnet 4.6",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-haiku-4-5",
        "Claude Haiku 4.5",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-6",
        "Claude Opus 4.6",
        "Anthropic compatibility model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-20250514",
        "Claude Opus 4",
        "Anthropic compatibility model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
];
