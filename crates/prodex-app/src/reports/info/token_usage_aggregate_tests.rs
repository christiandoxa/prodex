use super::super::InfoTokenUsageProfile;
use super::*;
use std::collections::BTreeMap;

#[test]
fn token_aggregation_groups_completed_events_across_logs() {
    let first = concat!(
        "token_usage profile=β input_tokens=18446744073709551615 cached_input_tokens=2 output_tokens=3 reasoning_tokens=4\n",
        "token_usage_progress profile=ignored input_tokens=10000 output_tokens=10000\n",
        "token_usage profile=invalid input_tokens=bad output_tokens=10\n",
    );
    let second = concat!(
        "token_usage profile=a input_tokens=10 cached_input_tokens=20 output_tokens=30 reasoning_tokens=40\n",
        "token_usage profile=β input_tokens=1 cached_input_tokens=5 output_tokens=6 reasoning_tokens=7\n",
    );
    assert_eq!(
        collect_info_token_usage_summary_from_texts(17, [first, second]),
        InfoTokenUsageSummary {
            log_count: 17,
            event_count: 3,
            total: InfoTokenUsageCounts {
                input_tokens: u64::MAX,
                cached_input_tokens: 27,
                output_tokens: 39,
                reasoning_tokens: 51
            },
            by_profile: BTreeMap::from([
                (
                    "a".into(),
                    InfoTokenUsageProfile {
                        event_count: 1,
                        total: InfoTokenUsageCounts {
                            input_tokens: 10,
                            cached_input_tokens: 20,
                            output_tokens: 30,
                            reasoning_tokens: 40,
                        }
                    }
                ),
                (
                    "β".into(),
                    InfoTokenUsageProfile {
                        event_count: 2,
                        total: InfoTokenUsageCounts {
                            input_tokens: u64::MAX,
                            cached_input_tokens: 7,
                            output_tokens: 9,
                            reasoning_tokens: 11,
                        }
                    }
                ),
            ]),
        }
    );
}

#[test]
fn token_aggregation_preserves_zero_events_and_the_supplied_log_count() {
    assert_eq!(
        collect_info_token_usage_summary_from_texts(
            9,
            ["", "token_usage_progress output_tokens=15"]
        ),
        InfoTokenUsageSummary {
            log_count: 9,
            ..InfoTokenUsageSummary::default()
        }
    );
}

#[test]
fn token_aggregation_saturates_all_four_counters_across_source_logs() {
    let texts = [u64::MAX - 1, 2, 7]
        .into_iter()
        .enumerate()
        .map(|(index, count)| {
            serde_json::json!({"event": "token_usage", "fields": {
                "profile": if index < 2 { "a" } else { "b" }, "input_tokens": count,
                "cached_input_tokens": count, "output_tokens": count, "reasoning_tokens": count,
            }})
            .to_string()
        })
        .collect::<Vec<_>>();
    let max = InfoTokenUsageCounts {
        input_tokens: u64::MAX,
        cached_input_tokens: u64::MAX,
        output_tokens: u64::MAX,
        reasoning_tokens: u64::MAX,
    };
    let seven = InfoTokenUsageCounts {
        input_tokens: 7,
        cached_input_tokens: 7,
        output_tokens: 7,
        reasoning_tokens: 7,
    };
    assert_eq!(
        collect_info_token_usage_summary_from_texts(3, texts),
        InfoTokenUsageSummary {
            log_count: 3,
            event_count: 3,
            total: max,
            by_profile: BTreeMap::from([
                (
                    "a".into(),
                    InfoTokenUsageProfile {
                        event_count: 2,
                        total: max
                    }
                ),
                (
                    "b".into(),
                    InfoTokenUsageProfile {
                        event_count: 1,
                        total: seven
                    }
                ),
            ]),
        }
    );
}
