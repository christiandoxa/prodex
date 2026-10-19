#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::info_render::{
    InfoStatusReset, InfoStatusRunway, InfoStatusRunwayEstimate, format_info_runway,
    format_relative_duration, format_status_runway,
};

fn input(burn_per_hour: f64) -> InfoStatusRunway<'static> {
    InfoStatusRunway {
        profiles_with_data: 3,
        current_remaining: 1,
        earliest_reset: None,
        estimate: Some(InfoStatusRunwayEstimate {
            burn_per_hour,
            observed_profiles: 2,
            observed_span_seconds: 59,
            exhaust_at: 59,
            exhaust_text: "soon 🧭",
        }),
        now: 0,
    }
}

#[test]
fn info_runway_retains_exact_fixed_decimal_for_ieee_values_and_status_compatibility() {
    for burn in [
        0.0,
        -0.0,
        1.25,
        1.15,
        1.35,
        2.25,
        -1.25,
        -1.15,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
        -f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::from_bits(0xfff8_0000_0000_0001),
    ] {
        assert_eq!(
            format_info_runway(input(burn)).unwrap(),
            format!(
                "soon 🧭 (~<1m) at {burn:.1} aggregated-%/h from 2 profile(s), observed over <1m, no-reset estimate"
            ),
            "bits={:x}",
            burn.to_bits()
        );
    }
    assert_eq!(
        format_status_runway(input(1.25)).unwrap(),
        "soon 🧭 (~<1m) at 1.3 aggregated-%/h from 2 profile(s), observed over <1m, no-reset estimate"
    );
}

#[test]
fn info_runway_matches_standard_fixed_decimal_across_all_ieee_exponents() {
    for exponent in 0_u64..2048 {
        for fraction in [0, 1, 0x000f_ffff_ffff_ffff] {
            for sign in [0, 1_u64 << 63] {
                let bits = sign | (exponent << 52) | fraction;
                let burn = f64::from_bits(bits);
                assert_eq!(
                    format_info_runway(input(burn)).unwrap(),
                    format!(
                        "soon 🧭 (~<1m) at {burn:.1} aggregated-%/h from 2 profile(s), observed over <1m, no-reset estimate"
                    ),
                    "bits={bits:x}"
                );
            }
        }
    }
}

#[test]
fn info_runway_preserves_short_circuits_empty_labels_reset_precedence_and_saturated_time() {
    let mut value = input(f64::NAN);
    value.profiles_with_data = 0;
    value.current_remaining = -1;
    assert_eq!(format_info_runway(value).unwrap(), "Unavailable");
    value.profiles_with_data = 1;
    assert_eq!(format_info_runway(value).unwrap(), "Exhausted");
    value.current_remaining = 1;
    value.estimate = None;
    assert_eq!(
        format_info_runway(value).unwrap(),
        "Unavailable (no recent quota decay observed in active runtime logs)"
    );
    value = input(1.25);
    value.now = i64::MIN;
    value.estimate.as_mut().unwrap().exhaust_at = i64::MAX;
    value.estimate.as_mut().unwrap().observed_profiles = usize::MAX;
    value.earliest_reset = Some(InfoStatusReset {
        timestamp: i64::MAX,
        text: "",
    });
    assert_eq!(
        format_info_runway(value).unwrap(),
        format!(
            "Earliest reset  arrives before the no-reset runway (~{} at 1.2 aggregated-%/h, {} profile(s), observed over <1m)",
            format_relative_duration(i64::MAX).unwrap(),
            usize::MAX
        )
    );
    value = input(1.25);
    value.estimate.as_mut().unwrap().exhaust_text = "";
    value.earliest_reset = Some(InfoStatusReset {
        timestamp: 60,
        text: "later",
    });
    assert_eq!(
        format_info_runway(value).unwrap(),
        " (~<1m) at 1.2 aggregated-%/h from 2 profile(s), observed over <1m, no-reset estimate"
    );
}

#[test]
fn info_runway_abi_rejects_wrong_counts_and_presence_without_writes() {
    unsafe extern "C" {
        fn prodex_terminal_info_render_v1(
            version: i64,
            operation: i64,
            signed: u64,
            signed_count: i64,
            unsigned: u64,
            unsigned_count: i64,
            texts: u64,
            text_count: i64,
            presence: u64,
            output: u64,
            capacity: i64,
            written: u64,
        ) -> i64;
    }
    let signed = [0_i64; 5];
    let unsigned = [0_u64; 3];
    let texts = [[b"".as_ptr() as u64, 0]; 2];
    let mut output = [77_u8; 64];
    let mut written = 71_i64;
    for (signed_count, unsigned_count, text_count, presence) in
        [(4, 3, 2, 0), (5, 2, 2, 0), (5, 3, 1, 0), (5, 3, 2, 4)]
    {
        assert_eq!(
            unsafe {
                prodex_terminal_info_render_v1(
                    1,
                    34,
                    signed.as_ptr() as u64,
                    signed_count,
                    unsigned.as_ptr() as u64,
                    unsigned_count,
                    texts.as_ptr() as u64,
                    text_count,
                    presence,
                    output.as_mut_ptr() as u64,
                    64,
                    (&mut written as *mut i64) as u64,
                )
            },
            1
        );
        assert_eq!(written, 71);
        assert_eq!(output, [77; 64]);
    }
    assert_eq!(
        unsafe {
            prodex_terminal_info_render_v1(
                1,
                34,
                signed.as_ptr() as u64,
                5,
                unsigned.as_ptr() as u64,
                3,
                texts.as_ptr() as u64,
                2,
                0,
                output.as_mut_ptr() as u64,
                64,
                (&mut written as *mut i64) as u64,
            )
        },
        0
    );
    assert_eq!(written, 11);
    assert_eq!(&output[..11], b"Unavailable");
}
