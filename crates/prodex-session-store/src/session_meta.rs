use chrono::{SecondsFormat, Utc};
use prodex_mojo_core::json::{
    SessionRepairLine, SessionSyntheticSessionIdSource, SessionSyntheticSpan,
    session_synthetic_metadata_json, session_synthetic_metadata_plan,
};
use std::path::Path;

pub(crate) fn line_starts_codex_rollout_metadata(line: &str) -> bool {
    prodex_mojo_core::json::session_report_update_json(line)
        .ok()
        .is_some_and(|plan| plan.starts_rollout_metadata)
}

fn session_meta_span_string(
    lines: &[String],
    span: Option<SessionSyntheticSpan>,
) -> Option<String> {
    let span = span?;
    let line = lines.get(span.line_index)?;
    let token = line.get(span.start..span.end)?;
    serde_json::from_str::<String>(token)
        .ok()
        .map(|value| value.trim().to_string())
}

pub(crate) fn synthetic_session_metadata_line(
    path: &Path,
    selector: &str,
    lines: &[String],
    line_facts: &[SessionRepairLine<'_>],
) -> Option<String> {
    let path_id = super::session_path_id_matching_selector(path, selector, false);
    let plan = session_synthetic_metadata_plan(
        selector,
        line_facts,
        path_id.is_some(),
        super::full_codex_session_id(selector).is_some(),
    )
    .ok()??;
    let selector_id = super::full_codex_session_id(selector).map(ToOwned::to_owned);
    let session_id = match plan.session_id {
        SessionSyntheticSessionIdSource::Line(span) => session_meta_span_string(lines, Some(span))?,
        SessionSyntheticSessionIdSource::Path => path_id?,
        SessionSyntheticSessionIdSource::Selector => selector_id?,
    };
    let timestamp = session_meta_span_string(lines, plan.timestamp)
        .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true));
    let cwd = session_meta_span_string(lines, plan.cwd)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| ".".to_string());
    let model_provider = session_meta_span_string(lines, plan.model_provider);
    session_synthetic_metadata_json(
        &session_id,
        &timestamp,
        &cwd,
        model_provider.as_deref(),
        env!("CARGO_PKG_VERSION"),
    )
    .ok()
}
