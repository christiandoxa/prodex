use super::{
    RuntimeDoctorQuotaPressureBand, RuntimeDoctorQuotaWindowStatus, RuntimeDoctorRouteKind,
    RuntimeDoctorUsageSnapshot,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeDoctorQuotaWindowSummary {
    pub(super) status: RuntimeDoctorQuotaWindowStatus,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeDoctorQuotaSummary {
    pub(super) five_hour: RuntimeDoctorQuotaWindowSummary,
    pub(super) weekly: RuntimeDoctorQuotaWindowSummary,
    pub(super) route_band: RuntimeDoctorQuotaPressureBand,
}

#[cfg(not(feature = "mojo"))]
fn runtime_doctor_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeDoctorQuotaWindowStatus,
    _remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeDoctorQuotaWindowSummary {
    if reset_at != i64::MAX && reset_at <= now {
        return RuntimeDoctorQuotaWindowSummary {
            status: RuntimeDoctorQuotaWindowStatus::Ready,
        };
    }
    RuntimeDoctorQuotaWindowSummary { status }
}

#[cfg(not(feature = "mojo"))]
fn runtime_doctor_quota_pressure_band_from_window_status(
    status: RuntimeDoctorQuotaWindowStatus,
) -> RuntimeDoctorQuotaPressureBand {
    match status {
        RuntimeDoctorQuotaWindowStatus::Ready => RuntimeDoctorQuotaPressureBand::Healthy,
        RuntimeDoctorQuotaWindowStatus::Thin => RuntimeDoctorQuotaPressureBand::Thin,
        RuntimeDoctorQuotaWindowStatus::Critical => RuntimeDoctorQuotaPressureBand::Critical,
        RuntimeDoctorQuotaWindowStatus::Exhausted => RuntimeDoctorQuotaPressureBand::Exhausted,
        RuntimeDoctorQuotaWindowStatus::Unknown => RuntimeDoctorQuotaPressureBand::Unknown,
    }
}

#[cfg(not(feature = "mojo"))]
pub(super) fn runtime_doctor_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeDoctorUsageSnapshot,
    route_kind: RuntimeDoctorRouteKind,
    now: i64,
) -> RuntimeDoctorQuotaSummary {
    let five_hour = runtime_doctor_quota_window_summary_from_usage_snapshot_at(
        snapshot.five_hour_status,
        snapshot.five_hour_remaining_percent,
        snapshot.five_hour_reset_at,
        now,
    );
    let weekly = runtime_doctor_quota_window_summary_from_usage_snapshot_at(
        snapshot.weekly_status,
        snapshot.weekly_remaining_percent,
        snapshot.weekly_reset_at,
        now,
    );
    let route_band = [
        five_hour.status,
        weekly.status,
        match route_kind {
            RuntimeDoctorRouteKind::Responses | RuntimeDoctorRouteKind::Websocket => weekly.status,
            RuntimeDoctorRouteKind::Compact | RuntimeDoctorRouteKind::Standard => five_hour.status,
        },
    ]
    .into_iter()
    .map(runtime_doctor_quota_pressure_band_from_window_status)
    .fold(
        RuntimeDoctorQuotaPressureBand::Healthy,
        RuntimeDoctorQuotaPressureBand::max,
    );
    RuntimeDoctorQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

#[cfg(feature = "mojo")]
fn runtime_doctor_quota_status_code(status: RuntimeDoctorQuotaWindowStatus) -> i64 {
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

#[cfg(feature = "mojo")]
fn runtime_doctor_quota_status_from_code(value: i64) -> RuntimeDoctorQuotaWindowStatus {
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

#[cfg(feature = "mojo")]
fn runtime_doctor_quota_route_code(route_kind: RuntimeDoctorRouteKind) -> i64 {
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

#[cfg(feature = "mojo")]
pub(super) fn runtime_doctor_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeDoctorUsageSnapshot,
    route_kind: RuntimeDoctorRouteKind,
    now: i64,
) -> RuntimeDoctorQuotaSummary {
    let plan = prodex_mojo_core::rich::runtime_doctor_state_plan(
        prodex_mojo_core::rich::RuntimeDoctorStatePlanInput {
            operation: prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_OP_QUOTA,
            route_kind: runtime_doctor_quota_route_code(route_kind),
            now,
            checked_at: snapshot.checked_at,
            five_hour_status: runtime_doctor_quota_status_code(snapshot.five_hour_status),
            five_hour_reset_at: snapshot.five_hour_reset_at,
            weekly_status: runtime_doctor_quota_status_code(snapshot.weekly_status),
            weekly_reset_at: snapshot.weekly_reset_at,
            decay_seconds: 1,
            ..prodex_mojo_core::rich::RuntimeDoctorStatePlanInput::default()
        },
    )
    .expect("Mojo runtime-doctor quota plan returned invalid output");
    RuntimeDoctorQuotaSummary {
        five_hour: RuntimeDoctorQuotaWindowSummary {
            status: runtime_doctor_quota_status_from_code(plan.five_hour_status),
        },
        weekly: RuntimeDoctorQuotaWindowSummary {
            status: runtime_doctor_quota_status_from_code(plan.weekly_status),
        },
        route_band: match plan.route_band {
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
        },
    }
}

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

pub(super) fn runtime_doctor_unknown_quota_summary() -> RuntimeDoctorQuotaSummary {
    RuntimeDoctorQuotaSummary {
        five_hour: RuntimeDoctorQuotaWindowSummary {
            status: RuntimeDoctorQuotaWindowStatus::Unknown,
        },
        weekly: RuntimeDoctorQuotaWindowSummary {
            status: RuntimeDoctorQuotaWindowStatus::Unknown,
        },
        route_band: RuntimeDoctorQuotaPressureBand::Unknown,
    }
}
