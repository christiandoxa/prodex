use super::{InfoStringView, InfoTokenUsageProfile, MojoError};

/// Token counters shared by report aggregation, caller models and terminal display.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}
const _: () = assert!(std::mem::size_of::<TokenUsageCounts>() == 32);

/// One complete profile group selected and aggregated by Mojo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoTokenUsageProfileSummary<'a> {
    pub profile: &'a str,
    pub event_count: usize,
    pub total: TokenUsageCounts,
}

/// Completed-event totals with lexically ordered profile groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoTokenUsageAggregate<'a> {
    pub event_count: usize,
    pub total: TokenUsageCounts,
    pub profiles: Vec<InfoTokenUsageProfileSummary<'a>>,
}

unsafe extern "C" {
    fn prodex_info_token_usage_summary_v1(
        abi: i64,
        profiles: u64,
        counts: u64,
        count: i64,
        output: u64,
        output_words: i64,
    ) -> i64;
}

fn decode_counts(values: [u64; 4]) -> TokenUsageCounts {
    TokenUsageCounts {
        input_tokens: values[0],
        cached_input_tokens: values[1],
        output_tokens: values[2],
        reasoning_tokens: values[3],
    }
}

/// Groups acquired usage events and computes all saturating totals in Mojo.
pub fn aggregate_info_token_usage<'a>(
    records: &[InfoTokenUsageProfile<'a>],
) -> Result<InfoTokenUsageAggregate<'a>, MojoError> {
    let views = records
        .iter()
        .map(|row| InfoStringView {
            ptr: row.profile.as_ptr() as u64,
            len: row.profile.len() as u64,
        })
        .collect::<Vec<_>>();
    let counts = records
        .iter()
        .map(|row| TokenUsageCounts {
            input_tokens: row.input_tokens,
            cached_input_tokens: row.cached_input_tokens,
            output_tokens: row.output_tokens,
            reasoning_tokens: row.reasoning_tokens,
        })
        .collect::<Vec<_>>();
    let capacity = records
        .len()
        .checked_mul(7)
        .and_then(|count| count.checked_add(6))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u64; capacity];
    let status = unsafe {
        prodex_info_token_usage_summary_v1(
            1,
            views.as_ptr() as u64,
            counts.as_ptr() as u64,
            i64::try_from(records.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let groups = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if groups > records.len() || output[1] != records.len() as u64 {
        return Err(MojoError::InvalidOutput);
    }
    let profiles = output[6..6 + groups * 6]
        .as_chunks::<6>()
        .0
        .iter()
        .map(|group| {
            let index = usize::try_from(group[0]).map_err(|_| MojoError::InvalidOutput)?;
            let profile = records.get(index).ok_or(MojoError::InvalidOutput)?.profile;
            let event_count = usize::try_from(group[1]).map_err(|_| MojoError::InvalidOutput)?;
            if event_count == 0 || event_count > records.len() {
                return Err(MojoError::InvalidOutput);
            }
            Ok(InfoTokenUsageProfileSummary {
                profile,
                event_count,
                total: decode_counts([group[2], group[3], group[4], group[5]]),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    Ok(InfoTokenUsageAggregate {
        event_count: usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        total: decode_counts([output[2], output[3], output[4], output[5]]),
        profiles,
    })
}

#[cfg(test)]
#[path = "token_usage_tests.rs"]
mod tests;
