use super::{
    StatusOverview,
    token_presentation::{StatusTuiTone, status_tui_line},
};
use anyhow::Result;
use prodex_mojo_core::json::{StatusHeaderInput, status_footer_json, status_header_json};
use ratatui::text::Line;

pub(super) fn status_header_line(
    overview: Option<&StatusOverview>,
    refreshing: bool,
) -> Result<Line<'static>> {
    status_bar_line(status_header_json(
        overview.map(|overview| StatusHeaderInput {
            profile: &overview.runtime_profile,
            updated_at: &overview.updated_at,
        }),
        refreshing,
    ))
}

pub(super) fn status_footer_line(error: Option<&str>) -> Result<Line<'static>> {
    status_bar_line(status_footer_json(error))
}

fn status_bar_line(
    rendered: std::result::Result<String, prodex_mojo_core::MojoError>,
) -> Result<Line<'static>> {
    let rendered = rendered
        .map_err(|error| anyhow::anyhow!("Mojo status bar presentation failed: {error:?}"))?;
    let spans: Vec<(StatusTuiTone, String)> = serde_json::from_str(&rendered)?;
    Ok(status_tui_line(spans))
}
