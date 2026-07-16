use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct RecoveredLoad<T> {
    pub value: T,
    pub recovered_from_backup: bool,
}

#[derive(Debug, Clone)]
pub struct RecoveredVersionedLoad<T> {
    pub value: T,
    pub generation: u64,
    pub recovered_from_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedJson<T> {
    #[serde(default)]
    pub generation: u64,
    pub value: T,
}
