use crate::MojoError;

/// Numeric source metadata; finding locations and detector identities remain with the host.
pub struct GovernanceInspectionSource<'a> {
    pub coverage: u8,
    pub finding_kinds: &'a [u8],
    pub masked_kinds: &'a [u8],
    pub tag_count: usize,
    pub reason_count: usize,
}

/// The aggregate preserves the domain count error separately from malformed ABI input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceInspectionAggregationError {
    LimitExceeded,
    Abi(MojoError),
}

impl From<MojoError> for GovernanceInspectionAggregationError {
    fn from(error: MojoError) -> Self {
        Self::Abi(error)
    }
}

/// Compiled coverage/classification and the ordered unique source-local mask union.
#[derive(Debug, PartialEq, Eq)]
pub struct GovernanceInspectionAggregation {
    pub coverage: u8,
    pub classification: u8,
    pub masked_kinds: Vec<u8>,
}

#[repr(C)]
struct SourceFacts {
    coverage: i64,
    finding_address: u64,
    finding_count: i64,
    masked_address: u64,
    masked_count: i64,
    tag_count: i64,
    reason_count: i64,
}

unsafe extern "C" {
    fn prodex_mojo_governance_inspection_aggregate_v1(
        abi_version: i64,
        sources_address: u64,
        source_count: i64,
        default_classification: i64,
        output_address: u64,
    ) -> i64;
}

/// Aggregate at most eight sources; count failures are returned before reading finding tags.
/// Source coverage is 0..=2; classification is 0..=3; finding kinds are 0..=11.
pub fn governance_inspection_aggregate(
    sources: &[GovernanceInspectionSource<'_>],
    default_classification: u8,
) -> Result<GovernanceInspectionAggregation, GovernanceInspectionAggregationError> {
    if sources.len() > 8 {
        return Err(MojoError::InvalidInput.into());
    }
    let facts = sources
        .iter()
        .map(|source| {
            Ok(SourceFacts {
                coverage: i64::from(source.coverage),
                finding_address: source.finding_kinds.as_ptr() as u64,
                finding_count: i64::try_from(source.finding_kinds.len())
                    .map_err(|_| MojoError::InvalidInput)?,
                masked_address: source.masked_kinds.as_ptr() as u64,
                masked_count: i64::try_from(source.masked_kinds.len())
                    .map_err(|_| MojoError::InvalidInput)?,
                tag_count: i64::try_from(source.tag_count).map_err(|_| MojoError::InvalidInput)?,
                reason_count: i64::try_from(source.reason_count)
                    .map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let mut output = [-1_i64; 15];
    let status = unsafe {
        prodex_mojo_governance_inspection_aggregate_v1(
            6,
            facts.as_ptr() as u64,
            facts.len() as i64,
            i64::from(default_classification),
            output.as_mut_ptr() as u64,
        )
    };
    if status == 5 {
        return Err(GovernanceInspectionAggregationError::LimitExceeded);
    }
    super::governance_status(status)?;
    if !(0..=2).contains(&output[0])
        || !(0..=3).contains(&output[1])
        || !(0..=12).contains(&output[2])
    {
        return Err(MojoError::InvalidOutput.into());
    }
    let values = &output[3..3 + output[2] as usize];
    if values.iter().any(|value| !(0..=11).contains(value))
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(MojoError::InvalidOutput.into());
    }
    Ok(GovernanceInspectionAggregation {
        coverage: output[0] as u8,
        classification: output[1] as u8,
        masked_kinds: values.iter().map(|value| *value as u8).collect(),
    })
}

#[cfg(test)]
#[path = "inspection_aggregation_tests.rs"]
mod tests;
