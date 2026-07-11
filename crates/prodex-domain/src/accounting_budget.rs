use crate::accounting::UsageAmount;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetLimit {
    pub max: UsageAmount,
    #[serde(default = "unlimited_request_budget")]
    pub max_requests: u64,
}

const fn unlimited_request_budget() -> u64 {
    u64::MAX
}

impl fmt::Debug for BudgetLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetLimit")
            .field("max", &"<redacted>")
            .finish()
    }
}

impl BudgetLimit {
    pub fn new(max_tokens: u64, max_cost_micros: u64) -> Self {
        Self {
            max: UsageAmount::new(max_tokens, max_cost_micros),
            max_requests: u64::MAX,
        }
    }

    pub fn with_max_requests(mut self, max_requests: u64) -> Self {
        self.max_requests = max_requests;
        self
    }
}
