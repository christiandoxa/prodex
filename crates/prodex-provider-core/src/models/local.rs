//! Local provider model catalog.

use super::model;
use crate::OPENAI_ENDPOINTS;
use crate::{ProviderId, ProviderModelSpec};

pub(super) const MODELS: &[ProviderModelSpec] = &[model!(
    ProviderId::Local,
    "local",
    "local",
    "Local OpenAI Compatible",
    "Local OpenAI-compatible server default model.",
    None,
    None,
    None,
    OPENAI_ENDPOINTS,
    ["default"]
)];
