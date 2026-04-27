use criterion::{Criterion, criterion_group};
use prodex::bench_support::{
    RuntimeProxyCompactSessionSelectionBenchCase, RuntimeProxyHotPathBenchCheckConfig,
    RuntimeProxyLineageCleanupBenchCase, RuntimeProxyMixedPoolSelectionBenchCase,
    RuntimeProxyPreviousResponseBenchCase, RuntimeProxyQuotaFallbackBenchCase,
    RuntimeProxySseInspectBenchCase, RuntimeProxyWebsocketStaleReuseBenchCase,
    run_runtime_proxy_hot_path_bench_check,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::Path;

const RUNTIME_PROXY_BENCH_CHECK_ENV: &str = "PRODEX_RUNTIME_PROXY_BENCH_CHECK";
const RUNTIME_PROXY_BENCH_THRESHOLD_SCALE_ENV: &str = "PRODEX_RUNTIME_PROXY_BENCH_THRESHOLD_SCALE";
const RUNTIME_PROXY_BENCH_THRESHOLD_FILE_ENV: &str = "PRODEX_RUNTIME_PROXY_BENCH_THRESHOLD_FILE";
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

    let compact_session_selection = RuntimeProxyCompactSessionSelectionBenchCase::new(64);
    c.bench_function("runtime_compact_session_affinity_selection", |b| {
        b.iter(|| black_box(compact_session_selection.select_compact_session_candidate()))
    });

    let websocket_stale_reuse = RuntimeProxyWebsocketStaleReuseBenchCase::new(64);
    c.bench_function("runtime_websocket_stale_reuse_affinity", |b| {
        b.iter(|| black_box(websocket_stale_reuse.evaluate_stale_reuse_affinity()))
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

#[derive(Debug, Deserialize)]
struct RuntimeProxyHotPathThresholdFile {
    #[serde(default)]
    version: Option<u64>,
    #[serde(default)]
    default_scale_percent: Option<u64>,
    #[serde(default)]
    case_scale_percent: BTreeMap<String, u64>,
}

fn load_threshold_file(path: &Path) -> Result<RuntimeProxyHotPathThresholdFile, String> {
    let raw = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "{RUNTIME_PROXY_BENCH_THRESHOLD_FILE_ENV} could not read {}: {error}",
            path.display()
        )
    })?;
    let file: RuntimeProxyHotPathThresholdFile = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{RUNTIME_PROXY_BENCH_THRESHOLD_FILE_ENV} could not parse {}: {error}",
            path.display()
        )
    })?;
    if let Some(version) = file.version
        && version != 1
    {
        return Err(format!(
            "{RUNTIME_PROXY_BENCH_THRESHOLD_FILE_ENV} unsupported version {} in {}",
            version,
            path.display()
        ));
    }
    Ok(file)
}

fn bench_check_config() -> Result<RuntimeProxyHotPathBenchCheckConfig, String> {
    let mut config = RuntimeProxyHotPathBenchCheckConfig::default();

    if let Some(path) = std::env::var_os(RUNTIME_PROXY_BENCH_THRESHOLD_FILE_ENV) {
        let threshold_file = load_threshold_file(Path::new(&path))?;
        if let Some(default_scale_percent) = threshold_file.default_scale_percent {
            config = config.with_threshold_scale_percent(default_scale_percent);
        }
        config = config.with_threshold_scale_overrides(threshold_file.case_scale_percent);
    }

    if std::env::var_os(RUNTIME_PROXY_BENCH_THRESHOLD_SCALE_ENV).is_some() {
        config = config.with_threshold_scale_percent(parse_threshold_scale_percent()?);
    }

    Ok(config)
}

fn main() {
    if bench_check_requested() {
        let config = match bench_check_config() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("runtime_proxy_hot_path_check status=error message={error}");
                std::process::exit(2);
            }
        };

        let results = run_runtime_proxy_hot_path_bench_check(config.clone());

        let mut failures = 0usize;
        for result in &results {
            if !result.passed() {
                failures += 1;
            }
            println!(
                "runtime_proxy_hot_path_check case={} status={} median_ns={} p90_ns={} threshold_ns={} base_threshold_ns={} threshold_scale_percent={} required_scale_percent={} scale_headroom_percent={} threshold_headroom_ns={} iterations={} warmup_iterations={} samples={}",
                result.name,
                if result.passed() { "pass" } else { "fail" },
                result.median_ns_per_iteration,
                result.p90_ns_per_iteration,
                result.threshold_ns_per_iteration,
                result.base_threshold_ns_per_iteration,
                result.threshold_scale_percent,
                result.required_threshold_scale_percent(),
                result.scale_headroom_percent(),
                result.threshold_headroom_ns_per_iteration(),
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
                    "threshold_scale_percent": result.threshold_scale_percent,
                    "required_scale_percent": result.required_threshold_scale_percent(),
                    "scale_headroom_percent": result.scale_headroom_percent(),
                    "threshold_headroom_ns": result.threshold_headroom_ns_per_iteration(),
                    "iterations": result.measure_iterations,
                    "warmup_iterations": result.warmup_iterations,
                    "samples": result.samples,
                })
            );
        }
        println!(
            "runtime_proxy_hot_path_check summary cases={} failures={} default_threshold_scale_percent={} threshold_scale_overrides={}",
            results.len(),
            failures,
            config.threshold_scale_percent,
            config.threshold_scale_overrides.len(),
        );
        println!(
            "{} {}",
            RUNTIME_PROXY_BENCH_CHECK_JSON_PREFIX,
            json!({
                "event": "summary",
                "cases": results.len(),
                "failures": failures,
                "default_threshold_scale_percent": config.threshold_scale_percent,
                "threshold_scale_overrides": config.threshold_scale_overrides,
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
