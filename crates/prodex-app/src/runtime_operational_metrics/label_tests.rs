use super::{
    RuntimeOperationalMetricKey, RuntimeOperationalMetricRegistry, runtime_operational_metric_key,
};
use prodex_observability::TelemetryAttribute;
use std::collections::BTreeMap;

#[test]
fn metric_key_keeps_pair_order_duplicates_and_empty_labels() {
    let response = TelemetryAttribute::metric_label("route", "responses");
    let compact = TelemetryAttribute::metric_label("route", "compact");
    let status = TelemetryAttribute::metric_label("status_class", "ok");
    assert_eq!(
        runtime_operational_metric_key("metric", &[&status, &response, &compact, &response]),
        Some(RuntimeOperationalMetricKey {
            name: "metric",
            labels: vec![
                ("route".into(), "compact".into()),
                ("route".into(), "responses".into()),
                ("route".into(), "responses".into()),
                ("status_class".into(), "ok".into()),
            ],
        })
    );
    assert_eq!(
        runtime_operational_metric_key("metric", &[]),
        Some(RuntimeOperationalMetricKey {
            name: "metric",
            labels: vec![]
        })
    );
}

#[test]
fn reversed_labels_share_counter_and_histogram_entries() {
    let registry = RuntimeOperationalMetricRegistry::default();
    let route = TelemetryAttribute::metric_label("route", "responses");
    let status = TelemetryAttribute::metric_label("status_class", "ok");
    registry.record("metric", 3, &[&status, &route]);
    registry.record("metric", 5, &[&route, &status]);
    registry.observe_histogram("metric", 3, &[&route, &status]);
    registry.observe_histogram("metric", 7, &[&status, &route]);
    let key = RuntimeOperationalMetricKey {
        name: "metric",
        labels: vec![
            ("route".into(), "responses".into()),
            ("status_class".into(), "ok".into()),
        ],
    };
    assert_eq!(
        *registry.counters.lock().unwrap(),
        BTreeMap::from([(key.clone(), 8)])
    );
    let histograms = registry.histograms.lock().unwrap();
    assert_eq!(histograms.len(), 1);
    let histogram = histograms.get(&key).unwrap();
    assert_eq!((histogram.count, histogram.sum), (2, 10));
}

#[test]
fn invalid_labels_drop_observations_before_registry_mutation() {
    let registry = RuntimeOperationalMetricRegistry::default();
    let valid = TelemetryAttribute::metric_label("route", "responses");
    for invalid in [
        TelemetryAttribute::metric_label("request_id", "fixture"),
        TelemetryAttribute::metric_label("route", "has spaces"),
    ] {
        assert_eq!(
            runtime_operational_metric_key("metric", &[&valid, &invalid]),
            None
        );
        registry.record("metric", 1, &[&valid, &invalid]);
        registry.observe_histogram("metric", 1, &[&valid, &invalid]);
    }
    assert!(registry.counters.lock().unwrap().is_empty());
    assert!(registry.histograms.lock().unwrap().is_empty());
}
