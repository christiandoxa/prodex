//! Kiro provider model catalog.

use super::KIRO_CONTEXT_WINDOW_TOKENS;
use super::model;
use crate::KIRO_ENDPOINTS;
use crate::{ProviderId, ProviderModelSpec};

pub(super) const MODELS: &[ProviderModelSpec] = &[model!(
    ProviderId::Kiro,
    "kiro-cli",
    "auto",
    "Kiro Auto",
    "Kiro alias routed through the imported runtime model selection.",
    Some(KIRO_CONTEXT_WINDOW_TOKENS),
    None,
    None,
    KIRO_ENDPOINTS,
    ["default"]
)];
