//! Runtime broker registry, health, and metrics transfer models.
//!
//! This crate intentionally stays side-effect-free. The binary crate still owns
//! broker leases, proxy handles, and registry file I/O.

use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeProfileHealth, RuntimeStateLockWaitMetrics,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

pub const RUNTIME_BROKER_HEALTH_PATH: &str = "/__prodex/runtime/health";
pub const RUNTIME_BROKER_METRICS_PATH: &str = "/__prodex/runtime/metrics";
pub const RUNTIME_BROKER_METRICS_PROMETHEUS_PATH: &str = "/__prodex/runtime/metrics/prometheus";
pub const RUNTIME_BROKER_ACTIVATE_PATH: &str = "/__prodex/runtime/activate";
pub const RUNTIME_BROKER_ADMIN_TOKEN_HEADER: &str = "X-Prodex-Admin-Token";

mod admin;
mod continuity;
mod metrics;
mod process;
mod registry;
mod version_guard;

pub use admin::*;
pub use continuity::*;
pub use metrics::*;
pub use process::*;
pub use registry::*;
pub use version_guard::*;
#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
