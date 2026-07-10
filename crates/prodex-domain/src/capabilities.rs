use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    ResponsesApi,
    Streaming,
    Tools,
    Vision,
    JsonMode,
    RemoteCompact,
    WebSocket,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    capabilities: Vec<ModelCapability>,
}

impl fmt::Debug for CapabilitySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilitySet")
            .field("count", &self.capabilities.len())
            .finish()
    }
}

impl CapabilitySet {
    pub fn new(mut capabilities: Vec<ModelCapability>) -> Self {
        capabilities.sort();
        capabilities.dedup();
        Self { capabilities }
    }

    pub fn contains(&self, capability: ModelCapability) -> bool {
        self.capabilities.binary_search(&capability).is_ok()
    }

    pub fn missing_from(&self, offered: &CapabilitySet) -> Vec<ModelCapability> {
        self.capabilities
            .iter()
            .copied()
            .filter(|capability| !offered.contains(*capability))
            .collect()
    }

    pub fn as_slice(&self) -> &[ModelCapability] {
        &self.capabilities
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRouteCandidate {
    pub provider: String,
    pub model: String,
    pub capabilities: CapabilitySet,
}

impl fmt::Debug for ModelRouteCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelRouteCandidate")
            .field("provider", &"<redacted>")
            .field("model", &"<redacted>")
            .field("capabilities", &self.capabilities.as_slice())
            .finish()
    }
}

impl ModelRouteCandidate {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        capabilities: CapabilitySet,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            capabilities,
        }
    }

    pub fn is_well_formed(&self) -> bool {
        route_token_is_well_formed(&self.provider) && route_token_is_well_formed(&self.model)
    }
}

fn route_token_is_well_formed(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| character.is_ascii_graphic())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub required: CapabilitySet,
}

impl fmt::Debug for CapabilityRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilityRequest")
            .field("required", &"<redacted>")
            .finish()
    }
}

impl CapabilityRequest {
    pub fn new(required: CapabilitySet) -> Self {
        Self { required }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityDecision {
    Compatible(ModelRouteCandidate),
    Incompatible {
        candidate: ModelRouteCandidate,
        missing: Vec<ModelCapability>,
    },
    NoCandidate,
}

impl fmt::Debug for CapabilityDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compatible(_) => f.debug_tuple("Compatible").field(&"<redacted>").finish(),
            Self::Incompatible { .. } => f
                .debug_struct("Incompatible")
                .field("candidate", &"<redacted>")
                .field("missing", &"<redacted>")
                .finish(),
            Self::NoCandidate => f.write_str("NoCandidate"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityErrorStatus {
    UnprocessableRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityErrorResponsePlan {
    pub status: CapabilityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_capability_decision_error_response(
    decision: &CapabilityDecision,
) -> Option<CapabilityErrorResponsePlan> {
    match decision {
        CapabilityDecision::Compatible(_) => None,
        CapabilityDecision::Incompatible { .. } => Some(CapabilityErrorResponsePlan {
            status: CapabilityErrorStatus::UnprocessableRequest,
            code: "model_capability_unsupported",
            message: "requested model capabilities are not supported",
        }),
        CapabilityDecision::NoCandidate => Some(CapabilityErrorResponsePlan {
            status: CapabilityErrorStatus::ServiceUnavailable,
            code: "model_route_unavailable",
            message: "no compatible model route is available",
        }),
    }
}

pub fn negotiate_capability(
    request: &CapabilityRequest,
    candidates: &[ModelRouteCandidate],
) -> CapabilityDecision {
    let mut first_incompatible = None;
    for candidate in candidates {
        if !candidate.is_well_formed() {
            continue;
        }
        let missing = request.required.missing_from(&candidate.capabilities);
        if missing.is_empty() {
            return CapabilityDecision::Compatible(candidate.clone());
        }
        if first_incompatible.is_none() {
            first_incompatible = Some((candidate.clone(), missing));
        }
    }

    match first_incompatible {
        Some((candidate, missing)) => CapabilityDecision::Incompatible { candidate, missing },
        None => CapabilityDecision::NoCandidate,
    }
}
