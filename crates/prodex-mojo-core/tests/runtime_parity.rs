#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    RUNTIME_CANDIDATE_PLAN_FIELD_COUNT, runtime_candidate_plan_batch,
    smart_context_pressure_snapshot,
};
use std::cmp::Ordering;

fn field(fields: &[i64], index: usize, offset: usize) -> i64 {
    fields[index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT + offset]
}

fn ready_cmp(fields: &[i64], left: usize, right: usize, route_kind: i64) -> Ordering {
    let left_source = if route_kind == 0 || route_kind == 2 {
        field(fields, left, 11)
    } else {
        0
    };
    let right_source = if route_kind == 0 || route_kind == 2 {
        field(fields, right, 11)
    } else {
        0
    };
    let ascending = [
        (field(fields, left, 1), field(fields, right, 1)),
        (field(fields, left, 2), field(fields, right, 2)),
        (field(fields, left, 3), field(fields, right, 3)),
        (field(fields, left, 4), field(fields, right, 4)),
        (field(fields, left, 8), field(fields, right, 8)),
        (field(fields, left, 9), field(fields, right, 9)),
        (left_source, right_source),
        (field(fields, left, 12), field(fields, right, 12)),
        (field(fields, left, 13), field(fields, right, 13)),
        (field(fields, left, 14), field(fields, right, 14)),
        (field(fields, left, 15), field(fields, right, 15)),
        (field(fields, left, 16), field(fields, right, 16)),
        (field(fields, left, 17), field(fields, right, 17)),
    ];
    for (left_value, right_value) in ascending {
        let ordering = left_value.cmp(&right_value);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    for offset in [5, 6, 7] {
        let ordering = field(fields, right, offset).cmp(&field(fields, left, offset));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.cmp(&right)
}

fn fallback_cmp(fields: &[i64], left: usize, right: usize, route_kind: i64) -> Ordering {
    for offset in 18..=21 {
        let ordering = field(fields, left, offset).cmp(&field(fields, right, offset));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    ready_cmp(fields, left, right, route_kind)
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

#[derive(Clone, Copy)]
struct PressureCase {
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_tokens: u64,
    source: i64,
    unknown_window: bool,
    zero_context_window: bool,
    reserved_output_consumes_window: bool,
}

fn generated_pressure_case(state: &mut u64, case: usize) -> PressureCase {
    PressureCase {
        model_context_window_tokens: if case.is_multiple_of(5) {
            None
        } else {
            Some(next_random(state) % 200_000)
        },
        reserved_output_tokens: if case.is_multiple_of(7) {
            u64::MAX
        } else {
            next_random(state) % 200_000
        },
        effective_input_tokens: if case.is_multiple_of(11) {
            u64::MAX
        } else {
            next_random(state) % 400_000
        },
        source: (case % 4) as i64,
        unknown_window: case.is_multiple_of(6),
        zero_context_window: case.is_multiple_of(9),
        reserved_output_consumes_window: case.is_multiple_of(8),
    }
}

fn expected_pressure_snapshot(
    input: PressureCase,
) -> prodex_mojo_core::runtime::SmartContextPressureSnapshot {
    let usable = input
        .model_context_window_tokens
        .and_then(|window| window.checked_sub(input.reserved_output_tokens));
    let pressure_basis_points = usable.and_then(|usable| {
        (usable > 0).then(|| {
            input
                .effective_input_tokens
                .saturating_mul(10_000)
                .checked_div(usable)
                .unwrap_or(u64::MAX)
                .min(u64::from(u32::MAX)) as u32
        })
    });
    let pressure_band = match pressure_basis_points {
        None => 0,
        Some(value) if value >= 10_000 => 5,
        Some(value) if value >= 9_000 => 4,
        Some(value) if value >= 7_500 => 3,
        Some(value) if value >= 5_000 => 2,
        Some(_) => 1,
    };
    let estimator_confidence = if input.unknown_window
        || input.zero_context_window
        || input.reserved_output_consumes_window
    {
        2
    } else {
        match input.source {
            0 | 2 => 0,
            1 => 1,
            _ => 2,
        }
    };
    let usable_for_floor = input
        .model_context_window_tokens
        .map(|window| window.saturating_sub(input.reserved_output_tokens));
    prodex_mojo_core::runtime::SmartContextPressureSnapshot {
        effective_usable_context_tokens: usable,
        effective_used_tokens: input.effective_input_tokens,
        pressure_basis_points,
        pressure_band,
        absolute_safety_floor_tokens: usable_for_floor
            .map(|value| (value / 20).clamp(1_000, 8_000))
            .unwrap_or(2_000),
        estimator_confidence,
    }
}

#[test]
fn candidate_plan_matches_rust_oracle_for_generated_batches() {
    let mut state = 0x6d6f6a6f5f706c61_u64;
    for case in 0..300 {
        let count = (next_random(&mut state) % 25) as usize;
        let route_kind = (case % 4) as i64;
        let mut fields = vec![0_i64; count * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT];
        for index in 0..count {
            let base = index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT;
            fields[base] = (next_random(&mut state) % 2) as i64;
            fields[base + 1] = (next_random(&mut state) % 8) as i64;
            fields[base + 2] = (next_random(&mut state) % 5) as i64;
            for offset in 3..=10 {
                fields[base + offset] = (next_random(&mut state) % 401) as i64 - 200;
            }
            fields[base + 11] = (next_random(&mut state) % 2) as i64;
            fields[base + 12] = (next_random(&mut state) % 32) as i64;
            fields[base + 13] = (next_random(&mut state) % 32) as i64;
            fields[base + 14] = (next_random(&mut state) % 2) as i64;
            fields[base + 15] = next_random(&mut state) as i64;
            fields[base + 16] = index as i64;
            fields[base + 17] = next_random(&mut state) as i64;
            fields[base + 18] = (next_random(&mut state) % 8) as i64;
            for offset in 19..=21 {
                fields[base + offset] = (next_random(&mut state) % 401) as i64 - 200;
            }
        }

        let mut expected_ready = (0..count)
            .filter(|index| field(&fields, *index, 0) == 0)
            .collect::<Vec<_>>();
        expected_ready.sort_by(|left, right| ready_cmp(&fields, *left, *right, route_kind));
        let mut expected_fallback = (0..count).collect::<Vec<_>>();
        expected_fallback.sort_by(|left, right| fallback_cmp(&fields, *left, *right, route_kind));

        let actual = runtime_candidate_plan_batch(&fields, route_kind)
            .expect("strict Mojo candidate plan should accept generated input");
        assert_eq!(
            actual.ready_indices, expected_ready,
            "candidate case {case}"
        );
        assert_eq!(
            actual.fallback_indices, expected_fallback,
            "candidate case {case}"
        );
    }
}

#[test]
fn pressure_snapshot_matches_rust_oracle_for_generated_inputs() {
    let mut state = 0x7072657373757265_u64;
    for case in 0..300 {
        let input = generated_pressure_case(&mut state, case);
        let expected = expected_pressure_snapshot(input);
        let actual = smart_context_pressure_snapshot(
            input.model_context_window_tokens,
            input.reserved_output_tokens,
            input.effective_input_tokens,
            input.source,
            input.unknown_window,
            input.zero_context_window,
            input.reserved_output_consumes_window,
        )
        .expect("strict Mojo pressure snapshot should accept generated input");
        assert_eq!(actual, expected, "pressure case {case}");
    }
}
