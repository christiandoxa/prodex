use super::*;

pub(super) fn skip_sync_probe(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "selection_skip_sync_probe";
    if diagnosis::runtime_doctor_marker_count(summary, marker) == 0 {
        return;
    }
    let deferred = summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| {
            fields
                .get("cold_start_jobs")
                .map(|count| format!("{count} job(s)"))
                .or_else(|| {
                    fields
                        .get("cold_start_profiles")
                        .map(|count| format!("{count} profile(s)"))
                })
        })
        .unwrap_or_else(|| "-".to_string());
    fields
        .push("Sync-probe route", marker_field(summary, marker, "route"))
        .push("Sync-probe reason", marker_field(summary, marker, "reason"))
        .push("Sync-probe deferred", deferred)
        .push(
            "Sync-probe next step",
            diagnosis::runtime_doctor_sync_probe_skip_next_step(summary),
        );
}

pub(super) fn plan(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "selection_plan";
    if diagnosis::runtime_doctor_marker_count(summary, marker) == 0 {
        return;
    }
    fields
        .push("Selection route", marker_field(summary, marker, "route"))
        .push("Ready candidates", marker_field(summary, marker, "ready"))
        .push(
            "Fallback candidates",
            marker_field(summary, marker, "fallback"),
        )
        .push(
            "Excluded profiles",
            marker_field(summary, marker, "excluded_count"),
        )
        .push(
            "Cold-start jobs",
            marker_field(summary, marker, "cold_start_jobs"),
        )
        .push(
            "Sync-probe jobs",
            marker_field(summary, marker, "sync_probe_jobs"),
        )
        .push(
            "Sync-probe mode",
            marker_field(summary, marker, "sync_probe_mode"),
        )
        .push(
            "Pressure mode",
            marker_field(summary, marker, "pressure_mode"),
        );
}

pub(super) fn compat_request_surface(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
) {
    fields
        .push("Compat warnings", summary.compat_warning_count.to_string())
        .push(
            "Client family",
            summary
                .top_client_family
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push(
            "Top client",
            summary
                .top_client
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push(
            "Tool surface",
            summary
                .top_tool_surface
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push(
            "Compat warning",
            summary
                .top_compat_warning
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
    for (label, facet) in [
        ("Hot lane", "lane"),
        ("Hot route", "route"),
        ("Hot profile", "profile"),
        ("Hot reason", "reason"),
        ("Quota source", "quota_source"),
    ] {
        fields.push(
            label,
            diagnosis::runtime_doctor_top_facet(summary, facet).unwrap_or_else(|| "-".to_string()),
        );
    }
}
