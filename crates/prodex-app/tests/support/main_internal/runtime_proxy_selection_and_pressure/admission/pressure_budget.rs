use super::*;

#[test]
fn runtime_proxy_pressure_mode_shrinks_precommit_budget() {
    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS + 5,
        ))
        .expect("checked_sub should succeed");

    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, true
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, false
    ));
}

#[test]
fn runtime_proxy_pressure_mode_preserves_continuation_budget() {
    let pressure_budget_elapsed = Instant::now()
        .checked_sub(Duration::from_millis(
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS + 5,
        ))
        .expect("checked_sub should succeed");

    assert!(
        !runtime_proxy_precommit_budget_exhausted(pressure_budget_elapsed, 0, true, true),
        "continuations should keep extended budget under pressure"
    );
    assert!(
        !runtime_proxy_precommit_budget_exhausted(
            Instant::now(),
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            true,
            true,
        ),
        "continuations should keep extended attempt limit under pressure"
    );
    assert!(runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
        true,
        true,
    ));
}
