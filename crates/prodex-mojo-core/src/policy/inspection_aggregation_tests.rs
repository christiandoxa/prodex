use super::*;
use GovernanceInspectionAggregationError::{Abi, LimitExceeded};

fn source<'a>(
    coverage: u8,
    findings: &'a [u8],
    masked: &'a [u8],
) -> GovernanceInspectionSource<'a> {
    GovernanceInspectionSource {
        coverage,
        finding_kinds: findings,
        masked_kinds: masked,
        tag_count: 0,
        reason_count: 0,
    }
}

#[test]
fn compiled_inspection_aggregation_keeps_source_local_mask_intersection_and_kind_order() {
    assert_eq!(
        governance_inspection_aggregate(&[source(0, &[0], &[8]), source(2, &[8], &[0]),], 0),
        Ok(GovernanceInspectionAggregation {
            coverage: 1,
            classification: 3,
            masked_kinds: vec![],
        })
    );
    let reversed = (0..12).rev().collect::<Vec<u8>>();
    assert_eq!(
        governance_inspection_aggregate(
            &[source(0, &reversed, &reversed), source(0, &[0], &[0, 0]),],
            0
        ),
        Ok(GovernanceInspectionAggregation {
            coverage: 0,
            classification: 3,
            masked_kinds: (0..12).collect(),
        })
    );
}

#[test]
fn compiled_inspection_aggregation_preserves_defaults_minimums_and_old_coverage_facade() {
    for default in 0..=3 {
        assert_eq!(
            governance_inspection_aggregate(&[], default),
            Ok(GovernanceInspectionAggregation {
                coverage: 2,
                classification: default,
                masked_kinds: vec![],
            })
        );
    }
    for (kind, expected) in [
        (0, 2),
        (1, 2),
        (2, 2),
        (3, 2),
        (4, 3),
        (5, 3),
        (6, 3),
        (7, 3),
        (8, 3),
        (9, 3),
        (10, 3),
        (11, 2),
    ] {
        for (default, classification) in [(0, expected), (3, 3)] {
            let plan =
                governance_inspection_aggregate(&[source(0, &[kind], &[kind])], default).unwrap();
            assert_eq!(plan.classification, classification);
            assert_eq!(plan.masked_kinds, vec![kind]);
        }
    }
    for (left, right, expected) in [
        (0, 0, 0),
        (0, 1, 1),
        (0, 2, 1),
        (1, 0, 1),
        (1, 1, 1),
        (1, 2, 1),
        (2, 0, 1),
        (2, 1, 1),
        (2, 2, 2),
    ] {
        assert_eq!(
            super::super::governance_coverage_combine(left, right),
            Ok(expected)
        );
        let plan =
            governance_inspection_aggregate(&[source(left, &[], &[]), source(right, &[], &[])], 1)
                .unwrap();
        assert_eq!((plan.coverage, plan.classification), (expected, 1));
    }
}

#[test]
fn inspection_aggregation_is_bounded_and_rejects_unknown_numeric_tags() {
    for facts in [
        source(3, &[], &[]),
        source(0, &[12], &[]),
        source(0, &[], &[12]),
        source(0, &[], &[0; 13]),
    ] {
        assert_eq!(
            governance_inspection_aggregate(&[facts], 0),
            Err(Abi(MojoError::InvalidInput))
        );
    }
    assert_eq!(
        governance_inspection_aggregate(&[], 4),
        Err(Abi(MojoError::InvalidInput))
    );
    let sources = (0..9).map(|_| source(0, &[], &[])).collect::<Vec<_>>();
    assert_eq!(
        governance_inspection_aggregate(&sources, 0),
        Err(Abi(MojoError::InvalidInput))
    );
    assert_eq!(
        governance_inspection_aggregate(&sources[..8], 0)
            .unwrap()
            .coverage,
        0
    );
    let kinds = vec![0; 256];
    assert_eq!(
        governance_inspection_aggregate(&[source(0, &kinds, &[0; 12])], 0),
        Ok(GovernanceInspectionAggregation {
            coverage: 0,
            classification: 2,
            masked_kinds: vec![0]
        })
    );
    assert_eq!(
        governance_inspection_aggregate(&[source(0, &vec![0; 65_537], &[])], 0),
        Err(LimitExceeded)
    );
}

