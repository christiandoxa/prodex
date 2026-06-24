use super::*;

fn replay_metric(
    scenario_id: &str,
    variant: SmartContextReplayVariant,
    input_tokens: u64,
) -> SmartContextReplayScenarioMetrics {
    SmartContextReplayScenarioMetrics {
        scenario_id: scenario_id.to_string(),
        variant,
        scenario_tags: Vec::new(),
        context_window_tokens: None,
        eligible: true,
        turns: 30,
        input_tokens,
        total_tokens_until_completion: input_tokens.saturating_add(1_000),
        completion_success: true,
        test_or_build_passed: true,
        critical_signal_recall_percent: 100,
        continuation_integrity_percent: 100,
        tool_call_integrity_percent: 100,
        missing_context_recovery_turns: 0,
        full_request_fallback: false,
        unresolved_mandatory_artifact_refs: 0,
        corrupted_json: false,
        rewrite_overhead_ms: 10,
        explicit_exact_mode: false,
        unsafe_request: false,
    }
}

fn replay_metric_with_coverage(
    scenario_id: &str,
    variant: SmartContextReplayVariant,
    input_tokens: u64,
    tags: &[&str],
    context_window_tokens: u64,
) -> SmartContextReplayScenarioMetrics {
    SmartContextReplayScenarioMetrics {
        scenario_tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        context_window_tokens: Some(context_window_tokens),
        ..replay_metric(scenario_id, variant, input_tokens)
    }
}

#[test]
fn replay_evaluation_passes_acceptance_thresholds() {
    let metrics = vec![
        replay_metric("a", SmartContextReplayVariant::Exact, 10_000),
        replay_metric("a", SmartContextReplayVariant::Current, 8_000),
        replay_metric_with_coverage(
            "a",
            SmartContextReplayVariant::Optimized,
            6_000,
            &[
                "repeated_build_test_output",
                "compiler_runtime_errors",
                "large_git_diffs",
            ],
            16_384,
        ),
        replay_metric("b", SmartContextReplayVariant::Exact, 20_000),
        replay_metric("b", SmartContextReplayVariant::Current, 16_000),
        replay_metric_with_coverage(
            "b",
            SmartContextReplayVariant::Optimized,
            10_000,
            &[
                "repository_navigation",
                "multi_file_refactoring",
                "changing_static_instructions",
            ],
            32_000,
        ),
        replay_metric("c", SmartContextReplayVariant::Exact, 30_000),
        replay_metric("c", SmartContextReplayVariant::Current, 24_000),
        replay_metric_with_coverage(
            "c",
            SmartContextReplayVariant::Optimized,
            15_000,
            &[
                "missing_corrupted_artifacts",
                "duplicate_tool_calls_outputs",
                "noisy_binary_like_commands",
            ],
            128_000,
        ),
        replay_metric("d", SmartContextReplayVariant::Exact, 40_000),
        replay_metric("d", SmartContextReplayVariant::Current, 32_000),
        replay_metric_with_coverage(
            "d",
            SmartContextReplayVariant::Optimized,
            20_000,
            &["failure_recovery"],
            200_000,
        ),
    ];

    let evaluation = smart_context_evaluate_replay_corpus(&metrics);
    let report = smart_context_render_replay_evaluation_markdown(&evaluation);

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(
        evaluation
            .acceptance_thresholds
            .min_median_input_token_reduction_percent_vs_exact,
        35
    );
    assert_eq!(
        evaluation
            .acceptance_thresholds
            .max_continuation_fallback_rate_percent,
        19
    );
    assert_eq!(evaluation.eligible_long_sessions, 4);
    assert_eq!(evaluation.current_comparison_sessions, 4);
    assert_eq!(evaluation.median_input_token_reduction_percent_vs_exact, 50);
    assert_eq!(
        evaluation.current_median_input_token_reduction_percent_vs_exact,
        20
    );
    assert_eq!(
        evaluation.median_additional_input_token_reduction_percent_vs_current,
        37
    );
    assert_eq!(
        evaluation.long_sessions_with_at_least_20_percent_reduction_percent,
        100
    );
    assert_eq!(evaluation.current_success_rate_percent, 100);
    assert!(report.contains("# Smart Context Replay Evaluation"));
    assert!(report.contains("## Acceptance Thresholds"));
    assert!(report.contains("- min_median_input_token_reduction_percent_vs_exact: 35"));
    assert!(report.contains("## Measurements"));
    assert!(report.contains("- current_comparison_sessions: 4"));
    assert!(report.contains("- median_additional_input_token_reduction_percent_vs_current: 37"));
    assert!(report.contains("- required_replay_coverage: complete"));
    assert!(report.contains("- passed: true"));
}

