use crate::{
    RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS, RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS,
    RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS, RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS,
    RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS, RuntimeDoctorSummary,
};

use super::super::marker_accessors::*;
use super::runtime_doctor_top_facet;

pub(super) fn runtime_doctor_selection_pressure(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS) {
        "elevated".to_string()
    } else {
        "low".to_string()
    }
}

pub(super) fn runtime_doctor_transport_pressure(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS) {
        "elevated".to_string()
    } else {
        "low".to_string()
    }
}

pub(super) fn runtime_doctor_persistence_pressure(summary: &RuntimeDoctorSummary) -> String {
    if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS) {
        "elevated".to_string()
    } else if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS) {
        "active".to_string()
    } else {
        "low".to_string()
    }
}

pub(super) fn runtime_doctor_startup_audit_pressure(summary: &RuntimeDoctorSummary) -> String {
    if !summary.orphan_managed_dirs.is_empty()
        || runtime_doctor_marker_count(summary, "runtime_proxy_startup_audit") > 0
            && summary
                .marker_last_fields
                .get("runtime_proxy_startup_audit")
                .is_some_and(|fields| {
                    fields
                        .get("missing_managed_dirs")
                        .is_some_and(|value| value != "0")
                        || fields
                            .get("orphan_managed_dirs")
                            .is_some_and(|value| value != "0")
                })
    {
        "elevated".to_string()
    } else {
        "low".to_string()
    }
}

pub(super) fn runtime_doctor_quota_freshness_pressure(summary: &RuntimeDoctorSummary) -> String {
    if summary.stale_persisted_usage_snapshots > 0
        || runtime_doctor_marker_count(summary, "profile_probe_refresh_error") > 0
        || runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0
        || runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0
        || runtime_doctor_top_facet(summary, "quota_source")
            .is_some_and(|value| value.starts_with("persisted_snapshot "))
    {
        "stale_risk".to_string()
    } else if runtime_doctor_has_any_markers(summary, RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS) {
        "active".to_string()
    } else {
        "low".to_string()
    }
}
