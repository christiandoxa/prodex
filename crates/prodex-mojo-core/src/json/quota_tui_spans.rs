use super::{MojoError, signed, status};

/// Ratatui-neutral tones selected by the compiled quota presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaTuiTone {
    Raw,
    Title,
    Muted,
    Error,
    Success,
    Bold,
    Detail,
}

impl TryFrom<i64> for QuotaTuiTone {
    type Error = MojoError;

    fn try_from(value: i64) -> Result<Self, MojoError> {
        match value {
            0 => Ok(Self::Raw),
            1 => Ok(Self::Title),
            2 => Ok(Self::Muted),
            3 => Ok(Self::Error),
            4 => Ok(Self::Success),
            5 => Ok(Self::Bold),
            6 => Ok(Self::Detail),
            _ => Err(MojoError::InvalidOutput),
        }
    }
}

/// Whole-line styling or borrowed field slices, including the original colon.
#[derive(Debug, PartialEq, Eq)]
pub enum QuotaTuiLine<'a> {
    Whole(QuotaTuiTone),
    Field { label: &'a str, value: &'a str },
}

#[repr(i64)]
enum QuotaTuiContext {
    HumanLine = 0,
    StatusCell = 1,
}

unsafe extern "C" {
    fn prodex_quota_tui_span_plan_v1(
        abi: i64,
        operation: i64,
        text: u64,
        text_length: i64,
        output: u64,
        output_capacity: i64,
    ) -> i64;
}

fn quota_tui_plan(text: &str, context: QuotaTuiContext) -> Result<[i64; 3], MojoError> {
    let mut output = [-1_i64; 3];
    // SAFETY: the borrowed UTF-8 text and distinct three-word output outlive
    // the synchronous call; Mojo validates the version and both bounds.
    status(unsafe {
        prodex_quota_tui_span_plan_v1(
            1,
            context as i64,
            text.as_ptr() as u64,
            signed(text.len())?,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
        )
    })?;
    Ok(output)
}

/// Select human-line styles and checked field slices without copying text.
pub fn quota_tui_line_plan(text: &str) -> Result<QuotaTuiLine<'_>, MojoError> {
    match quota_tui_plan(text, QuotaTuiContext::HumanLine)? {
        [7, label_end, value_start] => {
            let label_end = usize::try_from(label_end).map_err(|_| MojoError::InvalidOutput)?;
            let value_start = usize::try_from(value_start).map_err(|_| MojoError::InvalidOutput)?;
            if label_end == 0 || label_end > value_start {
                return Err(MojoError::InvalidOutput);
            }
            Ok(QuotaTuiLine::Field {
                label: text.get(..label_end).ok_or(MojoError::InvalidOutput)?,
                value: text.get(value_start..).ok_or(MojoError::InvalidOutput)?,
            })
        }
        [kind, 0, 0] => Ok(QuotaTuiLine::Whole(QuotaTuiTone::try_from(kind)?)),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select the table status tone under its narrower legacy matching rules.
pub fn quota_tui_status_tone(text: &str) -> Result<QuotaTuiTone, MojoError> {
    match quota_tui_plan(text, QuotaTuiContext::StatusCell)? {
        [kind @ (0 | 3 | 4), 0, 0] => QuotaTuiTone::try_from(kind),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "quota_tui_spans_tests.rs"]
mod tests;
