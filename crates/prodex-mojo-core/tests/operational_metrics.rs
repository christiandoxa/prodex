#![cfg(feature = "mojo-observability")]

use prodex_mojo_core::operational_metrics::{histogram_bucket_bounds, observe_histogram};

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

#[test]
fn operational_histogram_observation_is_mojo_owned() {
    let bounds = [5_u64, 10, 25];
    let mut counts = [0_u64; 3];
    let mut count = 0_u64;
    let mut sum = 0_u64;

    observe_histogram(5, &bounds, &mut counts, &mut count, &mut sum).unwrap();
    observe_histogram(11, &bounds, &mut counts, &mut count, &mut sum).unwrap();

    assert_eq!(counts, [1, 1, 2]);
    assert_eq!(count, 2);
    assert_eq!(sum, 16);
}

#[test]
fn operational_histogram_observation_saturates_and_rejects_shape_mismatch() {
    let bounds = [5_u64, 10];
    let mut counts = [u64::MAX, 0];
    let mut count = u64::MAX;
    let mut sum = u64::MAX - 2;

    observe_histogram(5, &bounds, &mut counts, &mut count, &mut sum).unwrap();
    assert_eq!(counts, [u64::MAX, 1]);
    assert_eq!(count, u64::MAX);
    assert_eq!(sum, u64::MAX);

    let before_counts = counts;
    let before_count = count;
    let before_sum = sum;
    assert_eq!(
        observe_histogram(1, &bounds, &mut counts[..1], &mut count, &mut sum),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
    assert_eq!(counts, before_counts);
    assert_eq!(count, before_count);
    assert_eq!(sum, before_sum);
}
