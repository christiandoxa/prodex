use prodex_mojo_core::provider_constraints::{
    ProviderReasoningEffortClass, provider_reasoning_effort_class,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
    Ultra,
    Unknown,
}

impl ProviderReasoningEffort {
    pub(crate) fn parse(value: &str) -> Self {
        match provider_reasoning_effort_class(value)
            .expect("Mojo provider reasoning-effort classification failed")
        {
            ProviderReasoningEffortClass::None => Self::None,
            ProviderReasoningEffortClass::Minimal => Self::Minimal,
            ProviderReasoningEffortClass::Low => Self::Low,
            ProviderReasoningEffortClass::Medium => Self::Medium,
            ProviderReasoningEffortClass::High => Self::High,
            ProviderReasoningEffortClass::XHigh => Self::XHigh,
            ProviderReasoningEffortClass::Max => Self::Max,
            ProviderReasoningEffortClass::Ultra => Self::Ultra,
            ProviderReasoningEffortClass::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_effort_parse_uses_mojo_scalar_policy() {
        assert_eq!(
            ProviderReasoningEffort::parse(" XHIGH "),
            ProviderReasoningEffort::XHigh
        );
        assert_eq!(
            ProviderReasoningEffort::parse("\u{2003}ultra\u{2003}"),
            ProviderReasoningEffort::Ultra
        );
        assert_eq!(
            ProviderReasoningEffort::parse("not-an-effort"),
            ProviderReasoningEffort::Unknown
        );
    }
}
