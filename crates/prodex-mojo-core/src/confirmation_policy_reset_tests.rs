use super::*;

#[test]
fn compiled_manual_reset_selection_preserves_absence_thresholds_and_first_ties() {
    for (primary, weekly, expected) in [
        (None, None, None),
        (Some(4601), Some(604800), None),
        (
            Some(4600),
            Some(604800),
            Some(ManualRedeemResetWindow::Primary { reset_at: 4600 }),
        ),
        (
            Some(1060),
            Some(1030),
            Some(ManualRedeemResetWindow::Weekly { reset_at: 1030 }),
        ),
        (
            Some(1030),
            Some(1030),
            Some(ManualRedeemResetWindow::Primary { reset_at: 1030 }),
        ),
        (
            None,
            Some(0),
            Some(ManualRedeemResetWindow::Weekly { reset_at: 0 }),
        ),
    ] {
        assert_eq!(
            manual_redeem_reset_window(ManualRedeemResetInput {
                primary_reset_at: primary,
                weekly_reset_at: weekly,
                now: 1000,
                near_reset_seconds: 3600,
            }),
            Ok(expected)
        );
    }
}

#[test]
fn compiled_manual_reset_selection_preserves_full_signed_saturation_and_past_resets() {
    for (primary, now, near, expected) in [
        (
            800,
            1000,
            -100,
            Some(ManualRedeemResetWindow::Primary { reset_at: 800 }),
        ),
        (
            i64::MAX,
            i64::MIN,
            i64::MAX,
            Some(ManualRedeemResetWindow::Primary { reset_at: i64::MAX }),
        ),
        (i64::MAX, i64::MIN, i64::MAX - 1, None),
        (
            i64::MIN,
            i64::MAX,
            i64::MIN,
            Some(ManualRedeemResetWindow::Primary { reset_at: i64::MIN }),
        ),
    ] {
        assert_eq!(
            manual_redeem_reset_window(ManualRedeemResetInput {
                primary_reset_at: Some(primary),
                weekly_reset_at: None,
                now,
                near_reset_seconds: near,
            }),
            Ok(expected)
        );
    }
}

#[test]
fn compiled_manual_reset_abi_rejects_versions_tags_and_noncanonical_absence() {
    for (version, primary_present, primary, weekly_present, weekly, expected) in [
        (2, 0, 0, 0, 0, -2),
        (1, 2, 0, 0, 0, -1),
        (1, 0, 1, 0, 0, -1),
        (1, 0, 0, -1, 0, -1),
        (1, 0, 0, 0, 1, -1),
    ] {
        assert_eq!(
            unsafe {
                prodex_manual_redeem_reset_v1(
                    version,
                    primary_present,
                    primary,
                    weekly_present,
                    weekly,
                    0,
                    0,
                )
            },
            expected
        );
    }
}
