use super::*;

#[test]
fn runtime_proxy_hot_path_bench_check_config_normalizes_zero_values() {
    let config = RuntimeProxyHotPathBenchCheckConfig {
        samples: 0,
        warmup_iterations: 0,
        min_measure_iterations: 0,
        max_measure_iterations: 0,
        target_sample_time: Duration::ZERO,
        threshold_scale_percent: 0,
        threshold_scale_overrides: BTreeMap::from([("runtime_case".to_string(), 0)]),
    }
    .normalized();

    assert_eq!(config.samples, 1);
    assert_eq!(config.warmup_iterations, 1);
    assert_eq!(config.min_measure_iterations, 1);
    assert_eq!(config.max_measure_iterations, 1);
    assert_eq!(
        config.target_sample_time,
        Duration::from_millis(DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS)
    );
    assert_eq!(config.threshold_scale_percent, 1);
    assert_eq!(config.threshold_scale_percent_for("runtime_case"), 1);
}

#[test]
fn runtime_proxy_hot_path_bench_config_uses_case_override_thresholds() {
    let config = RuntimeProxyHotPathBenchCheckConfig::default()
        .with_threshold_scale_percent(150)
        .with_threshold_scale_overrides([("runtime_case", 180), ("runtime_other", 90)]);

    assert_eq!(config.threshold_scale_percent_for("runtime_case"), 180);
    assert_eq!(config.threshold_scale_percent_for("runtime_other"), 90);
    assert_eq!(config.threshold_scale_percent_for("runtime_missing"), 150);
}

#[test]
fn runtime_proxy_hot_path_bench_summary_uses_sorted_percentiles() {
    let result = summarize_runtime_proxy_hot_path_samples(RuntimeProxyHotPathSummaryInput {
        name: "runtime_case",
        samples: 5,
        warmup_iterations: 32,
        measure_iterations: 128,
        base_threshold_ns_per_iteration: 10,
        threshold_scale_percent: 70,
        threshold_ns_per_iteration: 7,
        ns_per_iteration_samples: vec![9, 3, 5, 1, 7],
    });

    assert_eq!(result.name, "runtime_case");
    assert_eq!(result.samples, 5);
    assert_eq!(result.warmup_iterations, 32);
    assert_eq!(result.measure_iterations, 128);
    assert_eq!(result.base_threshold_ns_per_iteration, 10);
    assert_eq!(result.threshold_scale_percent, 70);
    assert_eq!(result.median_ns_per_iteration, 5);
    assert_eq!(result.p90_ns_per_iteration, 7);
    assert_eq!(result.required_threshold_scale_percent(), 50);
    assert_eq!(result.scale_headroom_percent(), 20);
    assert_eq!(result.threshold_headroom_ns_per_iteration(), 2);
    assert!(result.passed());
}

#[test]
fn runtime_proxy_hot_path_bench_case_specs_keep_expected_order() {
    let names = RUNTIME_PROXY_HOT_PATH_BENCH_CASE_SPECS
        .iter()
        .map(|spec| spec.name)
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "runtime_route_eligible_quota_fallback_scan",
            "runtime_previous_response_candidate_selection",
            "runtime_mixed_pool_response_selection",
            "runtime_compact_session_affinity_selection",
            "runtime_websocket_stale_reuse_affinity",
            "runtime_sse_lookahead_inspection",
            "runtime_dead_lineage_cleanup",
            "runtime_smart_context_large_tool_output_rewrite",
            "runtime_mem_super_slim_token_heavy_shadow",
        ]
    );
    assert!(
        RUNTIME_PROXY_HOT_PATH_BENCH_CASE_SPECS
            .iter()
            .all(|spec| spec.name == spec.threshold.name && spec.default_size > 0)
    );

    assert_eq!(
        RuntimeProxyHotPathBenchScenarioSizes::default(),
        RuntimeProxyHotPathBenchScenarioSizes {
            quota_fallback: 64,
            previous_response: 64,
            mixed_pool_selection: 96,
            compact_session_selection: 64,
            websocket_stale_reuse: 64,
            sse_inspect: 128,
            lineage_cleanup: 128,
            smart_context_rewrite: 96,
            runtime_mem_super_slim: 96,
        }
    );
}
