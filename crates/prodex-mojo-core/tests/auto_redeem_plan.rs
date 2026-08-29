#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::{
    MojoError,
    runtime::{AutoRedeemCandidateInput, auto_redeem_plan_batch},
};

fn plan_priority(plan_type: Option<&str>) -> i64 {
    let normalized = plan_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_'))
        .collect::<String>();
    match normalized.as_str() {
        "plus" => 0,
        "free" | "basic" => 1,
        "prolite" | "pro" | "pro5x" | "5x" | "pro20x" | "pro20" | "20x" | "ultra" | "max"
        | "team" | "business" | "enterprise" => 3,
        _ => 2,
    }
}

fn expected(inputs: &[AutoRedeemCandidateInput<'_>], now: i64) -> Option<usize> {
    inputs
        .iter()
        .enumerate()
        .filter(|(_, input)| {
            input.available_count > 0
                && input.weekly_status == 3
                && input.weekly_reset_at != i64::MAX
                && input.weekly_reset_at.saturating_sub(now) > 300
        })
        .min_by_key(|(index, input)| {
            (
                plan_priority(input.plan_type),
                input.weekly_reset_at.saturating_neg(),
                input.inflight_count,
                input.health_sort_key,
                input.order_index,
                *index,
            )
        })
        .map(|(index, _)| index)
}

#[test]
fn auto_redeem_plan_matches_rust_oracle_for_seeded_candidates() {
    let inputs = [
        AutoRedeemCandidateInput {
            plan_type: Some(" free "),
            available_count: 1,
            weekly_status: 3,
            weekly_reset_at: 10_000,
            inflight_count: 0,
            health_sort_key: 0,
            order_index: 0,
        },
        AutoRedeemCandidateInput {
            plan_type: Some("Pro-5x"),
            available_count: 1,
            weekly_status: 3,
            weekly_reset_at: 10_000,
            inflight_count: 0,
            health_sort_key: 0,
            order_index: 1,
        },
        AutoRedeemCandidateInput {
            plan_type: Some("PLUS"),
            available_count: 1,
            weekly_status: 3,
            weekly_reset_at: 10_000,
            inflight_count: 0,
            health_sort_key: 0,
            order_index: 2,
        },
        AutoRedeemCandidateInput {
            plan_type: Some("未知"),
            available_count: 1,
            weekly_status: 3,
            weekly_reset_at: 10_000,
            inflight_count: 0,
            health_sort_key: 0,
            order_index: 3,
        },
        AutoRedeemCandidateInput {
            plan_type: None,
            available_count: 1,
            weekly_status: 3,
            weekly_reset_at: 1_300,
            inflight_count: 0,
            health_sort_key: 0,
            order_index: 4,
        },
    ];
    let now = 1_000;
    assert_eq!(expected(&inputs, now), Some(2));
    assert_eq!(auto_redeem_plan_batch(&inputs, now), Ok(Some(2)));
}

#[test]
fn auto_redeem_plan_requires_exhaustion_and_a_late_natural_reset() {
    let now = 10_000;
    let inputs = [
        AutoRedeemCandidateInput {
            weekly_status: 2,
            weekly_reset_at: now + 86_400,
            ..valid_candidate(0)
        },
        AutoRedeemCandidateInput {
            weekly_reset_at: now + 300,
            ..valid_candidate(1)
        },
        AutoRedeemCandidateInput {
            weekly_reset_at: now + 301,
            ..valid_candidate(2)
        },
        AutoRedeemCandidateInput {
            available_count: 0,
            ..valid_candidate(3)
        },
    ];

    assert_eq!(expected(&inputs, now), Some(2));
    assert_eq!(auto_redeem_plan_batch(&inputs, now), Ok(Some(2)));
}

fn valid_candidate(order_index: i64) -> AutoRedeemCandidateInput<'static> {
    AutoRedeemCandidateInput {
        plan_type: Some("plus"),
        available_count: 1,
        weekly_status: 3,
        weekly_reset_at: 86_400,
        inflight_count: 0,
        health_sort_key: 0,
        order_index,
    }
}

