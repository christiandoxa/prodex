//! Provider parameter support metadata.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderParamSupport {
    pub supported: bool,
    pub unsupported: Vec<ProviderUnsupportedReason>,
}

impl ProviderParamSupport {
    pub fn full() -> Self {
        Self {
            supported: true,
            unsupported: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderUnsupportedReason {
    pub field: String,
    pub reason: String,
}
