#![cfg(feature = "mojo-quota")]

use prodex_mojo_core::quota::{
    QUOTA_BLOCKED_WINDOW_KIND_ADDITIONAL, QUOTA_BLOCKED_WINDOW_KIND_CODE_REVIEW,
    QUOTA_BLOCKED_WINDOW_KIND_MAIN, QuotaBlockedWindowInput, quota_blocked_window_plan,
};

#[test]
fn required_mojo_plans_main_additional_and_code_review_blocked_rows() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };
    let inputs = [
        QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_MAIN,
            used_percent: None,
        },
        QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_MAIN,
            used_percent: Some(100),
        },
        QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_MAIN,
            used_percent: Some(99),
        },
        QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_ADDITIONAL,
            used_percent: Some(100),
        },
        QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_ADDITIONAL,
            used_percent: None,
        },
        QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_CODE_REVIEW,
            used_percent: Some(100),
        },
    ];
    assert_eq!(
        quota_blocked_window_plan(false, true, &inputs).unwrap(),
        [true, true, false, true, false, true]
    );
    assert_eq!(
        quota_blocked_window_plan(true, true, &inputs).unwrap(),
        [false, false, false, false, false, true]
    );
    assert_eq!(
        quota_blocked_window_plan(false, false, &inputs),
        Ok(vec![true, true, false, true, false, false])
    );
}

#[test]
fn required_mojo_blocked_window_plan_batches_without_reordering() {
    let inputs = (0..2_049)
        .map(|index| QuotaBlockedWindowInput {
            kind: QUOTA_BLOCKED_WINDOW_KIND_ADDITIONAL,
            used_percent: Some(if index % 2 == 0 { 100 } else { 0 }),
        })
        .collect::<Vec<_>>();
    let planned = quota_blocked_window_plan(false, false, &inputs).unwrap();
    assert_eq!(planned.len(), inputs.len());
    assert!(planned.iter().step_by(2).all(|blocked| *blocked));
    assert!(planned.iter().skip(1).step_by(2).all(|blocked| !blocked));
}

#[test]
fn required_mojo_rejects_invalid_blocked_window_kind() {
    assert!(
        quota_blocked_window_plan(
            false,
            false,
            &[QuotaBlockedWindowInput {
                kind: 99,
                used_percent: Some(100),
            }],
        )
        .is_err()
    );
}
