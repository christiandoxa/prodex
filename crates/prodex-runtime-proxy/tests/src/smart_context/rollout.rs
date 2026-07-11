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
    assert!(bucket < 100);

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
