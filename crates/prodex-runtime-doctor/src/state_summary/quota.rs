use super::{RuntimeDoctorQuotaWindowStatus, RuntimeDoctorUsageSnapshot};

#[cfg(feature = "state-summary-mojo")]
use super::{RuntimeDoctorQuotaPressureBand, RuntimeDoctorRouteKind};

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_quota_status_code(status: RuntimeDoctorQuotaWindowStatus) -> i64 {
    match status {
        RuntimeDoctorQuotaWindowStatus::Ready => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_READY
        }
        RuntimeDoctorQuotaWindowStatus::Thin => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_THIN
        }
        RuntimeDoctorQuotaWindowStatus::Critical => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_CRITICAL
        }
        RuntimeDoctorQuotaWindowStatus::Exhausted => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_EXHAUSTED
        }
        RuntimeDoctorQuotaWindowStatus::Unknown => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN
        }
    }
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_quota_status_from_code(value: i64) -> RuntimeDoctorQuotaWindowStatus {
    match value {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_READY => {
            RuntimeDoctorQuotaWindowStatus::Ready
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_THIN => {
            RuntimeDoctorQuotaWindowStatus::Thin
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_CRITICAL => {
            RuntimeDoctorQuotaWindowStatus::Critical
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_EXHAUSTED => {
            RuntimeDoctorQuotaWindowStatus::Exhausted
        }
        _ => RuntimeDoctorQuotaWindowStatus::Unknown,
    }
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_quota_pressure_band_from_code(
    value: i64,
) -> RuntimeDoctorQuotaPressureBand {
    match value {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_READY => {
            RuntimeDoctorQuotaPressureBand::Healthy
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_THIN => {
            RuntimeDoctorQuotaPressureBand::Thin
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_CRITICAL => {
            RuntimeDoctorQuotaPressureBand::Critical
        }
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_EXHAUSTED => {
            RuntimeDoctorQuotaPressureBand::Exhausted
        }
        _ => RuntimeDoctorQuotaPressureBand::Unknown,
    }
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_quota_route_code(route_kind: RuntimeDoctorRouteKind) -> i64 {
    match route_kind {
        RuntimeDoctorRouteKind::Responses => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_ROUTE_RESPONSES
        }
        RuntimeDoctorRouteKind::Websocket => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_ROUTE_WEBSOCKET
        }
        RuntimeDoctorRouteKind::Compact => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_ROUTE_COMPACT
        }
        RuntimeDoctorRouteKind::Standard => {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_ROUTE_STANDARD
        }
    }
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_quota_pressure_band_reason(
    band: RuntimeDoctorQuotaPressureBand,
) -> &'static str {
    match band {
        RuntimeDoctorQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeDoctorQuotaPressureBand::Thin => "quota_thin",
        RuntimeDoctorQuotaPressureBand::Critical => "quota_critical",
        RuntimeDoctorQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeDoctorQuotaPressureBand::Unknown => "quota_unknown",
    }
}

#[cfg(feature = "state-summary-mojo")]
pub(super) fn runtime_doctor_quota_window_status_reason(
    status: RuntimeDoctorQuotaWindowStatus,
) -> &'static str {
    match status {
        RuntimeDoctorQuotaWindowStatus::Ready => "ready",
        RuntimeDoctorQuotaWindowStatus::Thin => "thin",
        RuntimeDoctorQuotaWindowStatus::Critical => "critical",
        RuntimeDoctorQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeDoctorQuotaWindowStatus::Unknown => "unknown",
    }
}

#[cfg(not(feature = "mojo"))]
fn runtime_doctor_usage_snapshot_hold_active(
    snapshot: &RuntimeDoctorUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeDoctorQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

#[cfg(not(feature = "mojo"))]
fn runtime_doctor_usage_snapshot_hold_expired(
    snapshot: &RuntimeDoctorUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeDoctorQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

#[cfg(not(feature = "mojo"))]
fn runtime_doctor_usage_snapshot_is_usable(
    snapshot: &RuntimeDoctorUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    if runtime_doctor_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_doctor_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_quota_freshness_label(
    snapshot: Option<&RuntimeDoctorUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> &'static str {
    match snapshot {
        Some(snapshot)
            if runtime_doctor_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds) =>
        {
            "fresh"
        }
        Some(_) => "stale",
        None => "missing",
    }
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_quota_freshness_label(
    snapshot: Option<&RuntimeDoctorUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> &'static str {
    let Some(snapshot) = snapshot else {
        return "missing";
    };
    let plan = prodex_mojo_core::rich::runtime_doctor_state_plan(
        prodex_mojo_core::rich::RuntimeDoctorStatePlanInput {
            operation: prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_OP_QUOTA,
            now,
            checked_at: snapshot.checked_at,
            five_hour_status: runtime_doctor_quota_status_code(snapshot.five_hour_status),
            five_hour_reset_at: snapshot.five_hour_reset_at,
            weekly_status: runtime_doctor_quota_status_code(snapshot.weekly_status),
            weekly_reset_at: snapshot.weekly_reset_at,
            stale_grace_seconds,
            decay_seconds: 1,
            ..prodex_mojo_core::rich::RuntimeDoctorStatePlanInput::default()
        },
    )
    .expect("Mojo runtime-doctor freshness plan returned invalid output");
    if plan.freshness == prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_READY {
        "fresh"
    } else {
        "stale"
    }
}
