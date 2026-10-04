#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::quota::reset_epoch::{QuotaResetEpochInput, quota_reset_epoch_precedence};

#[test]
fn quota_reset_epoch_prefers_valid_candidates_in_declared_order() {
    let cases = [
        (
            QuotaResetEpochInput {
                resets_at: Some(-1),
                reset_at: Some(2),
                error_resets_at: Some(3),
                error_reset_at: Some(4),
                primary_reset_at: Some(5),
                secondary_reset_at: Some(6),
                ..QuotaResetEpochInput::default()
            },
            Some(-1),
        ),
        (
            QuotaResetEpochInput {
                reset_at: Some(2),
                error_resets_at: Some(3),
                error_reset_at: Some(4),
                ..QuotaResetEpochInput::default()
            },
            Some(2),
        ),
        (
            QuotaResetEpochInput {
                error_resets_at: Some(3),
                error_reset_at: Some(4),
                ..QuotaResetEpochInput::default()
            },
            Some(3),
        ),
        (
            QuotaResetEpochInput {
                error_reset_at: Some(4),
                ..QuotaResetEpochInput::default()
            },
            Some(4),
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(quota_reset_epoch_precedence(input), Ok(expected));
    }
}

#[test]
fn quota_reset_epoch_applies_used_percent_gates_and_missing_values() {
    let cases = [
        (
            QuotaResetEpochInput {
                primary_reset_at: Some(10),
                secondary_reset_at: Some(20),
                primary_used_percent: Some(100),
                secondary_used_percent: Some(100),
                ..QuotaResetEpochInput::default()
            },
            Some(10),
        ),
        (
            QuotaResetEpochInput {
                primary_reset_at: Some(10),
                secondary_reset_at: Some(20),
                primary_used_percent: Some(101),
                secondary_used_percent: Some(99),
                ..QuotaResetEpochInput::default()
            },
            Some(10),
        ),
        (
            QuotaResetEpochInput {
                primary_reset_at: Some(10),
                secondary_reset_at: Some(20),
                primary_used_percent: Some(99),
                secondary_used_percent: Some(100),
                ..QuotaResetEpochInput::default()
            },
            Some(20),
        ),
        (
            QuotaResetEpochInput {
                secondary_reset_at: Some(20),
                primary_used_percent: Some(100),
                ..QuotaResetEpochInput::default()
            },
            None,
        ),
        (
            QuotaResetEpochInput {
                primary_reset_at: Some(10),
                secondary_used_percent: Some(100),
                ..QuotaResetEpochInput::default()
            },
            None,
        ),
        (
            QuotaResetEpochInput {
                secondary_reset_at: Some(-20),
                primary_used_percent: Some(-1),
                secondary_used_percent: Some(99),
                ..QuotaResetEpochInput::default()
            },
            Some(-20),
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(quota_reset_epoch_precedence(input), Ok(expected));
    }
}
