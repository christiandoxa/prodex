#![allow(unsafe_code)]

//! Safe, stateless wrappers around the narrow Rust-to-Mojo C ABI.
//!
//! This crate is the only owner of unsafe FFI declarations. Callers exchange
//! bounded integers and caller-owned slices; no Rust heap object or Mojo
//! collection crosses the ABI.

pub const MOJO_ACTIVE: bool = cfg!(prodex_mojo_active);
pub const MOJO_REQUIRED: bool = cfg!(prodex_mojo_required);
pub const MOJO_VERSION: Option<&str> = option_env!("PRODEX_MOJO_VERSION");

#[cfg(feature = "mojo-core")]
pub fn self_test() -> bool {
    let quota = quota::remaining_percent(Some(42)) == 58
        && quota::window_status(5, true) == 2
        && quota::pressure_band(1, 2) == 2
        && quota::window_pair_has_ready_limit(Some(20), Some(30));
    let routing = routing::routing_plan_batch(
        &[routing::RoutingPlanInput {
            hard_eligible: true,
            capability_mask: 1,
            provider_order: 0,
            score: routing::ScoreInput {
                health: 10_000,
                load: 0,
                quota_headroom: 10_000,
                quota_present: true,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 10_000,
                affinity: true,
            },
        }],
        1,
        routing::ScoreWeights {
            health: 10_000,
            load: 0,
            cost: 0,
            latency: 0,
            risk: 0,
            priority: 0,
            affinity: 0,
        },
    );
    let capability = routing::capability_match_batch(&[true, true], &[1, 0], 1);
    let profile = runtime::profile_scores_batch(&[runtime::ProfileScoreInput {
        weekly_pressure: 1_000,
        five_hour_pressure: 2_000,
        scale_bps: 10_000,
        weekly_remaining: 90,
        five_hour_remaining: 80,
        reserve_bias: 0,
        weekly_weight: 1,
    }]);
    let routing_ok = routing.is_some_and(|plan| {
        plan.eligible == [true]
            && plan.reason_tags == [routing::ROUTING_REASON_ELIGIBLE]
            && plan.ordered_indices == [0]
            && plan.scores[0].score == 10_000
    });
    let capability_ok = capability.is_some_and(|result| {
        result.first_compatible == Some(0)
            && result.first_incompatible == Some(1)
            && result.compatible == [true, false]
    });
    let profile_ok = profile.first().is_some_and(|score| {
        score.total_pressure == 3_000
            && score.weekly_pressure == 1_000
            && score.five_hour_pressure == 2_000
            && score.reserve_floor == 80
    });
    let candidate_plan_ok = runtime::candidate_plan_self_test();
    let pressure_snapshot_ok = runtime::smart_context_pressure_snapshot_self_test();
    let rehydrate_plan_ok = runtime_decisions::rehydrate_plan_self_test();
    let quota_aggregation_ok = quota::main_quota_aggregation_self_test();
    let provider_constraints_ok = provider_constraints::self_test();
    let tuning_defaults_ok = runtime_decisions::tuning_defaults_self_test();
    let checks = [
        routing_ok,
        capability_ok,
        profile_ok,
        candidate_plan_ok,
        pressure_snapshot_ok,
        rehydrate_plan_ok,
        quota_aggregation_ok,
        provider_constraints_ok,
        tuning_defaults_ok,
        routing::abi_version() == Some(routing::ABI_VERSION),
        quota,
        runtime::pressure_band_for_route(Some((4, 1)), None, 0) == 2,
        runtime::smart_context_estimate_tokens_from_body_bytes(7) == 2,
    ];
    #[cfg(test)]
    if checks.iter().any(|check| !check) {
        eprintln!("Mojo self-test checks: {checks:?}");
    }
    checks.into_iter().all(|check| check)
}

#[cfg(all(test, feature = "mojo-core"))]
#[test]
fn compiled_core_self_test_passes() {
    assert!(self_test());
}

#[cfg(feature = "mojo-provider-constraints")]
pub mod provider_constraints;
#[cfg(feature = "mojo-quota")]
pub mod quota;
#[cfg(feature = "mojo-routing")]
pub mod routing;
#[cfg(feature = "mojo-runtime")]
pub mod runtime;
#[cfg(feature = "mojo-runtime")]
pub mod runtime_decisions;
