//! Provider model surface metadata types.

use serde::Serialize;

use super::{ProviderEndpoint, ProviderId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProviderModelCost {
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
}

impl ProviderModelCost {
    pub const fn any(self) -> bool {
        self.input_cost_per_million_microusd.is_some()
            || self.output_cost_per_million_microusd.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProviderModelSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub provider: ProviderId,
    pub owned_by: &'static str,
    pub context_window_tokens: Option<u64>,
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub endpoints: &'static [ProviderEndpoint],
    pub aliases: &'static [&'static str],
}

impl ProviderModelSpec {
    pub const fn cost(self) -> ProviderModelCost {
        ProviderModelCost {
            input_cost_per_million_microusd: self.input_cost_per_million_microusd,
            output_cost_per_million_microusd: self.output_cost_per_million_microusd,
        }
    }

    pub fn matches_id_or_alias(self, model: &str) -> bool {
        let model = model.trim();
        self.id.eq_ignore_ascii_case(model)
            || self
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(model))
    }
}
