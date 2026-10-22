//! Borrowed native rows keep legacy ping reports outside JSON acquisition limits.
use crate::MojoError;
use crate::rich::{RichStringView, ensure_rich_abi, status_error, view};
use crate::text_output::render_measured_string;

/// Original ping report surfaces selected by the caller's output mode.
#[derive(Clone, Copy)]
#[repr(i64)]
pub enum PingReportKind {
    Profile = 0,
    SummaryHuman = 1,
    SummaryJson = 2,
}

/// Non-secret observed ping fields, borrowed only for the synchronous render call.
pub struct PingReportRow<'a> {
    pub profile: &'a str,
    /// An existing PING_STATUS_* ABI tag; unknown tags are rejected by Mojo.
    pub status: i64,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub effective_model: Option<&'a str>,
    pub detail: &'a str,
    pub first_response_latency_ms: Option<u128>,
    pub latency_ms: Option<u128>,
}

/// Native rows and observed elapsed time for one compiled report.
pub struct PingReportInput<'a> {
    pub rows: &'a [PingReportRow<'a>],
    pub kind: PingReportKind,
    pub elapsed_ms: u128,
}

#[repr(C)]
struct RowFfi {
    status: i64,
    presence: u64,
    profile: RichStringView,
    model: RichStringView,
    effort: RichStringView,
    effective_model: RichStringView,
    detail: RichStringView,
    first_low: u64,
    first_high: u64,
    completion_low: u64,
    completion_high: u64,
}

const _: () = assert!(std::mem::size_of::<RowFfi>() == 128 && std::mem::align_of::<RowFfi>() == 8);

unsafe extern "C" {
    fn prodex_mojo_ping_report_v1(
        abi: i64,
        kind: i64,
        rows: u64,
        count: i64,
        elapsed_low: u64,
        elapsed_high: u64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Render through Mojo without reconstructing labels, schemas, defaults or totals.
pub fn render_ping_report(input: PingReportInput<'_>) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let count = i64::try_from(input.rows.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(input.rows.len())
        .map_err(|_| MojoError::Capacity)?;
    for row in input.rows {
        let first = row.first_response_latency_ms.unwrap_or_default();
        let completion = row.latency_ms.unwrap_or_default();
        let presence = [
            row.model.is_some(),
            row.effort.is_some(),
            row.effective_model.is_some(),
            row.first_response_latency_ms.is_some(),
            row.latency_ms.is_some(),
        ]
        .into_iter()
        .enumerate()
        .fold(0_u64, |bits, (index, present)| {
            bits | (u64::from(present) << index)
        });
        // Split native u128 values into stable C words without choosing a display value.
        rows.push(RowFfi {
            status: row.status,
            presence,
            profile: view(row.profile),
            model: view(row.model.unwrap_or_default()),
            effort: view(row.effort.unwrap_or_default()),
            effective_model: view(row.effective_model.unwrap_or_default()),
            detail: view(row.detail),
            first_low: first as u64,
            first_high: (first >> 64) as u64,
            completion_low: completion as u64,
            completion_high: (completion >> 64) as u64,
        });
    }
    // This typed ABI cannot enter the parsed-node JSON runner without manufacturing JSON.
    let invoke = |output: Option<&mut [u8]>| -> Result<usize, MojoError> {
        let (measuring, address, capacity) = match output {
            None => (1, 0, 0),
            Some(output) => (
                0,
                output.as_mut_ptr() as u64,
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            ),
        };
        let mut metadata = [-1_i64; 2];
        // SAFETY: rows and strings outlive this synchronous call; output and metadata
        // are distinct host allocations. Mojo validates all tags and lengths first.
        let code = unsafe {
            prodex_mojo_ping_report_v1(
                1,
                input.kind as i64,
                rows.as_ptr() as u64,
                count,
                input.elapsed_ms as u64,
                (input.elapsed_ms >> 64) as u64,
                measuring,
                address,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        };
        if code != 0 {
            return Err(status_error(code, 0, 0, 0, 0));
        }
        if metadata[0] != 1 {
            return Err(MojoError::InvalidOutput);
        }
        usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)
    };
    render_measured_string(invoke)
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
