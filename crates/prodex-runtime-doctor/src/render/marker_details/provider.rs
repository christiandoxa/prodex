use super::*;

pub(super) fn model_fallback(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let Some(marker_fields) = summary
        .marker_last_fields
        .get("local_rewrite_provider_model_fallback")
    else {
        return;
    };
    fields
        .push(
            "Provider fallback",
            format!(
                "{} {} -> {}",
                marker_fields
                    .get("provider")
                    .map(String::as_str)
                    .unwrap_or("provider"),
                marker_fields
                    .get("from_model")
                    .map(String::as_str)
                    .unwrap_or("-"),
                marker_fields
                    .get("to_model")
                    .map(String::as_str)
                    .unwrap_or("-")
            ),
        )
        .push(
            "Provider fallback class",
            marker_fields
                .get("class")
                .cloned()
                .unwrap_or_else(|| "-".to_string()),
        );
}

pub(super) fn auth_failure(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let Some(marker_fields) = summary
        .marker_last_fields
        .get("local_rewrite_provider_auth_failure")
    else {
        return;
    };
    fields
        .push(
            "Provider auth scope",
            format!(
                "{} profile={} status={}",
                marker_fields
                    .get("provider")
                    .map(String::as_str)
                    .unwrap_or("provider"),
                marker_fields
                    .get("profile")
                    .map(String::as_str)
                    .unwrap_or("-"),
                marker_fields
                    .get("status")
                    .map(String::as_str)
                    .unwrap_or("-")
            ),
        )
        .push(
            "Provider auth error",
            marker_fields
                .get("class")
                .cloned()
                .unwrap_or_else(|| "-".to_string()),
        );
}

pub(super) fn gemini_retry(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    let Some(marker_fields) = summary.marker_last_fields.get(marker) else {
        return;
    };
    fields.push(
        "Gemini retry scope",
        format!(
            "profile={} status={} reason={} retry={} delay_ms={}",
            value(marker_fields, "profile"),
            value(marker_fields, "status"),
            value(marker_fields, "reason"),
            value(marker_fields, "retry"),
            value(marker_fields, "delay_ms")
        ),
    );
}

pub(super) fn gemini_stream_retry(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    let Some(marker_fields) = summary.marker_last_fields.get(marker) else {
        return;
    };
    fields.push(
        "Gemini stream retry",
        format!(
            "profile={} model={} from={} to={} reason={}",
            value(marker_fields, "profile"),
            value(marker_fields, "model"),
            value(marker_fields, "from_model"),
            value(marker_fields, "to_model"),
            value(marker_fields, "reason")
        ),
    );
}

pub(super) fn gemini_compact(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    let Some(marker_fields) = summary.marker_last_fields.get(marker) else {
        return;
    };
    fields.push(
        "Gemini compact",
        format!(
            "mode={} profile={} request={} body_bytes={} reason={}",
            if marker == "local_rewrite_gemini_compact_semantic" {
                "semantic"
            } else {
                "local-fallback"
            },
            value(marker_fields, "profile"),
            value(marker_fields, "request"),
            value(marker_fields, "body_bytes"),
            value(marker_fields, "reason")
        ),
    );
}

pub(super) fn gemini_live_error(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    let Some(marker_fields) = summary.marker_last_fields.get(marker) else {
        return;
    };
    fields.push(
        "Gemini Live error",
        format!(
            "request={} profile={} error={}",
            value(marker_fields, "request"),
            value(marker_fields, "profile"),
            value(marker_fields, "error")
        ),
    );
}

pub(super) fn auth_recovery(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    marker: &str,
) {
    if marker == "profile_auth_recovered"
        && diagnosis::runtime_doctor_marker_count(summary, "profile_auth_recovery_failed") > 0
    {
        return;
    }
    fields
        .push(
            "Auth recovery profile",
            marker_field(summary, marker, "profile"),
        )
        .push(
            "Auth recovery route",
            marker_field(summary, marker, "route"),
        )
        .push(
            "Auth recovery source",
            marker_field(summary, marker, "source"),
        )
        .push(
            "Auth recovery error",
            marker_field(summary, marker, "error"),
        )
        .push(
            "Auth recovery next step",
            diagnosis::runtime_doctor_profile_auth_recovery_next_step(summary),
        );
}

fn value<'a>(fields: &'a std::collections::BTreeMap<String, String>, name: &str) -> &'a str {
    fields.get(name).map(String::as_str).unwrap_or("-")
}
