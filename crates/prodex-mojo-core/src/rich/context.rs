use std::collections::BTreeSet;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextGroup {
    pub normalized_key: String,
    pub kind: i64,
    pub severity: i64,
    pub first_line: usize,
    pub occurrences: usize,
    pub duplicate_count: usize,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextAnalysis {
    pub line_count: usize,
    pub counts: [usize; 7],
    pub noise_lines: usize,
    pub signal_lines: usize,
    pub token_count: usize,
    pub groups: Vec<ContextGroup>,
}

const SIGNAL_COUNT_BATCH_MAX: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_rich_context_signal_counts_batch_v1(
        inputs: u64,
        outputs: u64,
        count: i64,
    ) -> i64;
}

/// Classifies a bounded batch of already-separated context lines in one Mojo call.
pub fn signal_counts_batch(lines: &[&str]) -> Result<Vec<[usize; 7]>, MojoError> {
    ensure_rich_abi()?;
    if lines.len() > SIGNAL_COUNT_BATCH_MAX {
        return Err(MojoError::InvalidInput);
    }
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    let views = lines.iter().map(|line| view(line)).collect::<Vec<_>>();
    let mut output = vec![0_i64; lines.len() * 7];
    let status = unsafe {
        prodex_mojo_rich_context_signal_counts_batch_v1(
            mojo_pointer_address(views.as_ptr()),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(lines.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(if status == 2 {
            MojoError::InvalidInput
        } else {
            MojoError::InvalidOutput
        });
    }
    output
        .as_chunks::<7>()
        .0
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| usize::try_from(*value).map_err(|_| MojoError::InvalidOutput))
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| MojoError::InvalidOutput)
        })
        .collect()
}

pub fn analyze_context(input: &str) -> Result<ContextAnalysis, MojoError> {
    ensure_rich_abi()?;
    let line_capacity = input
        .bytes()
        .filter(|byte| matches!(byte, b'\n' | b'\r'))
        .count()
        .saturating_add(1);
    let record_capacity = line_capacity.max(1);
    let scratch_capacity = hash_capacity(record_capacity)?;
    let mut records = vec![RichContextRecord::default(); record_capacity];
    let mut output = vec![0_u8; input.len().max(1)];
    let mut hash_slots = vec![-1_i64; scratch_capacity];
    let mut result = RichContextResult::default();
    let input_view = view(input);
    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            mojo_pointer_address(&input_view),
            mojo_pointer_address(records.as_mut_ptr()),
            i64::try_from(line_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(output.as_mut_ptr()),
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(hash_slots.as_mut_ptr()),
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(
            status,
            1,
            result.issue_kind,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.abi_version != RICH_ABI_VERSION
        || result.line_count < 0
        || result.records_written < 0
        || result.records_written as usize > line_capacity
        || result.output_written < 0
        || result.output_written as usize > input.len()
        || result.noise_lines < 0
        || result.signal_lines < 0
        || result.signal_lines as usize > result.line_count as usize
        || result.token_count < 0
        || result.counts.iter().any(|value| *value < 0)
    {
        return Err(MojoError::InvalidOutput);
    }
    let output = &output[..result.output_written as usize];
    let mut groups = Vec::with_capacity(result.records_written as usize);
    let mut keys = BTreeSet::new();
    for record in &records[..result.records_written as usize] {
        let key = slice(output, record.key)?;
        let normalized_key = std::str::from_utf8(key)
            .map_err(|_| MojoError::InvalidOutput)?
            .to_string();
        let first_line =
            usize::try_from(record.first_line).map_err(|_| MojoError::InvalidOutput)?;
        let occurrences =
            usize::try_from(record.occurrences).map_err(|_| MojoError::InvalidOutput)?;
        let duplicate_count =
            usize::try_from(record.duplicate_count).map_err(|_| MojoError::InvalidOutput)?;
        let token_count =
            usize::try_from(record.token_count).map_err(|_| MojoError::InvalidOutput)?;
        if first_line == 0
            || first_line > result.line_count as usize
            || occurrences == 0
            || duplicate_count != occurrences.saturating_sub(1)
            || !keys.insert(normalized_key.clone())
        {
            return Err(MojoError::InvalidOutput);
        }
        groups.push(ContextGroup {
            normalized_key,
            kind: record.kind,
            severity: record.severity,
            first_line,
            occurrences,
            duplicate_count,
            token_count,
        });
    }
    Ok(ContextAnalysis {
        line_count: result.line_count as usize,
        counts: result
            .counts
            .map(|value| usize::try_from(value).map_err(|_| MojoError::InvalidOutput))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| MojoError::InvalidOutput)?,
        noise_lines: result.noise_lines as usize,
        signal_lines: result.signal_lines as usize,
        token_count: result.token_count as usize,
        groups,
    })
}
