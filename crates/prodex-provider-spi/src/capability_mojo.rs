use super::ProviderRouteCapabilityCandidate;
use prodex_domain::{CapabilityRequest, ModelCapability};

pub(crate) struct CapabilityMatchPlan {
    pub(crate) first_compatible: Option<usize>,
    pub(crate) first_incompatible: Option<usize>,
}

pub(crate) fn match_candidates(
    request: &CapabilityRequest,
    candidates: &[ProviderRouteCapabilityCandidate],
) -> Option<CapabilityMatchPlan> {
    let well_formed = candidates
        .iter()
        .map(|candidate| candidate.model_candidate().is_well_formed())
        .collect::<Vec<_>>();
    let capability_masks = candidates
        .iter()
        .map(|candidate| capability_mask(&candidate.capabilities))
        .collect::<Vec<_>>();
    prodex_mojo_core::routing::capability_match_batch(
        &well_formed,
        &capability_masks,
        capability_mask(&request.required),
    )
    .map(|result| CapabilityMatchPlan {
        first_compatible: result.first_compatible,
        first_incompatible: result.first_incompatible,
    })
}

fn capability_mask(capabilities: &prodex_domain::CapabilitySet) -> u8 {
    capabilities.as_slice().iter().fold(0, |mask, capability| {
        mask | match capability {
            ModelCapability::ResponsesApi => 1 << 0,
            ModelCapability::Streaming => 1 << 1,
            ModelCapability::Tools => 1 << 2,
            ModelCapability::Vision => 1 << 3,
            ModelCapability::JsonMode => 1 << 4,
            ModelCapability::RemoteCompact => 1 << 5,
            ModelCapability::WebSocket => 1 << 6,
        }
    })
}
