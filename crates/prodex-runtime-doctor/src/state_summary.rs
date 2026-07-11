use std::collections::BTreeMap;

#[cfg(test)]
use crate::RuntimeDoctorBindingProfileSummary;

mod bindings;
mod identity;
mod profiles;
mod quota;
mod routes;
mod runtime_adapters;

pub use bindings::*;
pub use identity::runtime_doctor_runtime_broker_mismatch_reason;
pub use profiles::{runtime_doctor_profile_summaries, runtime_doctor_route_circuit_state};
pub use quota::runtime_doctor_quota_freshness_label;
pub use routes::{runtime_doctor_degraded_routes, runtime_doctor_route_kind_label};
pub use runtime_adapters::{
    runtime_doctor_backoff_maps_from_runtime, runtime_doctor_health_scores_from_runtime,
    runtime_doctor_quota_window_status_from_runtime, runtime_doctor_usage_snapshot_from_runtime,
    runtime_doctor_usage_snapshots_from_runtime,
};

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorBackoffMaps<'a> {
    pub retry_backoff_until: &'a BTreeMap<String, i64>,
    pub transport_backoff_until: &'a BTreeMap<String, i64>,
    pub route_circuit_open_until: &'a BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorHealthScore {
    pub score: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorStateSummaryConfig {
    pub health_decay_seconds: i64,
    pub bad_pairing_decay_seconds: i64,
    pub performance_decay_seconds: i64,
    pub usage_snapshot_stale_grace_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDoctorRouteKind {
    Responses,
    Websocket,
    Compact,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDoctorQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeDoctorQuotaPressureBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorUsageSnapshot {
    pub checked_at: i64,
    pub five_hour_status: RuntimeDoctorQuotaWindowStatus,
    pub five_hour_remaining_percent: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: RuntimeDoctorQuotaWindowStatus,
    pub weekly_remaining_percent: i64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDoctorBinaryIdentity {
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

impl RuntimeDoctorBinaryIdentity {
    pub fn is_present(&self) -> bool {
        self.prodex_version.is_some()
            || self.executable_path.is_some()
            || self.executable_sha256.is_some()
    }
}

#[cfg(test)]
#[path = "../tests/src/state_summary.rs"]
mod tests;
