//! Gemini numeric plans from parsed counters or borrowed counter text.
use super::{
    GeminiBucketNumericInput, GeminiBucketNumericOutput, GeminiRemainingAmount,
    QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT,
};
use crate::MojoError;

/// Borrowed Serde counter and fraction facts for the compiled quota kernel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeminiBucketTextInput<'a> {
    pub remaining_amount: Option<&'a str>,
    pub remaining_fraction: Option<f64>,
}

#[repr(C)]
#[derive(Default)]
struct InputRow {
    amount_address: u64,
    amount_length: i64,
    amount_source: i64,
    amount: i64,
    fraction: f64,
    fraction_present: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OutputRow {
    remaining: i64,
    remaining_present: i64,
    total: i64,
    total_present: i64,
    remaining_percent: i64,
    remaining_percent_present: i64,
    exhausted: i64,
}

const _: () = {
    assert!(std::mem::size_of::<InputRow>() == 48);
    assert!(std::mem::align_of::<InputRow>() == 8);
    assert!(std::mem::size_of::<OutputRow>() == 56);
    assert!(std::mem::align_of::<OutputRow>() == 8);
};

unsafe extern "C" {
    fn prodex_quota_gemini_bucket_records_v1(
        version: i64,
        rows: u64,
        count: i64,
        output: u64,
        capacity: i64,
    ) -> i64;
}

fn batch_count(count: usize) -> Result<i64, MojoError> {
    if count > QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT {
        return Err(MojoError::InvalidInput);
    }
    i64::try_from(count).map_err(|_| MojoError::InvalidInput)
}

pub fn gemini_bucket_numeric_batch(
    inputs: &[GeminiBucketNumericInput],
) -> Result<Vec<GeminiBucketNumericOutput>, MojoError> {
    let count = batch_count(inputs.len())?;
    let rows = inputs
        .iter()
        .map(|input| {
            let (amount_source, amount) = match input.remaining_amount {
                GeminiRemainingAmount::Absent => (0, 0),
                GeminiRemainingAmount::Parsed(value) => (1, value),
                GeminiRemainingAmount::Invalid => (2, 0),
            };
            InputRow {
                amount_source,
                amount,
                fraction: input.remaining_fraction.unwrap_or_default(),
                fraction_present: i64::from(input.remaining_fraction.is_some()),
                ..InputRow::default()
            }
        })
        .collect::<Vec<_>>();
    call(&rows, count)
}

/// Trims/parses counters and computes numeric results in one compiled batch.
pub fn gemini_bucket_text_numeric_batch(
    inputs: &[GeminiBucketTextInput<'_>],
) -> Result<Vec<GeminiBucketNumericOutput>, MojoError> {
    let count = batch_count(inputs.len())?;
    let rows = inputs
        .iter()
        .map(|input| {
            let mut row = InputRow {
                fraction: input.remaining_fraction.unwrap_or_default(),
                fraction_present: i64::from(input.remaining_fraction.is_some()),
                ..InputRow::default()
            };
            if let Some(value) = input.remaining_amount {
                row.amount_source = 3;
                row.amount_address = value.as_ptr() as u64;
                row.amount_length =
                    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    call(&rows, count)
}

fn call(rows: &[InputRow], count: i64) -> Result<Vec<GeminiBucketNumericOutput>, MojoError> {
    let mut output = vec![OutputRow::default(); rows.len()];
    let status = unsafe {
        prodex_quota_gemini_bucket_records_v1(
            1,
            rows.as_ptr() as u64,
            count,
            output.as_mut_ptr() as u64,
            count,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    output
        .into_iter()
        .map(|row| {
            if !matches!(
                (
                    row.remaining_present,
                    row.total_present,
                    row.remaining_percent_present,
                    row.exhausted
                ),
                (0 | 1, 0 | 1, 0 | 1, 0 | 1)
            ) {
                return Err(MojoError::InvalidOutput);
            }
            Ok(GeminiBucketNumericOutput {
                remaining: (row.remaining_present == 1).then_some(row.remaining),
                total: (row.total_present == 1).then_some(row.total),
                remaining_percent: (row.remaining_percent_present == 1)
                    .then_some(row.remaining_percent),
                exhausted: row.exhausted == 1,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "gemini_amount_tests.rs"]
mod tests;
