use super::{QUOTA_MODEL_POLICY_ABI_VERSION, quota_model_policy_status};

const WINDOW_TEXT_MODE_DETAILS: i64 = 0;
const WINDOW_TEXT_MODE_STATUS: i64 = 1;
const WINDOW_TEXT_MODE_COMPACT: i64 = 2;
const WINDOW_TEXT_MODE_PAIR: i64 = 3;
const WINDOW_TEXT_MODE_PAIR_COMPACT: i64 = 4;
const WINDOW_TEXT_MODE_RESET_SUMMARY: i64 = 5;

const WINDOW_TEXT_BASE_CAPACITY: usize = 96;

type WindowParts = (i64, u64, i64, i64, i64, i64, u64, i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaWindowDisplayInput<'a> {
    pub label: &'a str,
    pub used_percent: Option<i64>,
    pub reset: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaResetWindowDisplayInput<'a> {
    pub reset: Option<&'a str>,
}

unsafe extern "C" {
    fn prodex_quota_window_text_v1(
        abi_version: i64,
        mode: i64,
        first_present: i64,
        first_label_address: u64,
        first_label_length: i64,
        first_used_present: i64,
        first_used: i64,
        first_reset_present: i64,
        first_reset_address: u64,
        first_reset_length: i64,
        second_present: i64,
        second_label_address: u64,
        second_label_length: i64,
        second_used_present: i64,
        second_used: i64,
        second_reset_present: i64,
        second_reset_address: u64,
        second_reset_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn text_parts(value: Option<&str>) -> Result<(u64, i64, i64), crate::MojoError> {
    let Some(value) = value else {
        return Ok((0, 0, 0));
    };
    Ok((
        value.as_ptr() as usize as u64,
        i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        1,
    ))
}

fn window_parts(
    value: Option<QuotaWindowDisplayInput<'_>>,
) -> Result<WindowParts, crate::MojoError> {
    let Some(value) = value else {
        return Ok((0, 0, 0, 0, 0, 0, 0, 0));
    };
    let (label_address, label_length, _) = text_parts(Some(value.label))?;
    let (reset_address, reset_length, reset_present) = text_parts(value.reset)?;
    Ok((
        1,
        label_address,
        label_length,
        i64::from(value.used_percent.is_some()),
        value.used_percent.unwrap_or_default(),
        reset_present,
        reset_address,
        reset_length,
    ))
}

fn output_capacity(
    mode: i64,
    first: Option<QuotaWindowDisplayInput<'_>>,
    second: Option<QuotaWindowDisplayInput<'_>>,
) -> Result<usize, crate::MojoError> {
    let input_len = |value: Option<QuotaWindowDisplayInput<'_>>| {
        value
            .map(|value| {
                value
                    .label
                    .len()
                    .checked_add(value.reset.map(str::len).unwrap_or(0))
                    .ok_or(crate::MojoError::InvalidInput)
            })
            .transpose()
            .map(|value| value.unwrap_or_default())
    };
    let first_len = input_len(first)?;
    let second_len = input_len(second)?;
    let text = first_len
        .checked_add(second_len)
        .and_then(|length| length.checked_add(WINDOW_TEXT_BASE_CAPACITY))
        .ok_or(crate::MojoError::InvalidInput)?;
    let minimum = if mode == WINDOW_TEXT_MODE_RESET_SUMMARY {
        32
    } else {
        1
    };
    Ok(text.max(minimum))
}

fn render(
    mode: i64,
    first: Option<QuotaWindowDisplayInput<'_>>,
    second: Option<QuotaWindowDisplayInput<'_>>,
) -> Result<String, crate::MojoError> {
    let first_parts = window_parts(first)?;
    let second_parts = window_parts(second)?;
    let capacity = output_capacity(mode, first, second)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_window_text_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            mode,
            first_parts.0,
            first_parts.1,
            first_parts.2,
            first_parts.3,
            first_parts.4,
            first_parts.5,
            first_parts.6,
            first_parts.7,
            second_parts.0,
            second_parts.1,
            second_parts.2,
            second_parts.3,
            second_parts.4,
            second_parts.5,
            second_parts.6,
            second_parts.7,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn quota_window_details_display(
    used_percent: Option<i64>,
    reset: &str,
) -> Result<String, crate::MojoError> {
    render(
        WINDOW_TEXT_MODE_DETAILS,
        Some(QuotaWindowDisplayInput {
            label: "",
            used_percent,
            reset: Some(reset),
        }),
        None,
    )
}

pub fn quota_window_status_display(
    input: QuotaWindowDisplayInput<'_>,
) -> Result<String, crate::MojoError> {
    render(WINDOW_TEXT_MODE_STATUS, Some(input), None)
}

pub fn quota_window_status_compact_display(
    label: &str,
    used_percent: Option<i64>,
) -> Result<String, crate::MojoError> {
    render(
        WINDOW_TEXT_MODE_COMPACT,
        Some(QuotaWindowDisplayInput {
            label,
            used_percent,
            reset: None,
        }),
        None,
    )
}

pub fn quota_window_pair_display(
    first: Option<QuotaWindowDisplayInput<'_>>,
    second: Option<QuotaWindowDisplayInput<'_>>,
    compact: bool,
) -> Result<String, crate::MojoError> {
    render(
        if compact {
            WINDOW_TEXT_MODE_PAIR_COMPACT
        } else {
            WINDOW_TEXT_MODE_PAIR
        },
        first,
        second,
    )
}

pub fn quota_reset_summary_display(
    five_hour: Option<QuotaResetWindowDisplayInput<'_>>,
    weekly: Option<QuotaResetWindowDisplayInput<'_>>,
) -> Result<String, crate::MojoError> {
    let first = five_hour.map(|value| QuotaWindowDisplayInput {
        label: "",
        used_percent: None,
        reset: value.reset,
    });
    let second = weekly.map(|value| QuotaWindowDisplayInput {
        label: "",
        used_percent: None,
        reset: value.reset,
    });
    render(WINDOW_TEXT_MODE_RESET_SUMMARY, first, second)
}