#[test]
fn auto_redeem_plan_matches_rust_trim_for_control_separators() {
    let inputs = [
        AutoRedeemCandidateInput {
            plan_type: Some("\u{1c}plus"),
            ..valid_candidate(0)
        },
        AutoRedeemCandidateInput {
            plan_type: Some("plus\u{1f}"),
            ..valid_candidate(1)
        },
        valid_candidate(2),
    ];

    assert_eq!(expected(&inputs, 0), Some(2));
    assert_eq!(auto_redeem_plan_batch(&inputs, 0), Ok(Some(2)));
}

#[test]
fn auto_redeem_plan_matches_rust_oracle_for_20000_generated_scenarios() {
    const PLANS: [Option<&str>; 10] = [
        None,
        Some("PLUS"),
        Some(" free "),
        Some("basic"),
        Some("Pro-5x"),
        Some("pro_20_x"),
        Some("team"),
        Some("unknown"),
        Some("未知"),
        Some("Pro\t5x"),
    ];
    let mut seed = 0x9e37_79b9_u64;
    let next = |seed: &mut u64| {
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        *seed
    };

    for case in 0..20_000 {
        let count = (next(&mut seed) % 8 + 1) as usize;
        let now = match next(&mut seed) % 5 {
            0 => 1_000,
            1 => i64::MIN,
            2 => i64::MAX,
            _ => (next(&mut seed) % 100_000) as i64,
        };
        let mut inputs = Vec::with_capacity(count);
        for index in 0..count {
            let reset_at = match next(&mut seed) % 7 {
                0 => i64::MAX,
                1 => now,
                2 => now.saturating_add(300),
                3 => now.saturating_add(301),
                4 => i64::MIN,
                _ => (next(&mut seed) % 100_000) as i64,
            };
            inputs.push(AutoRedeemCandidateInput {
                plan_type: PLANS[(next(&mut seed) % PLANS.len() as u64) as usize],
                available_count: (next(&mut seed) % 5) as i64 - 1,
                weekly_status: (next(&mut seed) % 5) as i64,
                weekly_reset_at: reset_at,
                inflight_count: (next(&mut seed) % 8) as i64,
                health_sort_key: (next(&mut seed) % 8) as i64,
                order_index: index as i64,
            });
        }
        assert_eq!(
            auto_redeem_plan_batch(&inputs, now),
            Ok(expected(&inputs, now)),
            "generated auto-redeem scenario {case}"
        );
    }
}

#[test]
fn auto_redeem_plan_rejects_invalid_inputs() {
    let valid = AutoRedeemCandidateInput {
        plan_type: Some("plus"),
        available_count: 1,
        weekly_status: 3,
        weekly_reset_at: 1_000,
        inflight_count: 0,
        health_sort_key: 0,
        order_index: 0,
    };
    assert_eq!(
        auto_redeem_plan_batch(
            &[AutoRedeemCandidateInput {
                weekly_status: 5,
                ..valid
            }],
            0
        ),
        Err(MojoError::InvalidInput)
    );
    for invalid in [
        AutoRedeemCandidateInput {
            inflight_count: -1,
            ..valid
        },
        AutoRedeemCandidateInput {
            health_sort_key: -1,
            ..valid
        },
        AutoRedeemCandidateInput {
            order_index: -1,
            ..valid
        },
    ] {
        assert_eq!(
            auto_redeem_plan_batch(&[invalid], 0),
            Err(MojoError::InvalidInput)
        );
    }

    let long_plan = "x".repeat(4_097);
    assert_eq!(
        auto_redeem_plan_batch(
            &[AutoRedeemCandidateInput {
                plan_type: Some(long_plan.as_str()),
                ..valid
            }],
            0,
        ),
        Err(MojoError::InvalidInput)
    );

    let oversized = vec![valid; 257];
    assert_eq!(
        auto_redeem_plan_batch(&oversized, 0),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn auto_redeem_plan_uses_exact_natural_reset_boundary() {
    let mut candidate = AutoRedeemCandidateInput {
        plan_type: Some("plus"),
        available_count: 1,
        weekly_status: 3,
        weekly_reset_at: 1_300,
        inflight_count: 0,
        health_sort_key: 0,
        order_index: 0,
    };

    assert_eq!(auto_redeem_plan_batch(&[candidate], 1_000), Ok(None));

    candidate.weekly_reset_at = 1_301;
    assert_eq!(auto_redeem_plan_batch(&[candidate], 1_000), Ok(Some(0)));

    candidate.available_count = 0;
    assert_eq!(auto_redeem_plan_batch(&[candidate], 1_000), Ok(None));
}
