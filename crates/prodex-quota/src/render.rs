use chrono::{Local, TimeZone};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use terminal_ui::{
    CLI_LABEL_WIDTH, CLI_TABLE_GAP, current_cli_width, fit_cell, format_field_lines_with_layout,
    panel_label_width, section_header_with_width, text_width, wrap_text,
};

use super::{
    BlockedLimit, CopilotQuotaInfo, GeminiQuotaBucket, GeminiQuotaInfo, MainWindowSnapshot,
    ProviderQuotaSnapshot, QuotaReport, QuotaReportSort, RenderedQuotaReportWindow,
    RuntimeQuotaPressureBand, RuntimeQuotaSummary, RuntimeQuotaWindowStatus,
    RuntimeQuotaWindowSummary, UsageResponse, UsageWindow, WindowPair,
};

mod copilot;
mod gemini;
mod panels;
mod pool;
mod reports;
mod windows;

pub use copilot::*;
pub use gemini::*;
pub use panels::*;
pub use pool::*;
pub use reports::*;
pub use windows::*;
#[cfg(test)]
#[path = "../tests/src/render.rs"]
mod tests;
