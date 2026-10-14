//! Typed Gemini bucket numeric batch marshalling and validation.
use super::*;

pub fn gemini_bucket_numeric_batch(
    inputs: &[GeminiBucketNumericInput],
) -> Result<Vec<GeminiBucketNumericOutput>, crate::MojoError> {
    if inputs.len() > QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }

    let remaining_amount = inputs
        .iter()
        .map(|input| match input.remaining_amount {
            GeminiRemainingAmount::Parsed(value) => value,
            GeminiRemainingAmount::Absent | GeminiRemainingAmount::Invalid => 0,
        })
        .collect::<Vec<_>>();
    let remaining_amount_state = inputs
        .iter()
        .map(|input| match input.remaining_amount {
            GeminiRemainingAmount::Absent => 0,
            GeminiRemainingAmount::Parsed(_) => 1,
            GeminiRemainingAmount::Invalid => 2,
        })
        .collect::<Vec<_>>();
    let remaining_fraction = inputs
        .iter()
        .map(|input| input.remaining_fraction.unwrap_or_default())
        .collect::<Vec<_>>();
    let remaining_fraction_present = inputs
        .iter()
        .map(|input| i64::from(input.remaining_fraction.is_some()))
        .collect::<Vec<_>>();
    let mut remaining = vec![0_i64; inputs.len()];
    let mut remaining_present = vec![0_i64; inputs.len()];
    let mut total = vec![0_i64; inputs.len()];
    let mut total_present = vec![0_i64; inputs.len()];
    let mut remaining_percent = vec![0_i64; inputs.len()];
    let mut remaining_percent_present = vec![0_i64; inputs.len()];
    let mut exhausted = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_quota_gemini_bucket_batch(
            remaining_amount.as_ptr(),
            remaining_amount_state.as_ptr(),
            remaining_fraction.as_ptr(),
            remaining_fraction_present.as_ptr(),
            remaining.as_mut_ptr(),
            remaining_present.as_mut_ptr(),
            total.as_mut_ptr(),
            total_present.as_mut_ptr(),
            remaining_percent.as_mut_ptr(),
            remaining_percent_present.as_mut_ptr(),
            exhausted.as_mut_ptr(),
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(if status == 1 {
            crate::MojoError::InvalidInput
        } else {
            crate::MojoError::InvalidOutput
        });
    }

    let mut outputs = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        if !matches!(
            (
                remaining_present[index],
                total_present[index],
                remaining_percent_present[index],
                exhausted[index],
            ),
            (0 | 1, 0 | 1, 0 | 1, 0 | 1)
        ) {
            return Err(crate::MojoError::InvalidOutput);
        }
        outputs.push(GeminiBucketNumericOutput {
            remaining: (remaining_present[index] == 1).then_some(remaining[index]),
            total: (total_present[index] == 1).then_some(total[index]),
            remaining_percent: (remaining_percent_present[index] == 1)
                .then_some(remaining_percent[index]),
            exhausted: exhausted[index] == 1,
        });
    }
    Ok(outputs)
}
