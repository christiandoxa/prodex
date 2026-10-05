#![cfg(feature = "mojo-observability")]

use prodex_mojo_core::operational_metrics::histogram_bucket_bounds;

#[test]
fn operational_histogram_bucket_plan_is_mojo_owned() {
    assert_eq!(
        histogram_bucket_bounds("prodex_api_request_duration_ms").unwrap(),
        [
            1, 2, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 30_000, 120_000
        ]
    );
    assert_eq!(
        histogram_bucket_bounds("prodex_inspection_duration_microseconds").unwrap(),
        [
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
            120_000_000
        ]
    );
    assert_eq!(
        histogram_bucket_bounds("duration_microseconds_suffix").unwrap(),
        [
            1, 2, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 30_000, 120_000
        ]
    );
}

#[test]
fn operational_histogram_bucket_plan_rejects_unbounded_names() {
    let long_name = "a".repeat(129);
    assert_eq!(
        histogram_bucket_bounds(&long_name),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}
