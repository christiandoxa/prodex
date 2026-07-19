//! Provider adapter contract and body-transform metadata.

use serde::Serialize;

use super::{ProviderId, ProviderWireFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransformPhase {
    ClientRequestToUpstream,
    UpstreamResponseToClient,
    UpstreamStreamEventToClient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBodyTransform {
    pub phase: ProviderTransformPhase,
    pub provider: ProviderId,
    pub from_format: ProviderWireFormat,
    pub to_format: ProviderWireFormat,
    pub body: Vec<u8>,
    pub lossy: bool,
}
