//! Local provider model catalog.

use super::model;
use crate::{ALL_PROVIDER_ENDPOINTS, ProviderId, ProviderModelSpec};

pub(super) const MODELS: &[ProviderModelSpec] = &[model!(
    ProviderId::Local,
    "local",
    "local",
    "Local OpenAI Compatible",
    "Local OpenAI-compatible server default model.",
    None,
    None,
    None,
    ALL_PROVIDER_ENDPOINTS,
    ["default"]
)];
