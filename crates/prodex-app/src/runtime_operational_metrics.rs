use prodex_domain::TelemetryAttribute;
use prodex_observability::{
    ApiAdmissionResult, ApiRouteKind, ApiStatusClass, InspectionMetricPlan, ProviderKind,
    ProviderResultClass, SecretProviderBackend, SecretProviderOperation, SecretProviderResult,
    plan_api_admission_metric, plan_api_red_metric, plan_provider_metric,
    plan_secret_provider_metric,
};
use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RuntimeOperationalMetricKey {
    name: &'static str,
    labels: Vec<(String, String)>,
}

#[derive(Default)]
struct RuntimeOperationalMetricRegistry {
    counters: Mutex<BTreeMap<RuntimeOperationalMetricKey, u64>>,
    histograms: Mutex<BTreeMap<RuntimeOperationalMetricKey, RuntimeOperationalHistogram>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RuntimeOperationalHistogram {
    bucket_bounds: Vec<u64>,
    bucket_counts: Vec<u64>,
    count: u64,
    sum: u64,
}

static RUNTIME_OPERATIONAL_METRICS: LazyLock<RuntimeOperationalMetricRegistry> =
    LazyLock::new(RuntimeOperationalMetricRegistry::default);

impl RuntimeOperationalMetricRegistry {
    fn record(&self, name: &'static str, increment: u64, labels: &[&TelemetryAttribute]) {
        let Some(key) = runtime_operational_metric_key(name, labels) else {
            return;
        };
        let mut counters = self
            .counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let counter = counters.entry(key).or_default();
        *counter = counter.saturating_add(increment);
    }

    fn observe_histogram(
        &self,
        name: &'static str,
        observation: u64,
        labels: &[&TelemetryAttribute],
    ) {
        let Some(key) = runtime_operational_metric_key(name, labels) else {
            return;
        };
        let mut histograms = self
            .histograms
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let histogram = histograms.entry(key).or_insert_with(|| {
            let bucket_bounds = runtime_operational_histogram_bounds(name).to_vec();
            RuntimeOperationalHistogram {
                bucket_counts: vec![0; bucket_bounds.len()],
                bucket_bounds,
                ..Default::default()
            }
        });
        histogram.count = histogram.count.saturating_add(1);
        histogram.sum = histogram.sum.saturating_add(observation);
        for (bound, count) in histogram
            .bucket_bounds
            .iter()
            .zip(histogram.bucket_counts.iter_mut())
        {
            if observation <= *bound {
                *count = count.saturating_add(1);
            }
        }
    }
}

fn runtime_operational_histogram_bounds(name: &str) -> &'static [u64] {
    if name.ends_with("_microseconds") {
        &[
            100,
            250,
            500,
            1_000,
            2_500,
            5_000,
            10_000,
            25_000,
            50_000,
            100_000,
            250_000,
            500_000,
            1_000_000,
            5_000_000,
            30_000_000,
            120_000_000,
        ]
    } else {
        &[
            1, 2, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 30_000, 120_000,
        ]
    }
}

fn runtime_operational_metric_key(
    name: &'static str,
    labels: &[&TelemetryAttribute],
) -> Option<RuntimeOperationalMetricKey> {
    let mut metric_labels = Vec::with_capacity(labels.len());
    for label in labels {
        let (key, value) = label.as_metric_label().ok()?;
        metric_labels.push((key.to_string(), value.to_string()));
    }
    metric_labels.sort();
    Some(RuntimeOperationalMetricKey {
        name,
        labels: metric_labels,
    })
}

pub(crate) fn record_runtime_api_red_metric(
    route: ApiRouteKind,
    status_class: ApiStatusClass,
    duration_ms: u64,
) {
    let Ok(plan) = plan_api_red_metric(route, status_class, duration_ms) else {
        return;
    };
    let labels = [&plan.route_label, &plan.status_label];
    RUNTIME_OPERATIONAL_METRICS.record(plan.request_count_metric_name, plan.increment, &labels);
    RUNTIME_OPERATIONAL_METRICS.observe_histogram(
        plan.duration_metric_name,
        plan.duration_ms,
        &labels,
    );
}

pub(crate) fn record_runtime_api_admission_metric(route: ApiRouteKind, result: ApiAdmissionResult) {
    let Ok(plan) = plan_api_admission_metric(route, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[&plan.route_label, &plan.result_label],
    );
}

pub(crate) fn record_runtime_secret_provider_metric(
    backend: SecretProviderBackend,
    operation: SecretProviderOperation,
    result: SecretProviderResult,
) {
    let Ok(plan) = plan_secret_provider_metric(backend, operation, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[
            &plan.backend_label,
            &plan.operation_label,
            &plan.result_label,
        ],
    );
}

pub(crate) fn record_runtime_inspection_metric(plan: &InspectionMetricPlan) {
    let labels = [
        &plan.stage_label,
        &plan.coverage_label,
        &plan.finding_category_label,
        &plan.masking_action_label,
        &plan.outcome_label,
    ];
    RUNTIME_OPERATIONAL_METRICS.record(plan.event_metric_name, plan.increment, &labels);
    RUNTIME_OPERATIONAL_METRICS.observe_histogram(
        plan.duration_metric_name,
        plan.duration_micros,
        &labels,
    );
}

pub(crate) fn record_runtime_provider_metric(
    provider: ProviderKind,
    result: ProviderResultClass,
    duration_ms: u64,
) {
    let Ok(plan) = plan_provider_metric(provider, result, duration_ms) else {
        return;
    };
    let labels = [&plan.provider_label, &plan.result_label];
    RUNTIME_OPERATIONAL_METRICS.record(plan.request_count_metric_name, plan.increment, &labels);
    RUNTIME_OPERATIONAL_METRICS.observe_histogram(
        plan.duration_metric_name,
        plan.duration_ms,
        &labels,
    );
}
