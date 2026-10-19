use super::*;

#[test]
fn info_runway_display_routes_exact_decimal_and_reset_policy_through_mojo() {
    let estimate = Some(InfoRunwayEstimateDisplay {
        burn_per_hour: 1.25,
        observed_profiles: 2,
        observed_span_seconds: 59,
        exhaust_at: 59,
        exhaust_text: "soon 🧭",
    });
    assert_eq!(
        format_info_runway_display(3, 1, None, estimate, 0),
        "soon 🧭 (~<1m) at 1.2 aggregated-%/h from 2 profile(s), observed over <1m, no-reset estimate"
    );
    assert_eq!(
        format_info_runway_display(
            3,
            1,
            Some(InfoRunwayResetDisplay {
                reset_at: 59,
                reset_text: "now 🧭"
            }),
            estimate,
            0
        ),
        "Earliest reset now 🧭 arrives before the no-reset runway (~<1m at 1.2 aggregated-%/h, 2 profile(s), observed over <1m)"
    );
    assert_eq!(
        format_info_runway_display(0, 0, None, estimate, 0),
        "Unavailable"
    );
    assert_eq!(
        format_info_runway_display(3, 0, None, estimate, 0),
        "Exhausted"
    );
}

#[test]
fn info_runway_display_preserves_large_source_labels_without_a_new_text_cap() {
    let label = "x".repeat(65_537) + "🧭\0";
    let estimate = Some(InfoRunwayEstimateDisplay {
        burn_per_hour: -0.0,
        observed_profiles: 1,
        observed_span_seconds: 0,
        exhaust_at: 0,
        exhaust_text: &label,
    });
    assert_eq!(
        format_info_runway_display(1, 1, None, estimate, 0),
        format!(
            "{label} (~now) at -0.0 aggregated-%/h from 1 profile(s), observed over now, no-reset estimate"
        )
    );
}
