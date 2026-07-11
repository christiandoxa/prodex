//! Openai provider model catalog.

use super::model;
use super::{OPENAI_CODEX_SPARK_CONTEXT_WINDOW_TOKENS, OPENAI_CONTEXT_WINDOW_TOKENS};
use crate::OPENAI_ENDPOINTS;
use crate::{ProviderId, ProviderModelSpec};

pub(super) const MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.4",
        "GPT-5.4",
        "Frontier agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["opus", "best", "default"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.4-mini",
        "GPT-5.4 Mini",
        "Smaller frontier agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["haiku", "mini"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.3-codex",
        "GPT-5.3 Codex",
        "Codex-optimized agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["codex", "sonnet"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.3-codex-spark",
        "GPT-5.3 Codex Spark",
        "Spark-tier Codex-optimized coding model routed through OpenAI Responses.",
        Some(OPENAI_CODEX_SPARK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["spark"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.2",
        "GPT-5.2",
        "General frontier model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        []
    ),
];
