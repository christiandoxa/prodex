//! Gemini provider model catalog data.

use super::super::GEMINI_CONTEXT_WINDOW_TOKENS;
use super::super::model;
use crate::{GEMINI_ENDPOINTS, PRODEX_GEMINI_CHAT_COMPRESSION_MODEL};
use crate::{ProviderId, ProviderModelSpec};

pub(in crate::models) const MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Gemini,
        "google",
        "auto",
        "Gemini Auto",
        "Gemini CLI-style auto routing through preview, pro, and flash fallback models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "auto-gemini-3",
        "Gemini 3 Auto",
        "Gemini CLI preview auto alias routed through Gemini 3 and stable fallback models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "auto-gemini-2.5",
        "Gemini 2.5 Auto",
        "Gemini CLI stable auto alias routed through Gemini 2.5 Pro and Flash models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "pro",
        "Gemini Pro",
        "Gemini Pro alias routed through preview and stable Pro models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "flash",
        "Gemini Flash",
        "Gemini Flash alias routed through preview and stable Flash models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "flash-lite",
        "Gemini Flash Lite",
        "Gemini Flash-Lite alias routed through Flash-Lite models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        PRODEX_GEMINI_CHAT_COMPRESSION_MODEL,
        "Gemini Chat Compression",
        "Gemini CLI semantic compaction alias routed through chat-compression-default.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-pro-preview-customtools",
        "Gemini 3.1 Pro Preview Custom Tools",
        "Gemini CLI preview Pro variant for custom tools routed through Prodex.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-pro-preview",
        "Gemini 3 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-flash-preview",
        "Gemini 3 Flash Preview",
        "Gemini CLI preview Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-flash",
        "Gemini 3 Flash",
        "Gemini CLI secondary Flash model name routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.5-flash",
        "Gemini 3.5 Flash",
        "Gemini CLI Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-flash-lite",
        "Gemini 3.1 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-pro",
        "Gemini 2.5 Pro",
        "Gemini CLI stable Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-flash",
        "Gemini 2.5 Flash",
        "Gemini CLI stable Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-flash-lite",
        "Gemini 2.5 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemma-4-31b-it",
        "Gemma 4 31B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemma-4-26b-a4b-it",
        "Gemma 4 26B A4B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
];
