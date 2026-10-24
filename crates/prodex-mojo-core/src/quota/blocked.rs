//! Borrowed observations for Mojo-owned blocked quota messages.
use crate::{
    MojoError,
    text_output::{render_measured_string, status},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum QuotaBlockedWindowKind {
    Main,
    Additional,
    CodeReview,
}

#[derive(Debug, Clone, Copy)]
pub struct QuotaBlockedWindowInput<'a> {
    pub kind: QuotaBlockedWindowKind,
    pub used_percent: Option<i64>,
    pub seconds: Option<i64>,
    /// Provider name, metered feature, and limit ID in their source order.
    pub names: [Option<&'a str>; 3],
    /// Host-local timezone formatting remains outside the compiled policy.
    pub reset: &'a str,
}

#[repr(C)]
#[derive(Clone, Copy)]
// The quota-only ABI is available without the rich JSON feature.
struct StringView {
    ptr: u64,
    len: u64,
}

impl From<&str> for StringView {
    fn from(value: &str) -> Self {
        Self {
            ptr: value.as_ptr() as u64,
            len: value.len() as u64,
        }
    }
}

#[repr(C)]
struct WindowRow {
    kind: i64,
    used_present: i64,
    used: i64,
    names_present: i64,
    seconds_present: i64,
    seconds: i64,
    names: [StringView; 3],
    reset: StringView,
}

unsafe extern "C" {
    fn prodex_quota_blocked_windows_json_v1(
        abi: i64,
        runtime_ready: i64,
        main_present: i64,
        include_code_review: i64,
        rows: u64,
        count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
}

/// Select and render blocked messages in main/additional/code-review order.
/// The caller supplies main windows in 5h/weekly order and all additional windows
/// in provider order; Mojo owns suppression, missing-data rows and name precedence.
pub fn quota_blocked_windows_json(
    runtime_ready: bool,
    main_present: bool,
    include_code_review: bool,
    inputs: &[QuotaBlockedWindowInput<'_>],
) -> Result<String, MojoError> {
    let rows = inputs
        .iter()
        .map(|input| WindowRow {
            kind: input.kind as i64,
            used_present: i64::from(input.used_percent.is_some()),
            used: input.used_percent.unwrap_or_default(),
            names_present: input
                .names
                .iter()
                .enumerate()
                .fold(0, |mask, (index, name)| {
                    mask | (i64::from(name.is_some()) << index)
                }),
            seconds_present: i64::from(input.seconds.is_some()),
            seconds: input.seconds.unwrap_or_default(),
            names: input.names.map(|value| value.unwrap_or_default().into()),
            reset: input.reset.into(),
        })
        .collect::<Vec<_>>();
    let count = i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?;
    render_measured_string(|output| {
        let (measuring, address, capacity) = match output {
            None => (1, 0, 0),
            Some(buffer) => (
                0,
                buffer.as_mut_ptr() as u64,
                i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?,
            ),
        };
        let mut written = -1_i64;
        // Rows and borrowed UTF-8 remain live; the output allocation is distinct.
        status(unsafe {
            prodex_quota_blocked_windows_json_v1(
                1,
                i64::from(runtime_ready),
                i64::from(main_present),
                i64::from(include_code_review),
                rows.as_ptr() as u64,
                count,
                measuring,
                address,
                capacity,
                &mut written as *mut i64 as u64,
            )
        })?;
        usize::try_from(written).map_err(|_| MojoError::InvalidOutput)
    })
}
