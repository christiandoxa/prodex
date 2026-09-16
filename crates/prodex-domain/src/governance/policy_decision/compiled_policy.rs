use std::fmt;

use crate::PolicyRevisionId;

use super::CompiledGovernancePolicy;

impl CompiledGovernancePolicy {
    pub fn revision(&self) -> PolicyRevisionId {
        self.revision
    }

    pub fn valid_until_unix_ms(&self) -> u64 {
        self.valid_until_unix_ms
    }

    pub fn is_valid_at(&self, now_unix_ms: u64) -> bool {
        now_unix_ms < self.valid_until_unix_ms
    }
}

impl fmt::Debug for CompiledGovernancePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledGovernancePolicy")
            .field("revision", &"<redacted>")
            .field("valid_until_unix_ms", &"<redacted>")
            .field("default_effect", &self.default_effect)
            .field("rule_count", &self.rules.len())
            .finish()
    }
}
