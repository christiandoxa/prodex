use super::{SessionReport, tui_hint_style, tui_primary_style, tui_secondary_style};
use anyhow::{Result, bail};
use prodex_mojo_core::json::{SessionTuiMode, SessionTuiRow, session_tui_rows_json};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SessionTuiTone {
    Raw,
    HintBold,
    Primary,
    Secondary,
}

pub(super) fn session_tui_report_lines(
    reports: &[SessionReport],
    mode: SessionTuiMode,
) -> Result<Vec<Vec<Line<'static>>>> {
    let rows = reports
        .iter()
        .map(|report| SessionTuiRow {
            id: &report.id,
            title: report.thread_name.as_deref(),
            updated_at: report.updated_at.as_deref(),
            cwd: report.cwd.as_deref(),
            profile: report.profile.as_deref(),
            provider: report.model_provider.as_deref(),
        })
        .collect::<Vec<_>>();
    let rendered = session_tui_rows_json(&rows, mode)
        .map_err(|error| anyhow::anyhow!("Mojo session presentation failed: {error:?}"))?;
    let rendered: Vec<Vec<Vec<(SessionTuiTone, String)>>> = serde_json::from_str(&rendered)?;
    if rendered.len() != reports.len() {
        bail!("Mojo session presentation returned an invalid report count");
    }
    // Ratatui styles stay at the host boundary; Mojo owns text, order and tone.
    Ok(rendered
        .into_iter()
        .map(|lines| {
            lines
                .into_iter()
                .map(|spans| {
                    Line::from(
                        spans
                            .into_iter()
                            .map(|(tone, text)| match tone {
                                SessionTuiTone::Raw => Span::raw(text),
                                SessionTuiTone::HintBold => Span::styled(
                                    text,
                                    tui_hint_style().add_modifier(Modifier::BOLD),
                                ),
                                SessionTuiTone::Primary => Span::styled(text, tui_primary_style()),
                                SessionTuiTone::Secondary => {
                                    Span::styled(text, tui_secondary_style())
                                }
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
        })
        .collect())
}
