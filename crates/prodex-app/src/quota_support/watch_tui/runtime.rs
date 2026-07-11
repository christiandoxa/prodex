pub(crate) use super::runtime_all::{render_all_quota_reports_once_tui, watch_all_quotas};
use super::*;

pub(crate) fn render_profile_quota_once_tui(
    frame: &mut ratatui::Frame<'_>,
    profile_name: &str,
    quota: ProviderQuotaSnapshot,
) {
    let data =
        build_profile_quota_watch_tui_frame(profile_name, &quota_watch_updated_at(), Ok(quota));
    render_all_quota_watch_tui(frame, &data);
}
