use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_SAMPLES: usize = 7;
pub const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_WARMUP_ITERATIONS: usize = 64;
pub const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MIN_MEASURE_ITERATIONS: usize = 32;
pub const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MAX_MEASURE_ITERATIONS: usize = 4_096;
pub const DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS: u64 = 15;
pub const BENCH_THRESHOLD_SCALE_DIVISOR: u64 = 100;

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchCheckConfig {
    pub samples: usize,
    pub warmup_iterations: usize,
    pub min_measure_iterations: usize,
    pub max_measure_iterations: usize,
    pub target_sample_time: Duration,
    pub threshold_scale_percent: u64,
    pub threshold_scale_overrides: BTreeMap<String, u64>,
}

impl Default for RuntimeProxyHotPathBenchCheckConfig {
    fn default() -> Self {
        Self {
            samples: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_SAMPLES,
            warmup_iterations: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_WARMUP_ITERATIONS,
            min_measure_iterations: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MIN_MEASURE_ITERATIONS,
            max_measure_iterations: DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_MAX_MEASURE_ITERATIONS,
            target_sample_time: Duration::from_millis(
                DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS,
            ),
            threshold_scale_percent: BENCH_THRESHOLD_SCALE_DIVISOR,
            threshold_scale_overrides: BTreeMap::new(),
        }
    }
}

impl RuntimeProxyHotPathBenchCheckConfig {
    pub fn with_threshold_scale_percent(mut self, threshold_scale_percent: u64) -> Self {
        self.threshold_scale_percent = threshold_scale_percent.max(1);
        self
    }

    pub fn with_threshold_scale_overrides<I, K>(mut self, threshold_scale_overrides: I) -> Self
    where
        I: IntoIterator<Item = (K, u64)>,
        K: Into<String>,
    {
        self.threshold_scale_overrides = threshold_scale_overrides
            .into_iter()
            .map(|(case_name, scale_percent)| (case_name.into(), scale_percent.max(1)))
            .collect();
        self
    }

    pub fn threshold_scale_percent_for(&self, case_name: &str) -> u64 {
        self.threshold_scale_overrides
            .get(case_name)
            .copied()
            .unwrap_or(self.threshold_scale_percent)
            .max(1)
    }

    pub fn normalized(self) -> Self {
        let min_measure_iterations = self.min_measure_iterations.max(1);
        let max_measure_iterations = self.max_measure_iterations.max(min_measure_iterations);
        let target_sample_time = if self.target_sample_time.is_zero() {
            Duration::from_millis(DEFAULT_RUNTIME_PROXY_HOT_PATH_BENCH_TARGET_SAMPLE_TIME_MS)
        } else {
            self.target_sample_time
        };

        Self {
            samples: self.samples.max(1),
            warmup_iterations: self.warmup_iterations.max(1),
            min_measure_iterations,
            max_measure_iterations,
            target_sample_time,
            threshold_scale_percent: self.threshold_scale_percent.max(1),
            threshold_scale_overrides: self
                .threshold_scale_overrides
                .into_iter()
                .map(|(case_name, scale_percent)| (case_name, scale_percent.max(1)))
                .collect(),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchCheckResult {
    pub name: &'static str,
    pub samples: usize,
    pub warmup_iterations: usize,
    pub measure_iterations: usize,
    pub base_threshold_ns_per_iteration: u64,
    pub threshold_scale_percent: u64,
    pub median_ns_per_iteration: u64,
    pub p90_ns_per_iteration: u64,
    pub threshold_ns_per_iteration: u64,
}

impl RuntimeProxyHotPathBenchCheckResult {
    pub fn passed(&self) -> bool {
        self.median_ns_per_iteration <= self.threshold_ns_per_iteration
    }

    pub fn required_threshold_scale_percent(&self) -> u64 {
        if self.base_threshold_ns_per_iteration == 0 {
            return u64::MAX;
        }
        let required_percent = self
            .median_ns_per_iteration
            .saturating_mul(BENCH_THRESHOLD_SCALE_DIVISOR)
            .saturating_add(self.base_threshold_ns_per_iteration - 1)
            / self.base_threshold_ns_per_iteration;
        required_percent.max(1)
    }

    pub fn scale_headroom_percent(&self) -> i128 {
        i128::from(self.threshold_scale_percent)
            - i128::from(self.required_threshold_scale_percent())
    }

    pub fn threshold_headroom_ns_per_iteration(&self) -> i128 {
        i128::from(self.threshold_ns_per_iteration) - i128::from(self.median_ns_per_iteration)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchThreshold {
    pub name: &'static str,
    pub max_median_ns_per_iteration: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchCaseSpec {
    pub name: &'static str,
    pub default_size: usize,
    pub threshold: RuntimeProxyHotPathBenchThreshold,
}

pub const RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_route_eligible_quota_fallback_scan",
        default_size: 64,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_route_eligible_quota_fallback_scan",
            max_median_ns_per_iteration: 30_000,
        },
    };

pub const RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_previous_response_candidate_selection",
        default_size: 64,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_previous_response_candidate_selection",
            max_median_ns_per_iteration: 110_000,
        },
    };

pub const RUNTIME_PROXY_MIXED_POOL_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_mixed_pool_response_selection",
        default_size: 96,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_mixed_pool_response_selection",
            max_median_ns_per_iteration: 2_400_000,
        },
    };

