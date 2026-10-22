use super::StatusOverview;
use anyhow::Result;
use prodex_mojo_core::json::{StatusTokenPanelInput, status_token_panel_json};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use serde::Deserialize;
use terminal_ui::{
    tui_accent_style, tui_error_style, tui_hint_style, tui_metric_style, tui_muted_style,
    tui_secondary_style, tui_title_style,
};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StatusTuiTone {
    Raw,
    Secondary,
    Metric,
    Muted,
    Accent,
    Title,
    Error,
    HintBold,
}

#[derive(Deserialize)]
struct TokenPresentation {
    lines: Vec<Vec<(StatusTuiTone, String)>>,
    history_title: String,
}

pub(super) fn status_token_panel_lines(
    overview: Option<&StatusOverview>,
) -> Result<(Vec<Line<'static>>, String)> {
    let rendered = status_token_panel_json(overview.map(|overview| StatusTokenPanelInput {
        tokens: overview.token_summary.total,
        event_count: overview.token_summary.event_count,
        log_count: overview.token_summary.log_count,
        profile_count: overview.profile_count,
        runtime_profile: &overview.runtime_profile,
        active_profile: &overview.active_profile,
        first_at: overview.token_first_at.as_deref(),
        last_at: overview.token_last_at.as_deref(),
    }))
    .map_err(|error| anyhow::anyhow!("Mojo status token presentation failed: {error:?}"))?;
    let report: TokenPresentation = serde_json::from_str(&rendered)?;
    // Ratatui styles stay at the host boundary; Mojo owns text, order and tone.
    let lines = report.lines.into_iter().map(status_tui_line).collect();
    Ok((lines, report.history_title))
}

pub(super) fn status_tui_line(spans: Vec<(StatusTuiTone, String)>) -> Line<'static> {
    Line::from(
        spans
            .into_iter()
            .map(|(tone, text)| match tone {
                StatusTuiTone::Title => Span::styled(text, tui_title_style()),
                StatusTuiTone::Error => Span::styled(text, tui_error_style()),
                StatusTuiTone::HintBold => {
                    Span::styled(text, tui_hint_style().add_modifier(Modifier::BOLD))
                }
                StatusTuiTone::Raw => Span::raw(text),
                StatusTuiTone::Secondary => Span::styled(text, tui_secondary_style()),
                StatusTuiTone::Metric => Span::styled(text, tui_metric_style()),
                StatusTuiTone::Muted => Span::styled(text, tui_muted_style()),
                StatusTuiTone::Accent => Span::styled(text, tui_accent_style()),
            })
            .collect::<Vec<_>>(),
    )
}
