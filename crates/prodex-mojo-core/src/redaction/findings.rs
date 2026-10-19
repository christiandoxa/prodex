//! Typed projection of analyzer findings onto inspected JSON field text.
use super::DryRunPathView;
use super::analyzer::{AnalyzerFinding, finding_records};
use crate::MojoError;

/// Full localization or selection for the remote anonymizer's field filter.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum FindingLocalizationMode {
    Localize,
    RetainWithinValues,
}

/// Versioned, validated field index, byte range and domain finding tags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FindingLocation {
    pub source: u64,
    pub value_index: u64,
    pub start_byte: u64,
    pub end_byte: u64,
    pub confidence: u64,
    pub kind: u64,
}

/// Semantic failures retain the production caller's existing diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingLocalizationError {
    Abi(MojoError),
    CrossFieldBoundary,
    InvalidConfidence,
    OffsetOverflow,
}

unsafe extern "C" {
    fn prodex_redaction_findings_plan_v1(
        abi: i64,
        mode: i64,
        findings: u64,
        count: i64,
        values: u64,
        value_count: i64,
        separator: u64,
        separator_length: i64,
        ranges: u64,
        output: u64,
        capacity: i64,
        selected: u64,
    ) -> i64;
}

/// Mojo owns selection, scalar/byte offsets, confidence and entity classification.
pub fn finding_localization_plan(
    findings: &[AnalyzerFinding<'_>],
    values: &[&str],
    separator: &str,
    mode: FindingLocalizationMode,
) -> Result<Vec<FindingLocation>, FindingLocalizationError> {
    use FindingLocalizationError::{Abi, CrossFieldBoundary, InvalidConfidence, OffsetOverflow};
    let signed = |value| i64::try_from(value).map_err(|_| Abi(MojoError::InvalidInput));
    let input = finding_records(findings);
    let views = values
        .iter()
        .map(|value| DryRunPathView {
            address: value.as_ptr() as usize as u64,
            length: value.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut ranges = vec![[0_u64; 2]; values.len()];
    let mut output = vec![FindingLocation::default(); findings.len()];
    let mut selected = -1_i64;
    let code = unsafe {
        prodex_redaction_findings_plan_v1(
            1,
            mode as i64,
            input.as_ptr() as usize as u64,
            signed(input.len())?,
            views.as_ptr() as usize as u64,
            signed(views.len())?,
            separator.as_ptr() as usize as u64,
            signed(separator.len())?,
            ranges.as_mut_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut selected as *mut i64) as usize as u64,
        )
    };
    match code {
        0 => {}
        5 => return Err(CrossFieldBoundary),
        6 => return Err(InvalidConfidence),
        7 => return Err(OffsetOverflow),
        _ => super::status(code).map_err(Abi)?,
    }
    let selected = usize::try_from(selected).map_err(|_| Abi(MojoError::InvalidOutput))?;
    if selected > output.len()
        || (matches!(mode, FindingLocalizationMode::Localize) && selected != findings.len())
    {
        return Err(Abi(MojoError::InvalidOutput));
    }
    output.truncate(selected);
    if output
        .windows(2)
        .any(|pair| pair[0].source >= pair[1].source)
    {
        return Err(Abi(MojoError::InvalidOutput));
    }
    for item in &output {
        let text = usize::try_from(item.value_index)
            .ok()
            .and_then(|index| values.get(index))
            .ok_or(Abi(MojoError::InvalidOutput))?;
        if item.source >= findings.len() as u64
            || item.start_byte > item.end_byte
            || item.end_byte > text.len() as u64
            || item.confidence > 10000
            || item.kind >= 12
            || !text.is_char_boundary(item.start_byte as usize)
            || !text.is_char_boundary(item.end_byte as usize)
        {
            return Err(Abi(MojoError::InvalidOutput));
        }
    }
    Ok(output)
}

const _: () = {
    assert!(std::mem::size_of::<FindingLocation>() == 48);
    assert!(std::mem::size_of::<usize>() == 8);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_finding_localization_preserves_unicode_and_confidence_rounding() {
        let findings = [AnalyzerFinding {
            start: 2,
            end: 4,
            score: 0.99,
            entity: "NATIONAL_ID",
        }];
        assert_eq!(
            finding_localization_plan(
                &findings,
                &["é 🧭東"],
                "",
                FindingLocalizationMode::Localize
            ),
            Ok(vec![FindingLocation {
                source: 0,
                value_index: 0,
                start_byte: 3,
                end_byte: 10,
                confidence: 9900,
                kind: 4,
            }])
        );
        for score in [
            0.0,
            -0.0,
            1.0,
            0.00005,
            0.00005_f64.next_down(),
            0.00005_f64.next_up(),
            0.99995,
        ] {
            let finding = [AnalyzerFinding {
                start: 0,
                end: 1,
                score,
                entity: "CUSTOM_ENTITY",
            }];
            let output =
                finding_localization_plan(&finding, &["a"], "", FindingLocalizationMode::Localize)
                    .unwrap();
            assert_eq!(
                output[0].confidence,
                (score * 10000.0).round() as u64,
                "{score:?}"
            );
            assert_eq!(output[0].kind, 11);
        }
    }

    #[test]
    fn compiled_finding_localization_filters_cross_field_spans_and_rejects_bad_scores() {
        let findings = [
            AnalyzerFinding {
                start: 1,
                end: 4,
                score: 0.9,
                entity: "PERSON",
            },
            AnalyzerFinding {
                start: 3,
                end: 5,
                score: 0.5,
                entity: "EMAIL_ADDRESS",
            },
        ];
        assert_eq!(
            finding_localization_plan(
                &findings,
                &["ab", "cd"],
                "|",
                FindingLocalizationMode::Localize
            ),
            Err(FindingLocalizationError::CrossFieldBoundary)
        );
        let filtered = finding_localization_plan(
            &findings,
            &["ab", "cd"],
            "|",
            FindingLocalizationMode::RetainWithinValues,
        )
        .unwrap();
        assert_eq!(
            filtered.iter().map(|item| item.source).collect::<Vec<_>>(),
            [1]
        );
        let finding = [AnalyzerFinding {
            start: 0,
            end: 1,
            score: f64::NAN,
            entity: "PERSON",
        }];
        assert_eq!(
            finding_localization_plan(&finding, &["a"], "", FindingLocalizationMode::Localize),
            Err(FindingLocalizationError::InvalidConfidence)
        );
    }

    #[test]
    fn compiled_finding_confidence_matches_rust_at_every_half_step() {
        for index in 0..10000 {
            let midpoint = (f64::from(index) + 0.5) / 10000.0;
            for score in [midpoint.next_down(), midpoint, midpoint.next_up()] {
                let finding = [AnalyzerFinding {
                    start: 0,
                    end: 1,
                    score,
                    entity: "PERSON",
                }];
                let plan = finding_localization_plan(
                    &finding,
                    &["a"],
                    "",
                    FindingLocalizationMode::Localize,
                )
                .unwrap();
                assert_eq!(
                    plan[0].confidence,
                    (score * 10000.0).round() as u64,
                    "{score:?}"
                );
            }
        }
    }

    #[test]
    fn compiled_finding_offsets_count_unicode_separator_scalars() {
        let finding = [AnalyzerFinding {
            start: 2,
            end: 3,
            score: 0.5,
            entity: "PERSON",
        }];
        assert_eq!(
            finding_localization_plan(
                &finding,
                &["é", "🧭"],
                "🔐",
                FindingLocalizationMode::Localize
            ),
            Ok(vec![FindingLocation {
                source: 0,
                value_index: 1,
                start_byte: 0,
                end_byte: 4,
                confidence: 5000,
                kind: 2,
            }])
        );
    }

    #[test]
    fn compiled_finding_localization_rejects_invalid_abi_and_shape_before_publication() {
        let mut selected = 71_i64;
        let address = (&mut selected as *mut i64) as usize as u64;
        assert_eq!(
            unsafe { prodex_redaction_findings_plan_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, address) },
            4
        );
        assert_eq!(selected, 71);
        assert_eq!(
            unsafe { prodex_redaction_findings_plan_v1(1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, address) },
            1
        );
        assert_eq!(selected, 71);
        assert_eq!(
            unsafe { prodex_redaction_findings_plan_v1(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, address) },
            1
        );
        assert_eq!(selected, 71);
    }
}