pub const RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_compact_session_affinity_selection",
        default_size: 64,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_compact_session_affinity_selection",
            max_median_ns_per_iteration: 160_000,
        },
    };

pub const RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_websocket_stale_reuse_affinity",
        default_size: 64,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_websocket_stale_reuse_affinity",
            max_median_ns_per_iteration: 220_000,
        },
    };

pub const RUNTIME_PROXY_SSE_INSPECT_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_sse_lookahead_inspection",
        default_size: 128,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_sse_lookahead_inspection",
            max_median_ns_per_iteration: 130_000,
        },
    };

pub const RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_CASE: RuntimeProxyHotPathBenchCaseSpec =
    RuntimeProxyHotPathBenchCaseSpec {
        name: "runtime_dead_lineage_cleanup",
        default_size: 128,
        threshold: RuntimeProxyHotPathBenchThreshold {
            name: "runtime_dead_lineage_cleanup",
            max_median_ns_per_iteration: 190_000,
        },
    };

pub const RUNTIME_PROXY_HOT_PATH_BENCH_CASE_SPECS: [RuntimeProxyHotPathBenchCaseSpec; 7] = [
    RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_CASE,
    RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_CASE,
    RUNTIME_PROXY_MIXED_POOL_BENCH_CASE,
    RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_CASE,
    RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_CASE,
    RUNTIME_PROXY_SSE_INSPECT_BENCH_CASE,
    RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_CASE,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProxyHotPathBenchScenarioSizes {
    pub quota_fallback: usize,
    pub previous_response: usize,
    pub mixed_pool_selection: usize,
    pub compact_session_selection: usize,
    pub websocket_stale_reuse: usize,
    pub sse_inspect: usize,
    pub lineage_cleanup: usize,
}

impl RuntimeProxyHotPathBenchScenarioSizes {
    pub const fn from_default_case_specs() -> Self {
        Self {
            quota_fallback: RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_CASE.default_size,
            previous_response: RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_CASE.default_size,
            mixed_pool_selection: RUNTIME_PROXY_MIXED_POOL_BENCH_CASE.default_size,
            compact_session_selection: RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_CASE
                .default_size,
            websocket_stale_reuse: RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_CASE.default_size,
            sse_inspect: RUNTIME_PROXY_SSE_INSPECT_BENCH_CASE.default_size,
            lineage_cleanup: RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_CASE.default_size,
        }
    }
}

impl Default for RuntimeProxyHotPathBenchScenarioSizes {
    fn default() -> Self {
        Self::from_default_case_specs()
    }
}

pub fn scaled_runtime_proxy_hot_path_threshold_ns(
    threshold: RuntimeProxyHotPathBenchThreshold,
    threshold_scale_percent: u64,
) -> u64 {
    threshold
        .max_median_ns_per_iteration
        .saturating_mul(threshold_scale_percent.max(1))
        / BENCH_THRESHOLD_SCALE_DIVISOR
}

fn measure_runtime_proxy_hot_path_batch<T, F>(iterations: usize, mut step: F) -> u128
where
    F: FnMut() -> T,
{
    let started_at = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(step());
    }
    started_at.elapsed().as_nanos()
}

pub struct RuntimeProxyHotPathSummaryInput {
    pub name: &'static str,
    pub samples: usize,
    pub warmup_iterations: usize,
    pub measure_iterations: usize,
    pub base_threshold_ns_per_iteration: u64,
    pub threshold_scale_percent: u64,
    pub threshold_ns_per_iteration: u64,
    pub ns_per_iteration_samples: Vec<u64>,
}

pub fn summarize_runtime_proxy_hot_path_samples(
    input: RuntimeProxyHotPathSummaryInput,
) -> RuntimeProxyHotPathBenchCheckResult {
    let RuntimeProxyHotPathSummaryInput {
        name,
        samples,
        warmup_iterations,
        measure_iterations,
        base_threshold_ns_per_iteration,
        threshold_scale_percent,
        threshold_ns_per_iteration,
        mut ns_per_iteration_samples,
    } = input;
    ns_per_iteration_samples.sort_unstable();
    let median_ns_per_iteration = ns_per_iteration_samples[ns_per_iteration_samples.len() / 2];
    let p90_index = ((ns_per_iteration_samples.len() - 1) * 9) / 10;
    let p90_ns_per_iteration = ns_per_iteration_samples[p90_index];

    RuntimeProxyHotPathBenchCheckResult {
        name,
        samples,
        warmup_iterations,
        measure_iterations,
        base_threshold_ns_per_iteration,
        threshold_scale_percent,
        median_ns_per_iteration,
        p90_ns_per_iteration,
        threshold_ns_per_iteration,
    }
}

