use super::*;

#[test]
fn rollout_decision_preserves_explicit_exact_mode() {
    let decision = smart_context_rollout_decision(SmartContextRolloutDecisionInput {
        enabled: true,
        explicit_exact_mode: true,
        shadow_mode: true,
        canary_percent: 100,
        stable_key: "request-a".to_string(),
    });

    assert_eq!(decision.mode, SmartContextRolloutMode::Disabled);
    assert_eq!(decision.reason, "explicit_exact");
    assert!(!decision.applies_rewrite());
    assert!(!decision.computes_shadow());
}

#[test]
fn rollout_decision_shadow_computes_without_applying() {
    let decision = smart_context_rollout_decision(SmartContextRolloutDecisionInput {
        enabled: true,
        explicit_exact_mode: false,
        shadow_mode: true,
        canary_percent: 100,
        stable_key: "request-b".to_string(),
    });

    assert_eq!(decision.mode, SmartContextRolloutMode::Shadow);
    assert_eq!(decision.reason, "shadow");
    assert!(!decision.applies_rewrite());
    assert!(decision.computes_shadow());
}

#[test]
fn rollout_decision_canary_is_stable_and_bounds_percent() {
    let key = "profile:alpha:request:42";
    let bucket = smart_context_rollout_bucket(key);
    assert_eq!(bucket, smart_context_rollout_bucket(key));
    assert!(bucket < 10_000);

    let disabled = smart_context_rollout_decision(SmartContextRolloutDecisionInput {
        enabled: true,
        explicit_exact_mode: false,
        shadow_mode: false,
        canary_percent: 0,
        stable_key: key.to_string(),
    });
    assert_eq!(disabled.mode, SmartContextRolloutMode::Disabled);
    assert_eq!(disabled.reason, "canary_out");

    let enabled = smart_context_rollout_decision(SmartContextRolloutDecisionInput {
        canary_percent: 101,
        ..SmartContextRolloutDecisionInput {
            enabled: true,
            explicit_exact_mode: false,
            shadow_mode: false,
            canary_percent: 100,
            stable_key: key.to_string(),
        }
    });
    assert_eq!(enabled.mode, SmartContextRolloutMode::Apply);
    assert_eq!(enabled.canary_percent, 100);
}

#[test]
fn rollout_bucket_uses_the_full_basis_point_range() {
    let buckets = (0..1_000)
        .map(|index| smart_context_rollout_bucket(&format!("scope-{index}")))
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        buckets.len() > 900,
        "only {} distinct buckets",
        buckets.len()
    );
    assert!(buckets.iter().any(|bucket| *bucket > 0));
    assert!(buckets.iter().any(|bucket| *bucket >= 9_000));
}

#[test]
fn rollout_bucket_distribution_is_uniform_enough_for_canary_assignment() {
    let included = (0..20_000)
        .filter(|index| smart_context_rollout_bucket(&format!("scope-{index}")) < 1_000)
        .count();

    assert!(
        (1_800..=2_200).contains(&included),
        "10% canary selected {included}/20000 scopes"
    );
}