#[test]
fn replay_evaluation_reports_failing_scenarios_and_thresholds() {
    let mut exact = replay_metric("bad", SmartContextReplayVariant::Exact, 10_000);
    exact.completion_success = true;
    let mut optimized = replay_metric("bad", SmartContextReplayVariant::Optimized, 9_500);
    optimized.completion_success = false;
    optimized.critical_signal_recall_percent = 99;
    optimized.continuation_integrity_percent = 99;
    optimized.tool_call_integrity_percent = 99;
    optimized.unresolved_mandatory_artifact_refs = 1;
    optimized.corrupted_json = true;
    optimized.rewrite_overhead_ms = 35;
    optimized.full_request_fallback = true;

    let evaluation = smart_context_evaluate_replay_corpus(&[exact, optimized]);
    let report = smart_context_render_replay_evaluation_markdown(&evaluation);

    assert!(!evaluation.passed);
    assert!(
        evaluation.failures.iter().any(|failure| {
            failure.criterion == "median_input_token_reduction_percent_vs_exact"
        })
    );
    assert!(
        evaluation
            .failures
            .iter()
            .any(|failure| { failure.criterion == "critical_signal_recall_percent" })
    );
    assert!(
        evaluation
            .failures
            .iter()
            .any(|failure| { failure.criterion == "unresolved_mandatory_artifact_refs" })
    );
    assert!(
        evaluation
            .failures
            .iter()
            .any(|failure| { failure.criterion == "required_replay_coverage" })
    );
    assert_eq!(evaluation.scenario_failures.len(), 1);
    assert_eq!(evaluation.scenario_failures[0].scenario_id, "bad");
    assert!(
        evaluation.scenario_failures[0]
            .criteria
            .iter()
            .any(|criterion| criterion == "completion_success=false")
    );
    assert!(
        evaluation.scenario_failures[0]
            .criteria
            .iter()
            .any(|criterion| criterion == "corrupted_json=true")
    );
    assert!(report.contains("## Failures"));
    assert!(report.contains("## Scenario Failures"));
    assert!(report.contains("- bad:"));
    assert!(report.contains("p95_rewrite_overhead_ms"));
}

#[test]
fn replay_evaluation_excludes_explicit_exact_and_unsafe_from_reduction_pool() {
    let exact = replay_metric("exact-mode", SmartContextReplayVariant::Exact, 10_000);
    let mut optimized = replay_metric("exact-mode", SmartContextReplayVariant::Optimized, 1_000);
    optimized.explicit_exact_mode = true;

    let evaluation = smart_context_evaluate_replay_corpus(&[exact, optimized]);

    assert_eq!(evaluation.eligible_long_sessions, 0);
    assert!(!evaluation.passed);
    assert!(
        evaluation.failures.iter().any(|failure| {
            failure.criterion == "median_input_token_reduction_percent_vs_exact"
        })
    );
}

#[test]
fn replay_evaluation_reduction_math_is_bounded_for_varied_token_counts() {
    for exact_tokens in [1, 2, 10, 99, 100, 101, 1_000, 10_000] {
        for optimized_tokens in [0, 1, 2, 9, 10, 50, 99, 100, 999, 10_000, 20_000] {
            let exact = replay_metric("scenario", SmartContextReplayVariant::Exact, exact_tokens);
            let optimized = replay_metric(
                "scenario",
                SmartContextReplayVariant::Optimized,
                optimized_tokens,
            );

            let evaluation = smart_context_evaluate_replay_corpus(&[exact, optimized]);

            assert!(
                evaluation.median_input_token_reduction_percent_vs_exact <= 100,
                "{evaluation:?}"
            );
            if optimized_tokens >= exact_tokens {
                assert_eq!(
                    evaluation.median_input_token_reduction_percent_vs_exact, 0,
                    "{evaluation:?}"
                );
            }
        }
    }
}