pub fn run_runtime_proxy_hot_path_case<T, F>(
    config: RuntimeProxyHotPathBenchCheckConfig,
    threshold: RuntimeProxyHotPathBenchThreshold,
    mut step: F,
) -> RuntimeProxyHotPathBenchCheckResult
where
    F: FnMut() -> T,
{
    let threshold_scale_percent = config.threshold_scale_percent_for(threshold.name);
    let threshold_ns_per_iteration =
        scaled_runtime_proxy_hot_path_threshold_ns(threshold, threshold_scale_percent);
    for _ in 0..config.warmup_iterations {
        std::hint::black_box(step());
    }

    let pilot_iterations = config.min_measure_iterations.min(64);
    let pilot_elapsed_ns = measure_runtime_proxy_hot_path_batch(pilot_iterations, &mut step);
    let pilot_ns_per_iteration =
        (pilot_elapsed_ns / u128::from(pilot_iterations.max(1) as u64)).max(1);
    let desired_iterations = (config.target_sample_time.as_nanos() / pilot_ns_per_iteration)
        .try_into()
        .unwrap_or(usize::MAX);
    let measure_iterations =
        desired_iterations.clamp(config.min_measure_iterations, config.max_measure_iterations);

    let mut ns_per_iteration_samples = Vec::with_capacity(config.samples);
    for _ in 0..config.samples {
        let elapsed_ns = measure_runtime_proxy_hot_path_batch(measure_iterations, &mut step);
        let ns_per_iteration = (elapsed_ns / u128::from(measure_iterations as u64))
            .try_into()
            .unwrap_or(u64::MAX);
        ns_per_iteration_samples.push(ns_per_iteration);
    }

    summarize_runtime_proxy_hot_path_samples(RuntimeProxyHotPathSummaryInput {
        name: threshold.name,
        samples: config.samples,
        warmup_iterations: config.warmup_iterations,
        measure_iterations,
        base_threshold_ns_per_iteration: threshold.max_median_ns_per_iteration,
        threshold_scale_percent,
        threshold_ns_per_iteration,
        ns_per_iteration_samples,
    })
}

#[doc(hidden)]
pub struct RuntimeProxyHotPathBenchCaseSuite<
    QuotaFallback,
    PreviousResponse,
    MixedPoolSelection,
    CompactSessionSelection,
    WebsocketStaleReuse,
    SseInspect,
    LineageCleanup,
> {
    pub quota_fallback: QuotaFallback,
    pub previous_response: PreviousResponse,
    pub mixed_pool_selection: MixedPoolSelection,
    pub compact_session_selection: CompactSessionSelection,
    pub websocket_stale_reuse: WebsocketStaleReuse,
    pub sse_inspect: SseInspect,
    pub lineage_cleanup: LineageCleanup,
}

#[doc(hidden)]
#[allow(clippy::type_complexity)]
pub fn run_runtime_proxy_hot_path_case_suite<
    QuotaFallback,
    QuotaFallbackResult,
    PreviousResponse,
    PreviousResponseResult,
    MixedPoolSelection,
    MixedPoolSelectionResult,
    CompactSessionSelection,
    CompactSessionSelectionResult,
    WebsocketStaleReuse,
    WebsocketStaleReuseResult,
    SseInspect,
    SseInspectResult,
    LineageCleanup,
    LineageCleanupResult,
>(
    config: RuntimeProxyHotPathBenchCheckConfig,
    mut suite: RuntimeProxyHotPathBenchCaseSuite<
        QuotaFallback,
        PreviousResponse,
        MixedPoolSelection,
        CompactSessionSelection,
        WebsocketStaleReuse,
        SseInspect,
        LineageCleanup,
    >,
) -> Vec<RuntimeProxyHotPathBenchCheckResult>
where
    QuotaFallback: FnMut() -> QuotaFallbackResult,
    PreviousResponse: FnMut() -> PreviousResponseResult,
    MixedPoolSelection: FnMut() -> MixedPoolSelectionResult,
    CompactSessionSelection: FnMut() -> CompactSessionSelectionResult,
    WebsocketStaleReuse: FnMut() -> WebsocketStaleReuseResult,
    SseInspect: FnMut() -> SseInspectResult,
    LineageCleanup: FnMut() -> LineageCleanupResult,
{
    let config = config.normalized();

    vec![
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_CASE.threshold,
            || (suite.quota_fallback)(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_CASE.threshold,
            || (suite.previous_response)(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_MIXED_POOL_BENCH_CASE.threshold,
            || (suite.mixed_pool_selection)(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_CASE.threshold,
            || (suite.compact_session_selection)(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_CASE.threshold,
            || (suite.websocket_stale_reuse)(),
        ),
        run_runtime_proxy_hot_path_case(
            config.clone(),
            RUNTIME_PROXY_SSE_INSPECT_BENCH_CASE.threshold,
            || (suite.sse_inspect)(),
        ),
        run_runtime_proxy_hot_path_case(
            config,
            RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_CASE.threshold,
            || (suite.lineage_cleanup)(),
        ),
    ]
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
