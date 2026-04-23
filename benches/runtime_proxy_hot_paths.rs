use criterion::{Criterion, black_box, criterion_group};
use prodex::bench_support::{
    RuntimeProxyHotPathBenchCheckConfig, RuntimeProxyLineageCleanupBenchCase,
    RuntimeProxyMixedPoolSelectionBenchCase, RuntimeProxyPreviousResponseBenchCase,
    RuntimeProxyQuotaFallbackBenchCase, RuntimeProxySseInspectBenchCase,
    run_runtime_proxy_hot_path_bench_check,
};
use serde_json::json;

const RUNTIME_PROXY_BENCH_CHECK_ENV: &str = "PRODEX_RUNTIME_PROXY_BENCH_CHECK";
const RUNTIME_PROXY_BENCH_THRESHOLD_SCALE_ENV: &str = "PRODEX_RUNTIME_PROXY_BENCH_THRESHOLD_SCALE";
const RUNTIME_PROXY_BENCH_CHECK_JSON_PREFIX: &str = "runtime_proxy_hot_path_check_json";

fn runtime_proxy_hot_paths(c: &mut Criterion) {
    let quota_fallback = RuntimeProxyQuotaFallbackBenchCase::new(64);
    c.bench_function("runtime_route_eligible_quota_fallback_scan", |b| {
        b.iter(|| black_box(quota_fallback.has_route_eligible_quota_fallback()))
    });

    let previous_response = RuntimeProxyPreviousResponseBenchCase::new(64);
    c.bench_function("runtime_previous_response_candidate_selection", |b| {
        b.iter(|| black_box(previous_response.next_previous_response_candidate()))
    });

    let mixed_pool_selection = RuntimeProxyMixedPoolSelectionBenchCase::new(96);
    c.bench_function("runtime_mixed_pool_response_selection", |b| {
        b.iter(|| black_box(mixed_pool_selection.select_fresh_response_candidate()))
    });

    let sse_inspect = RuntimeProxySseInspectBenchCase::new(128);
    c.bench_function("runtime_sse_lookahead_inspection", |b| {
        b.iter(|| black_box(sse_inspect.inspect()))
    });

    let lineage_cleanup = RuntimeProxyLineageCleanupBenchCase::new(128);
    c.bench_function("runtime_dead_lineage_cleanup", |b| {
        b.iter(|| black_box(lineage_cleanup.clear_dead_response_bindings()))
    });
}

criterion_group!(benches, runtime_proxy_hot_paths);

fn bench_check_requested() -> bool {
    std::env::var_os(RUNTIME_PROXY_BENCH_CHECK_ENV)
        .map(|value| value != "0")
        .unwrap_or(false)
}

fn parse_threshold_scale_percent() -> Result<u64, String> {
    let Some(raw_value) = std::env::var_os(RUNTIME_PROXY_BENCH_THRESHOLD_SCALE_ENV) else {
        return Ok(100);
    };

    let raw_value = raw_value.to_string_lossy();
    raw_value.parse::<u64>().map_err(|error| {
        format!("{RUNTIME_PROXY_BENCH_THRESHOLD_SCALE_ENV} must be an integer percent: {error}")
    })
}

fn main() {
    if bench_check_requested() {
        let threshold_scale_percent = match parse_threshold_scale_percent() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("runtime_proxy_hot_path_check status=error message={error}");
                std::process::exit(2);
            }
        };

        let results = run_runtime_proxy_hot_path_bench_check(
            RuntimeProxyHotPathBenchCheckConfig::default()
                .with_threshold_scale_percent(threshold_scale_percent),
        );

        let mut failures = 0usize;
        for result in &results {
            if !result.passed() {
                failures += 1;
            }
            println!(
                "runtime_proxy_hot_path_check case={} status={} median_ns={} p90_ns={} threshold_ns={} base_threshold_ns={} required_scale_percent={} iterations={} warmup_iterations={} samples={}",
                result.name,
                if result.passed() { "pass" } else { "fail" },
                result.median_ns_per_iteration,
                result.p90_ns_per_iteration,
                result.threshold_ns_per_iteration,
                result.base_threshold_ns_per_iteration,
                result.required_threshold_scale_percent(),
                result.measure_iterations,
                result.warmup_iterations,
                result.samples,
            );
            println!(
                "{} {}",
                RUNTIME_PROXY_BENCH_CHECK_JSON_PREFIX,
                json!({
                    "event": "case",
                    "case": result.name,
                    "status": if result.passed() { "pass" } else { "fail" },
                    "median_ns": result.median_ns_per_iteration,
                    "p90_ns": result.p90_ns_per_iteration,
                    "threshold_ns": result.threshold_ns_per_iteration,
                    "base_threshold_ns": result.base_threshold_ns_per_iteration,
                    "required_scale_percent": result.required_threshold_scale_percent(),
                    "threshold_scale_percent": threshold_scale_percent,
                    "iterations": result.measure_iterations,
                    "warmup_iterations": result.warmup_iterations,
                    "samples": result.samples,
                })
            );
        }
        println!(
            "runtime_proxy_hot_path_check summary cases={} failures={} threshold_scale_percent={}",
            results.len(),
            failures,
            threshold_scale_percent,
        );
        println!(
            "{} {}",
            RUNTIME_PROXY_BENCH_CHECK_JSON_PREFIX,
            json!({
                "event": "summary",
                "cases": results.len(),
                "failures": failures,
                "threshold_scale_percent": threshold_scale_percent,
            })
        );
        if failures > 0 {
            std::process::exit(1);
        }
        return;
    }

    benches();
    Criterion::default().configure_from_args().final_summary();
}