#[test]
fn replay_evaluation_percent_metrics_stay_bounded_for_generated_corpus() {
    let mut metrics = Vec::new();
    for index in 0..64u64 {
        let scenario_id = format!("scenario-{index}");
        let exact_tokens = 10_000 + index.saturating_mul(137);
        let optimized_tokens = exact_tokens.saturating_sub(index.saturating_mul(83));
        metrics.push(replay_metric(
            &scenario_id,
            SmartContextReplayVariant::Exact,
            exact_tokens,
        ));
        let mut optimized = replay_metric(
            &scenario_id,
            SmartContextReplayVariant::Optimized,
            optimized_tokens,
        );
        optimized.completion_success = index % 17 != 0;
        optimized.test_or_build_passed = index % 19 != 0;
        optimized.critical_signal_recall_percent = 100u8.saturating_sub((index % 3) as u8);
        optimized.continuation_integrity_percent = 100u8.saturating_sub((index % 2) as u8);
        optimized.tool_call_integrity_percent = 100u8.saturating_sub((index % 4) as u8);
        optimized.full_request_fallback = index % 11 == 0;
        optimized.rewrite_overhead_ms = index.saturating_mul(2);
        metrics.push(optimized);
    }

    let evaluation = smart_context_evaluate_replay_corpus(&metrics);

    assert!(evaluation.median_input_token_reduction_percent_vs_exact <= 100);
    assert!(evaluation.long_sessions_with_at_least_20_percent_reduction_percent <= 100);
    assert!(evaluation.exact_success_rate_percent <= 100);
    assert!(evaluation.current_success_rate_percent <= 100);
    assert!(evaluation.optimized_success_rate_percent <= 100);
    assert!(evaluation.continuation_integrity_percent <= 100);
    assert!(evaluation.tool_call_integrity_percent <= 100);
    assert!(evaluation.critical_signal_recall_percent <= 100);
    assert!(evaluation.continuation_fallback_rate_percent <= 100);
}

#[test]
fn replay_evaluation_reports_missing_exact_baseline_for_eligible_optimized() {
    let optimized = replay_metric(
        "missing-baseline",
        SmartContextReplayVariant::Optimized,
        5_000,
    );

    let evaluation = smart_context_evaluate_replay_corpus(&[optimized]);
    let report = smart_context_render_replay_evaluation_markdown(&evaluation);

    assert_eq!(evaluation.scenario_failures.len(), 1);
    assert_eq!(
        evaluation.scenario_failures[0].criteria,
        vec!["missing_exact_baseline".to_string()]
    );
    assert!(report.contains("missing-baseline: missing_exact_baseline"));
}

#[test]
fn replay_corpus_json_fixture_renders_acceptance_report() {
    let corpus_text = include_str!("../../fixtures/smart_context_replay_corpus.json");

    let corpus = smart_context_parse_replay_corpus_json(corpus_text).expect("valid corpus");
    let evaluation =
        smart_context_evaluate_replay_corpus_json(corpus_text).expect("valid evaluation");
    let report = smart_context_render_replay_corpus_markdown(corpus_text).expect("valid report");

    assert_eq!(corpus.metrics.len(), 36);
    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.eligible_long_sessions, 12);
    assert_eq!(evaluation.current_comparison_sessions, 12);
    assert!(evaluation.missing_required_coverage.is_empty());
    assert!(report.contains("- passed: true"));
    assert!(report.contains("- median_input_token_reduction_percent_vs_exact: 45"));
    assert!(report.contains("- current_median_input_token_reduction_percent_vs_exact: 18"));
    assert!(report.contains("- required_replay_coverage: complete"));
}

#[test]
fn replay_corpus_json_accepts_top_level_metrics_array() {
    let metrics = vec![
        replay_metric("array", SmartContextReplayVariant::Exact, 10_000),
        replay_metric("array", SmartContextReplayVariant::Optimized, 5_000),
    ];
    let text = serde_json::to_string(&metrics).expect("metrics should serialize");

    let corpus = smart_context_parse_replay_corpus_json(&text).expect("array corpus");
    let evaluation = smart_context_evaluate_replay_corpus(&corpus.metrics);

    assert_eq!(corpus.metrics.len(), 2);
    assert!(!evaluation.passed, "{evaluation:?}");
    assert!(
        evaluation
            .failures
            .iter()
            .any(|failure| failure.criterion == "required_replay_coverage")
    );
}
