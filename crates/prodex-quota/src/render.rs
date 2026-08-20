use chrono::{Local, TimeZone};
use std::cmp::Ordering;
use terminal_ui::{
    CLI_LABEL_WIDTH, CLI_TABLE_GAP, current_cli_width, format_field_lines_with_layout, pad_cell,
    panel_label_width, section_header_with_width, text_width, wrap_text,
};

#[cfg(test)]
use super::ExternalQuotaDetail;
use super::{
    AdditionalRateLimit, BlockedLimit, CopilotQuotaInfo, ExternalQuotaInfo, GeminiQuotaBucket,
    GeminiQuotaInfo, MainWindowSnapshot, ProviderQuotaSnapshot, QuotaReport, QuotaReportSort,
    RenderedQuotaReportWindow, RuntimeQuotaPressureBand, RuntimeQuotaSummary,
    RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary, UsageResponse, UsageWindow, WindowPair,
};

mod copilot;
mod gemini;
mod panels;
mod pool;
mod quota_policy;
mod remaining_percent;
mod reports;
mod windows;

pub use copilot::*;
pub use gemini::*;
pub use panels::*;
pub use pool::*;
pub use quota_policy::*;
pub use remaining_percent::*;
pub use reports::*;
pub use windows::*;
#[cfg(test)]
#[path = "../tests/src/render.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/src/render/quota_policy.rs"]
mod quota_policy_tests;

#[cfg(all(test, feature = "mojo", not(prodex_mojo_fallback)))]
#[path = "../tests/src/render/mojo.rs"]
mod mojo_tests;

#[cfg(all(test, feature = "mojo", prodex_mojo_required))]
#[test]
fn mojo_feature_requires_real_compiled_core() {
    #[cfg(not(prodex_mojo_active))]
    panic!("Mojo feature unexpectedly built without a real Mojo archive");
    #[cfg(prodex_mojo_fallback)]
    panic!("Mojo feature unexpectedly built with the Rust fallback");
}
