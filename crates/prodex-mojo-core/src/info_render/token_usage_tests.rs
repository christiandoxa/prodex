use super::*;

#[test]
fn token_aggregate_keeps_saturating_global_and_profile_totals() {
    let records = [
        InfoTokenUsageProfile {
            profile: "β",
            input_tokens: u64::MAX,
            cached_input_tokens: 2,
            output_tokens: 3,
            reasoning_tokens: 4,
        },
        InfoTokenUsageProfile {
            profile: "a",
            input_tokens: 10,
            cached_input_tokens: 20,
            output_tokens: 30,
            reasoning_tokens: 40,
        },
        InfoTokenUsageProfile {
            profile: "β",
            input_tokens: 1,
            cached_input_tokens: 5,
            output_tokens: 6,
            reasoning_tokens: 7,
        },
    ];
    assert_eq!(
        aggregate_info_token_usage(&records),
        Ok(InfoTokenUsageAggregate {
            event_count: 3,
            total: TokenUsageCounts {
                input_tokens: u64::MAX,
                cached_input_tokens: 27,
                output_tokens: 39,
                reasoning_tokens: 51
            },
            profiles: vec![
                InfoTokenUsageProfileSummary {
                    profile: "a",
                    event_count: 1,
                    total: TokenUsageCounts {
                        input_tokens: 10,
                        cached_input_tokens: 20,
                        output_tokens: 30,
                        reasoning_tokens: 40
                    }
                },
                InfoTokenUsageProfileSummary {
                    profile: "β",
                    event_count: 2,
                    total: TokenUsageCounts {
                        input_tokens: u64::MAX,
                        cached_input_tokens: 7,
                        output_tokens: 9,
                        reasoning_tokens: 11
                    }
                },
            ],
        })
    );
    assert_eq!(
        aggregate_info_token_usage(&[]),
        Ok(InfoTokenUsageAggregate {
            event_count: 0,
            total: TokenUsageCounts::default(),
            profiles: vec![],
        })
    );
}

#[test]
fn token_aggregate_abi_rejects_invalid_input_before_output() {
    let mut output = [71_u64; 13];
    for (abi, count, words, expected) in
        [(2, 0, 13, 4), (1, -1, 13, 1), (1, 0, 5, 3), (1, 1, 13, 1)]
    {
        assert_eq!(
            unsafe {
                prodex_info_token_usage_summary_v1(
                    abi,
                    0,
                    0,
                    count,
                    output.as_mut_ptr() as u64,
                    words,
                )
            },
            expected
        );
        assert_eq!(output, [71; 13]);
    }
}

#[test]
fn token_aggregate_sorts_large_groups_without_truncating_them() {
    let labels = (0..2048)
        .map(|index| format!("profile_{index:04}"))
        .collect::<Vec<_>>();
    let records = labels
        .iter()
        .rev()
        .map(|profile| InfoTokenUsageProfile {
            profile,
            input_tokens: 1,
            cached_input_tokens: 1,
            output_tokens: 1,
            reasoning_tokens: 1,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        aggregate_info_token_usage(&records),
        Ok(InfoTokenUsageAggregate {
            event_count: 2048,
            total: TokenUsageCounts {
                input_tokens: 2048,
                cached_input_tokens: 2048,
                output_tokens: 2048,
                reasoning_tokens: 2048
            },
            profiles: labels
                .iter()
                .map(|profile| InfoTokenUsageProfileSummary {
                    profile,
                    event_count: 1,
                    total: TokenUsageCounts {
                        input_tokens: 1,
                        cached_input_tokens: 1,
                        output_tokens: 1,
                        reasoning_tokens: 1,
                    },
                })
                .collect(),
        })
    );
}

#[test]
fn token_aggregate_abi_rejects_invalid_profile_spans_without_output_writes() {
    let profiles = [InfoStringView { ptr: 0, len: 1 }];
    let counts = [TokenUsageCounts::default()];
    let mut output = [71_u64; 13];
    assert_eq!(
        unsafe {
            prodex_info_token_usage_summary_v1(
                1,
                profiles.as_ptr() as u64,
                counts.as_ptr() as u64,
                1,
                output.as_mut_ptr() as u64,
                13,
            )
        },
        1
    );
    assert_eq!(output, [71; 13]);
}