#[test]
fn inspection_aggregation_abi_checks_late_records_before_writing_results() {
    assert_eq!(std::mem::size_of::<SourceFacts>(), 56);
    assert_eq!(std::mem::offset_of!(SourceFacts, masked_address), 24);
    let invalid = [12_u8];
    let mut sources = [
        SourceFacts {
            coverage: 0,
            finding_address: 0,
            finding_count: 0,
            masked_address: 0,
            masked_count: 0,
            tag_count: 0,
            reason_count: 0,
        },
        SourceFacts {
            coverage: 0,
            finding_address: invalid.as_ptr() as u64,
            finding_count: 1,
            masked_address: 0,
            masked_count: 0,
            tag_count: 0,
            reason_count: 0,
        },
    ];
    let mut output = [73_i64; 15];
    let mut call = |version, table, count, classification| unsafe {
        prodex_mojo_governance_inspection_aggregate_v1(
            version,
            table,
            count,
            classification,
            output.as_mut_ptr() as u64,
        )
    };
    let table = sources.as_ptr() as u64;
    assert_eq!(call(7, 0, 0, 0), 4);
    assert_eq!(call(6, 0, 1, 0), 1);
    assert_eq!(call(6, table + 1, 1, 0), 1);
    assert_eq!(call(6, table, -1, 0), 1);
    assert_eq!(call(6, table, 9, 0), 1);
    assert_eq!(call(6, table, 1, 4), 1);
    assert_eq!(call(6, table, 2, 0), 1);
    sources[1].finding_address = 0;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 1);
    sources[1].finding_address = invalid.as_ptr() as u64;
    sources[1].finding_count = 65_537;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 5);
    sources[1].finding_count = -1;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 1);
    sources[1].finding_count = 0;
    sources[1].tag_count = -1;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 1);
    sources[1].tag_count = 0;
    sources[1].reason_count = -1;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 1);
    sources[1].reason_count = 0;
    sources[1].masked_count = 1;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 1);
    sources[1].masked_address = invalid.as_ptr() as u64;
    assert_eq!(call(6, sources.as_ptr() as u64, 2, 0), 1);
    assert_eq!(output, [73; 15]);
}

#[test]
fn compiled_aggregate_counts_match_domain_limits_before_scanning_invalid_payloads() {
    let kinds = [0_u8; 128];
    let mut sources = [source(0, &kinds, &[]), source(0, &kinds, &[])];
    for facts in &mut sources {
        facts.tag_count = 16;
        facts.reason_count = 16;
    }
    assert_eq!(
        governance_inspection_aggregate(&sources, 0)
            .unwrap()
            .classification,
        2
    );
    sources[1].tag_count = 17;
    assert_eq!(
        governance_inspection_aggregate(&sources, 0),
        Err(LimitExceeded)
    );
    sources[1].tag_count = 16;
    sources[1].reason_count = 17;
    assert_eq!(
        governance_inspection_aggregate(&sources, 0),
        Err(LimitExceeded)
    );
    let invalid = [255_u8; 257];
    sources[1].reason_count = 16;
    sources[1].finding_kinds = &invalid;
    assert_eq!(
        governance_inspection_aggregate(&sources, 0),
        Err(LimitExceeded)
    );
    assert_eq!(
        super::super::governance_inspection_result_counts_valid(256, 32, 32, 256, 32, 32),
        Ok(true)
    );
    assert_eq!(
        super::super::governance_inspection_result_counts_valid(257, 32, 32, 256, 32, 32),
        Ok(false)
    );
}
