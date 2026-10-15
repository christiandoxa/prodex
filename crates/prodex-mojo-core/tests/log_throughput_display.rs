#![cfg(all(feature = "mojo-runtime", prodex_mojo_required))]

use prodex_mojo_core::log_throughput_policy::{ThroughputRateDisplayPlan, rate_display_plan};

#[test]
fn required_mojo_rate_display_plan_preserves_validation_and_precision() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    for rate in [
        None,
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(0.0),
        Some(-1.0),
    ] {
        assert_eq!(rate_display_plan(rate).unwrap(), None);
    }
    assert_eq!(
        rate_display_plan(Some(0.74)).unwrap(),
        Some(ThroughputRateDisplayPlan {
            display_rate: 0.7,
            decimal_places: 1,
        })
    );
    assert_eq!(
        rate_display_plan(Some(0.75)).unwrap(),
        Some(ThroughputRateDisplayPlan {
            display_rate: 0.8,
            decimal_places: 1,
        })
    );
    assert_eq!(
        rate_display_plan(Some(0.01)).unwrap(),
        Some(ThroughputRateDisplayPlan {
            display_rate: 0.0,
            decimal_places: 0,
        })
    );
    assert_eq!(
        rate_display_plan(Some(10.04)).unwrap(),
        Some(ThroughputRateDisplayPlan {
            display_rate: 10.0,
            decimal_places: 0,
        })
    );
    assert_eq!(
        rate_display_plan(Some(99.95)).unwrap(),
        Some(ThroughputRateDisplayPlan {
            display_rate: 100.0,
            decimal_places: 0,
        })
    );
    assert_eq!(
        rate_display_plan(Some(100.6)).unwrap(),
        Some(ThroughputRateDisplayPlan {
            display_rate: 100.6,
            decimal_places: 0,
        })
    );
}
