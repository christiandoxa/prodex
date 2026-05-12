use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProfileBackoffs {
    pub retry_backoff_until: BTreeMap<String, i64>,
    pub transport_backoff_until: BTreeMap<String, i64>,
    pub route_circuit_open_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProfileHealth {
    pub score: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProfileHealthSnapshot {
    pub score: u32,
    pub updated_at: i64,
}

pub trait RuntimeProfileHealthEntry {
    fn runtime_profile_health_score(&self) -> u32;
    fn runtime_profile_health_updated_at(&self) -> i64;
}

impl RuntimeProfileHealthEntry for RuntimeProfileHealth {
    fn runtime_profile_health_score(&self) -> u32 {
        self.score
    }

    fn runtime_profile_health_updated_at(&self) -> i64 {
        self.updated_at
    }
}

impl RuntimeProfileHealthEntry for RuntimeProfileHealthSnapshot {
    fn runtime_profile_health_score(&self) -> u32 {
        self.score
    }

    fn runtime_profile_health_updated_at(&self) -> i64 {
        self.updated_at
    }
}
