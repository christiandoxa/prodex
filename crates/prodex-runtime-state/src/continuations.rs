use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeProfileBinding {
    pub profile_name: String,
    pub bound_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(serialize = "B: Serialize", deserialize = "B: Deserialize<'de>"))]
pub struct RuntimeContinuationJournal<B = RuntimeProfileBinding> {
    #[serde(default)]
    pub saved_at: i64,
    #[serde(default)]
    pub continuations: RuntimeContinuationStore<B>,
}

impl<B> Default for RuntimeContinuationJournal<B> {
    fn default() -> Self {
        Self {
            saved_at: 0,
            continuations: RuntimeContinuationStore::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(serialize = "B: Serialize", deserialize = "B: Deserialize<'de>"))]
pub struct RuntimeContinuationStore<B = RuntimeProfileBinding> {
    #[serde(default)]
    pub response_profile_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub session_profile_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub turn_state_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub session_id_bindings: BTreeMap<String, B>,
    #[serde(default)]
    pub statuses: RuntimeContinuationStatuses,
}

impl<B> Default for RuntimeContinuationStore<B> {
    fn default() -> Self {
        Self {
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            statuses: RuntimeContinuationStatuses::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContinuationStatuses {
    #[serde(default)]
    pub response: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    pub turn_state: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    pub session_id: BTreeMap<String, RuntimeContinuationBindingStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RuntimeContinuationBindingLifecycle {
    #[default]
    Warm,
    Verified,
    Suspect,
    Dead,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContinuationBindingStatus {
    #[serde(default)]
    pub state: RuntimeContinuationBindingLifecycle,
    #[serde(default)]
    pub confidence: u32,
    #[serde(default)]
    pub last_touched_at: Option<i64>,
    #[serde(default)]
    pub last_verified_at: Option<i64>,
    #[serde(default)]
    pub last_verified_route: Option<String>,
    #[serde(default)]
    pub last_not_found_at: Option<i64>,
    #[serde(default)]
    pub not_found_streak: u32,
    #[serde(default)]
    pub success_count: u32,
    #[serde(default)]
    pub failure_count: u32,
}
