//! Borrowed records for compiled Presidio analyzer ordering and overlap reduction.
use crate::MojoError;

/// Host-decoded analyzer facts; language metadata stays with the source record.
#[derive(Debug, Clone, Copy)]
pub struct AnalyzerFinding<'a> {
    pub start: usize,
    pub end: usize,
    pub score: f64,
    pub entity: &'a str,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct FindingFfi {
    start: u64,
    end: u64,
    score: f64,
    entity_address: u64,
    entity_length: u64,
}

/// Mojo-selected source metadata and possibly expanded overlap range.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C)]
pub struct AnalyzerSelection {
    pub source: u64,
    pub start: u64,
    pub end: u64,
    pub score: f64,
}

const _: () = {
    assert!(std::mem::size_of::<FindingFfi>() == 40);
    assert!(std::mem::size_of::<AnalyzerSelection>() == 32);
};

unsafe extern "C" {
    fn prodex_redaction_analyzer_merge_v1(
        abi: i64,
        findings: u64,
        count: i64,
        order: u64,
        output: u64,
        capacity: i64,
        selected: u64,
    ) -> i64;
}

/// Stable selection with exact duplicate, containment, and partial-overlap rules.
pub fn analyzer_merge_plan(
    findings: &[AnalyzerFinding<'_>],
) -> Result<Vec<AnalyzerSelection>, MojoError> {
    let count = i64::try_from(findings.len()).map_err(|_| MojoError::InvalidInput)?;
    let input = findings
        .iter()
        .map(|finding| FindingFfi {
            start: finding.start as u64,
            end: finding.end as u64,
            score: finding.score,
            entity_address: finding.entity.as_ptr() as usize as u64,
            entity_length: finding.entity.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut order = vec![0_i64; findings.len()];
    let mut output = vec![AnalyzerSelection::default(); findings.len()];
    let mut selected = -1_i64;
    super::status(unsafe {
        prodex_redaction_analyzer_merge_v1(
            1,
            input.as_ptr() as usize as u64,
            count,
            order.as_mut_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            count,
            (&mut selected as *mut i64) as usize as u64,
        )
    })?;
    let selected = usize::try_from(selected).map_err(|_| MojoError::InvalidOutput)?;
    if selected > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(selected);
    for item in &output {
        let source = usize::try_from(item.source)
            .ok()
            .and_then(|index| findings.get(index))
            .ok_or(MojoError::InvalidOutput)?;
        if item.start > item.end
            || item.score != source.score
            || usize::try_from(item.start).is_err()
            || usize::try_from(item.end).is_err()
        {
            return Err(MojoError::InvalidOutput);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_analyzer_merge_rejects_malformed_ranges_and_abi() {
        assert_eq!(analyzer_merge_plan(&[]), Ok(vec![]));
        for (start, end, score) in [(2, 1, 0.9), (0, 1, f64::NAN), (0, 1, f64::INFINITY)] {
            assert_eq!(
                analyzer_merge_plan(&[AnalyzerFinding {
                    start,
                    end,
                    score,
                    entity: "EMAIL_ADDRESS"
                }]),
                Err(MojoError::InvalidInput)
            );
        }
        let mut selected = 71_i64;
        assert_eq!(
            unsafe {
                prodex_redaction_analyzer_merge_v1(
                    2,
                    0,
                    0,
                    0,
                    0,
                    0,
                    (&mut selected as *mut i64) as usize as u64,
                )
            },
            4
        );
        assert_eq!(selected, 71);
    }
}
