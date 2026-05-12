//! Runtime store merge, compaction, and small persisted cache helpers.
//!
//! Runtime state save orchestration stays in the binary crate. This crate keeps
//! reusable merge/retention primitives and the smart-context artifact JSON cache
//! boundary.

use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeContinuationStore, RuntimeStateSaveSections,
    RuntimeStateSaveSelectedSnapshot, RuntimeStateSaveStateSection,
};
use prodex_state::{
    AppState, AppStateCompactionPolicy, ProfileEntry, ResponseProfileBinding,
    merge_profile_bindings, prune_profile_bindings,
    prune_profile_bindings_for_housekeeping_without_retention,
};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;

mod continuations;
mod profile_backoff;
mod runtime_state;
mod selected_snapshot;
mod smart_context_store;

pub use continuations::*;
pub use prodex_runtime_state::{RuntimeProfileBackoffs, RuntimeProfileHealth, RuntimeRouteKind};
pub use profile_backoff::*;
pub use runtime_state::*;
pub use selected_snapshot::*;
pub use smart_context_store::*;

pub const RUNTIME_SCORE_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
pub const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
pub const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
pub const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
pub const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
pub const RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 15 };
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS: i64 = if cfg!(test) { 320 } else { 600 };
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub const RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS: i64 = if cfg!(test) { 12 } else { 1_800 };
pub const RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE: u32 = 4;
pub const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION: u32 = 1;
pub const RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_BYTES: usize = 1_024;
pub const RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_TOKENS: usize = 256;

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
