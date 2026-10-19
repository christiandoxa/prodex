use super::{ABI_VERSION, MojoError, rate_output, status};

/// Event-specific requirements for completed and streaming usage records.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum InfoTokenUsageKind {
    Completed = 0,
    Progress = 1,
}

/// Decoded scalar text; `None` remains distinct from an empty or invalid value.
#[derive(Debug, Clone, Copy, Default)]
pub struct InfoTokenUsageFields<'a> {
    pub input_tokens: Option<&'a str>,
    pub cached_input_tokens: Option<&'a str>,
    pub output_tokens: Option<&'a str>,
    pub reasoning_tokens: Option<&'a str>,
    pub request: Option<&'a str>,
    pub generation_ms: Option<&'a str>,
    pub output_tokens_per_second: Option<&'a str>,
}

/// Validated numbers and timing selected by the compiled log policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoTokenUsageNumbers {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub request: Option<u64>,
    pub generation_ms: Option<u64>,
    pub output_tokens_per_second: Option<f64>,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct TokenFieldView {
    address: u64,
    length: u64,
}

unsafe extern "C" {
    fn prodex_info_token_usage_numbers_v1(
        abi: i64,
        kind: i64,
        texts: u64,
        text_count: i64,
        presence: u64,
        output: u64,
        output_words: i64,
    ) -> i64;
}

/// Parses scalar fields and applies completed/progress defaults without recomputation.
pub fn info_token_usage_numbers(
    kind: InfoTokenUsageKind,
    fields: InfoTokenUsageFields<'_>,
) -> Result<Option<InfoTokenUsageNumbers>, MojoError> {
    let mut presence = 0_u64;
    let mut texts = [TokenFieldView::default(); 7];
    for (index, value) in [
        fields.input_tokens,
        fields.cached_input_tokens,
        fields.output_tokens,
        fields.reasoning_tokens,
        fields.request,
        fields.generation_ms,
        fields.output_tokens_per_second,
    ]
    .into_iter()
    .enumerate()
    {
        presence |= u64::from(value.is_some()) << index;
        let value = value.unwrap_or_default();
        texts[index] = TokenFieldView {
            address: value.as_ptr() as u64,
            length: value.len() as u64,
        };
    }
    let mut output = [0_u64; 9];
    status(unsafe {
        prodex_info_token_usage_numbers_v1(
            ABI_VERSION,
            kind as i64,
            texts.as_ptr() as u64,
            7,
            presence,
            output.as_mut_ptr() as u64,
            9,
        )
    })?;
    match output[0] {
        0 if output == [0; 9] => return Ok(None),
        1 if output[7] <= 7 => {}
        _ => return Err(MojoError::InvalidOutput),
    }
    let rate = rate_output(0, ((output[7] >> 2) & 1) as i64, f64::from_bits(output[8]))?;
    Ok(Some(InfoTokenUsageNumbers {
        input_tokens: output[1],
        cached_input_tokens: output[2],
        output_tokens: output[3],
        reasoning_tokens: output[4],
        request: (output[7] & 1 != 0).then_some(output[5]),
        generation_ms: (output[7] & 2 != 0).then_some(output[6]),
        output_tokens_per_second: rate,
    }))
}

#[cfg(test)]
#[path = "token_usage_tests.rs"]
mod tests;
