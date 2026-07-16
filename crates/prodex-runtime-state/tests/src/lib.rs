use crate::{
    RuntimeBackgroundQueueEnqueuePlan, RuntimeBackgroundQueueKind,
    RuntimeBackgroundQueuePressureThresholds, RuntimeDueJobs, RuntimeProbeUsageSnapshotApplyInput,
    RuntimeProfileUsageSnapshot, RuntimeProxyAdmissionLimit, RuntimeProxyLaneAdmission,
    RuntimeProxyLaneLimits, RuntimeQuotaWindowStatus, RuntimeRouteKind, RuntimeScheduledSaveJob,
    RuntimeStartupProbeRefreshCandidate, RuntimeStartupProbeRefreshInput,
    RuntimeStartupProbeRefreshPlan, RuntimeStateLockWaitMetricCounters,
    RuntimeStateLockWaitMetrics, RuntimeStateMutation, RuntimeStateSaveSections,
    RuntimeStateSaveStateSection, RuntimeWaitDurationMetrics, runtime_background_enqueue_backlog,
    runtime_background_queue_enqueue_plan, runtime_continuation_journal_save_debounce,
    runtime_continuation_journal_save_enqueue_plan, runtime_probe_usage_snapshot_apply_plan,
    runtime_profile_usage_snapshot_is_usable, runtime_profile_usage_snapshot_should_persist,
    runtime_profiles_needing_startup_probe_refresh,
    runtime_profiles_needing_startup_probe_refresh_from_snapshots,
    runtime_proxy_queue_pressure_active, runtime_state_save_debounce,
    runtime_state_save_enqueue_plan, runtime_state_save_sections, runtime_take_due_scheduled_jobs,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct TestJob {
    ready_at: Instant,
}

impl RuntimeScheduledSaveJob for TestJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

#[test]
fn due_jobs_are_removed_without_disturbing_future_jobs() {
    let now = Instant::now();
    let mut pending = BTreeMap::from([
        ("due-a", TestJob { ready_at: now }),
        (
            "future",
            TestJob {
                ready_at: now + Duration::from_secs(5),
            },
        ),
        (
            "due-b",
            TestJob {
                ready_at: now - Duration::from_secs(1),
            },
        ),
    ]);

    match runtime_take_due_scheduled_jobs(&mut pending, now) {
        RuntimeDueJobs::Due(due) => {
            assert_eq!(due.len(), 2);
            assert!(due.contains_key("due-a"));
            assert!(due.contains_key("due-b"));
        }
        RuntimeDueJobs::Wait(_) => panic!("expected due jobs"),
    }

    assert_eq!(pending.len(), 1);
    assert!(pending.contains_key("future"));
}

#[test]
fn future_jobs_return_wait_duration() {
    let now = Instant::now();
    let mut pending = BTreeMap::from([(
        "future",
        TestJob {
            ready_at: now + Duration::from_secs(3),
        },
    )]);

    match runtime_take_due_scheduled_jobs(&mut pending, now) {
        RuntimeDueJobs::Due(_) => panic!("expected wait"),
        RuntimeDueJobs::Wait(wait) => assert_eq!(wait, Duration::from_secs(3)),
    }

    assert_eq!(pending.len(), 1);
}

#[test]
fn save_sections_follow_typed_mutation_scope() {
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::UsageSnapshot("main".into())),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::None,
            continuations: false,
            profile_scores: false,
            usage_snapshots: true,
            backoffs: true,
        }
    );
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::ResponseIds("main".into())),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: true,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: false,
        }
    );
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::ProfileCommit("second".into())),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: false,
            profile_scores: true,
            usage_snapshots: false,
            backoffs: true,
        }
    );
    assert_eq!(
        runtime_state_save_sections(&RuntimeStateMutation::StartupAudit),
        RuntimeStateSaveSections::full()
    );
}

#[test]
fn debounce_only_applies_to_hot_continuation_reasons() {
    let debounce = Duration::from_millis(150);

    assert_eq!(
        runtime_state_save_debounce(&RuntimeStateMutation::TurnState("abc".into()), debounce),
        debounce
    );
    assert_eq!(
        runtime_continuation_journal_save_debounce(
            &RuntimeStateMutation::ProfileCommit("main".into()),
            debounce,
        ),
        Duration::ZERO
    );
}

#[test]
fn schedule_plan_combines_sections_debounce_and_journal_need() {
    let queued_at = Instant::now();
    let plan = runtime_state_save_enqueue_plan(
        &RuntimeStateMutation::TurnState("abc".into()),
        queued_at,
        Duration::from_millis(150),
    );

    assert_eq!(
        plan.schedule.sections,
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: true,
            profile_scores: false,
            usage_snapshots: false,
            backoffs: false,
        }
    );
    assert_eq!(plan.schedule.debounce, Duration::from_millis(150));
    assert!(plan.schedule.requires_continuation_journal);
    assert_eq!(plan.queue.queued_at, queued_at);
    assert_eq!(plan.queue.ready_in_ms(), 150);

    let journal_plan = runtime_continuation_journal_save_enqueue_plan(
        &RuntimeStateMutation::ProfileCommit("main".into()),
        queued_at,
        Duration::from_millis(150),
    );
    assert_eq!(journal_plan.ready_in(), Duration::ZERO);

    let release = runtime_state_save_enqueue_plan(
        &RuntimeStateMutation::SessionAffinityRelease("goal_resume".into()),
        queued_at,
        Duration::from_millis(150),
    );
    assert!(release.schedule.sections.continuations);
    assert!(release.schedule.requires_continuation_journal);
    assert_eq!(release.schedule.debounce, Duration::ZERO);
}

#[test]
fn pressure_helper_checks_each_queue_threshold() {
    let thresholds = RuntimeBackgroundQueuePressureThresholds {
        state_save: 8,
        continuation_journal: 8,
        probe_refresh: 16,
    };

    assert!(runtime_proxy_queue_pressure_active(8, 0, 0, thresholds));
    assert!(runtime_proxy_queue_pressure_active(0, 8, 0, thresholds));
    assert!(runtime_proxy_queue_pressure_active(0, 0, 16, thresholds));
    assert!(!runtime_proxy_queue_pressure_active(7, 7, 15, thresholds));
    assert_eq!(runtime_background_enqueue_backlog(0), 0);
    assert_eq!(runtime_background_enqueue_backlog(1), 0);
    assert_eq!(runtime_background_enqueue_backlog(3), 2);
    assert_eq!(
        runtime_background_queue_enqueue_plan(
            RuntimeBackgroundQueueKind::ProbeRefresh,
            18,
            thresholds
        ),
        RuntimeBackgroundQueueEnqueuePlan {
            backlog: 17,
            pressure_active: true,
        }
    );
}

#[test]
fn startup_probe_refresh_filters_stale_profiles_to_warm_limit() {
    let exhausted_hold = RuntimeProfileUsageSnapshot {
        checked_at: 0,
        five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
        five_hour_remaining_percent: 0,
        five_hour_reset_at: 300,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 90,
        weekly_reset_at: 0,
    };
    let expired_hold = RuntimeProfileUsageSnapshot {
        five_hour_reset_at: 100,
        ..exhausted_hold.clone()
    };
    assert!(runtime_profile_usage_snapshot_is_usable(
        &exhausted_hold,
        200,
        60,
        |status| status == RuntimeQuotaWindowStatus::Exhausted,
    ));
    assert!(!runtime_profile_usage_snapshot_is_usable(
        &expired_hold,
        200,
        60,
        |status| status == RuntimeQuotaWindowStatus::Exhausted,
    ));

    let candidates = [
        RuntimeStartupProbeRefreshCandidate {
            profile_name: "fresh-probe",
            probe_fresh: true,
            snapshot_usable: false,
        },
        RuntimeStartupProbeRefreshCandidate {
            profile_name: "usable-snapshot",
            probe_fresh: false,
            snapshot_usable: true,
        },
        RuntimeStartupProbeRefreshCandidate {
            profile_name: "alpha",
            probe_fresh: false,
            snapshot_usable: false,
        },
        RuntimeStartupProbeRefreshCandidate {
            profile_name: "bravo",
            probe_fresh: false,
            snapshot_usable: false,
        },
        RuntimeStartupProbeRefreshCandidate {
            profile_name: "charlie",
            probe_fresh: false,
            snapshot_usable: false,
        },
    ];

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(candidates, 2),
        vec!["alpha", "bravo"]
    );

    let usable_snapshot = RuntimeProfileUsageSnapshot {
        checked_at: 190,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 90,
        five_hour_reset_at: 0,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 80,
        weekly_reset_at: 0,
    };
    let stale_snapshot = RuntimeProfileUsageSnapshot {
        checked_at: 100,
        ..usable_snapshot.clone()
    };
    let inputs = [
        RuntimeStartupProbeRefreshInput {
            profile_name: "fresh-probe",
            probe_checked_at: Some(195),
            usage_snapshot: None,
        },
        RuntimeStartupProbeRefreshInput {
            profile_name: "usable-snapshot",
            probe_checked_at: None,
            usage_snapshot: Some(&usable_snapshot),
        },
        RuntimeStartupProbeRefreshInput {
            profile_name: "alpha",
            probe_checked_at: None,
            usage_snapshot: Some(&stale_snapshot),
        },
        RuntimeStartupProbeRefreshInput {
            profile_name: "bravo",
            probe_checked_at: None,
            usage_snapshot: None,
        },
    ];

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh_from_snapshots(
            inputs,
            RuntimeStartupProbeRefreshPlan {
                now: 200,
                probe_fresh_seconds: 10,
                stale_grace_seconds: 60,
                warm_limit: 8,
            },
            |status| status == RuntimeQuotaWindowStatus::Exhausted,
        ),
        vec!["alpha", "bravo"]
    );
}

#[test]
fn usage_snapshot_persist_requires_material_change_or_stale_touch() {
    let previous = RuntimeProfileUsageSnapshot {
        checked_at: 100,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 90,
        five_hour_reset_at: 0,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 80,
        weekly_reset_at: 0,
    };
    let mut next = previous.clone();
    next.checked_at = 101;

    assert!(!runtime_profile_usage_snapshot_should_persist(
        Some(&previous),
        &next,
        110,
        60,
    ));
    assert!(runtime_profile_usage_snapshot_should_persist(
        Some(&previous),
        &next,
        161,
        60,
    ));
    next.weekly_remaining_percent = 70;
    assert!(runtime_profile_usage_snapshot_should_persist(
        Some(&previous),
        &next,
        110,
        60,
    ));
}

#[test]
fn probe_usage_apply_plan_sets_quarantine_and_persist_flags() {
    let previous = RuntimeProfileUsageSnapshot {
        checked_at: 100,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 90,
        five_hour_reset_at: 0,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 80,
        weekly_reset_at: 0,
    };
    let mut next = previous.clone();
    next.checked_at = 120;

    let plan = runtime_probe_usage_snapshot_apply_plan(RuntimeProbeUsageSnapshotApplyInput {
        previous_snapshot: Some(&previous),
        previous_retry_backoff_until: Some(130),
        next_snapshot: &next,
        quota_blocked: true,
        blocking_reset_at: Some(200),
        now: 120,
        quota_quarantine_fallback_seconds: 60,
        touch_persist_interval_seconds: 60,
    });

    assert!(!plan.snapshot_should_persist);
    assert_eq!(plan.blocking_reset_at, Some(200));
    assert_eq!(plan.retry_backoff_until, Some(200));
    assert!(plan.retry_backoff_changed);

    let plan = runtime_probe_usage_snapshot_apply_plan(RuntimeProbeUsageSnapshotApplyInput {
        previous_snapshot: Some(&previous),
        previous_retry_backoff_until: Some(250),
        next_snapshot: &next,
        quota_blocked: true,
        blocking_reset_at: Some(200),
        now: 120,
        quota_quarantine_fallback_seconds: 60,
        touch_persist_interval_seconds: 60,
    });

    assert_eq!(plan.retry_backoff_until, Some(250));
    assert!(!plan.retry_backoff_changed);
}

#[test]
fn runtime_state_lock_wait_counters_track_totals_and_max() {
    let counters = RuntimeStateLockWaitMetricCounters::default();

    counters.record_wait(Duration::from_nanos(10));
    counters.record_wait(Duration::from_nanos(25));

    assert_eq!(
        counters.snapshot(),
        RuntimeWaitDurationMetrics {
            wait_total_ns: 35,
            wait_count: 2,
            wait_max_ns: 25,
        }
    );

    counters.reset();

    assert_eq!(counters.snapshot(), RuntimeStateLockWaitMetrics::default());
}

#[test]
fn runtime_proxy_lane_admission_owns_distinct_shared_wait_metrics() {
    let admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: 4,
        compact: 2,
        websocket: 2,
        standard: 1,
    });
    admission
        .admission_wait_metric_counters()
        .record_wait(Duration::from_nanos(11));
    admission
        .long_lived_queue_wait_metric_counters()
        .record_wait(Duration::from_nanos(17));

    let cloned = admission.clone();
    assert_eq!(
        cloned.admission_wait_metric_counters().snapshot(),
        RuntimeWaitDurationMetrics {
            wait_total_ns: 11,
            wait_count: 1,
            wait_max_ns: 11,
        }
    );
    assert_eq!(
        cloned.long_lived_queue_wait_metric_counters().snapshot(),
        RuntimeWaitDurationMetrics {
            wait_total_ns: 17,
            wait_count: 1,
            wait_max_ns: 17,
        }
    );
}

#[test]
fn runtime_proxy_admission_permit_releases_global_and_lane_capacity() {
    let admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: 1,
        compact: 1,
        websocket: 1,
        standard: 1,
    });
    let active = Arc::new(AtomicUsize::new(0));

    let acquired = admission
        .try_acquire(Arc::clone(&active), 2, RuntimeRouteKind::Responses, false)
        .expect("capacity should be available");
    assert!(!acquired.bypassed_lane_limit);
    assert_eq!(active.load(Ordering::SeqCst), 1);
    assert_eq!(
        admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        1
    );

    drop(acquired.permit);

    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(
        admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        admission
            .releases_total_counter(RuntimeRouteKind::Responses)
            .load(Ordering::Relaxed),
        1
    );
}

#[test]
fn runtime_proxy_admission_bypasses_only_lane_limit() {
    let admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: 1,
        compact: 1,
        websocket: 1,
        standard: 1,
    });
    let active = Arc::new(AtomicUsize::new(0));
    let held = admission
        .try_acquire(Arc::clone(&active), 2, RuntimeRouteKind::Responses, false)
        .expect("first permit should be acquired");

    assert!(matches!(
        admission.try_acquire(Arc::clone(&active), 2, RuntimeRouteKind::Responses, false,),
        Err(RuntimeProxyAdmissionLimit::Lane {
            active: 1,
            limit: 1,
        })
    ));
    let bypassed = admission
        .try_acquire(Arc::clone(&active), 2, RuntimeRouteKind::Responses, true)
        .expect("owned affinity may bypass the lane limit");
    assert!(bypassed.bypassed_lane_limit);
    assert!(matches!(
        admission.try_acquire(Arc::clone(&active), 2, RuntimeRouteKind::Responses, true,),
        Err(RuntimeProxyAdmissionLimit::Global {
            active: 2,
            limit: 2,
        })
    ));

    drop(bypassed.permit);
    drop(held.permit);
}

#[test]
fn runtime_proxy_profile_inflight_state_is_shared_and_underflow_safe() {
    let admission = RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
        responses: 1,
        compact: 1,
        websocket: 1,
        standard: 1,
    });
    let cloned = admission.clone();

    assert_eq!(admission.acquire_profile_inflight("main", 2), 2);
    assert_eq!(cloned.profile_inflight_count("main"), 2);
    assert_eq!(cloned.release_profile_inflight("main", 1), (1, 2, false));
    assert_eq!(admission.release_profile_inflight("main", 2), (0, 1, true));
    assert!(admission.profile_inflight_snapshot().is_empty());
    assert_eq!(admission.profile_inflight_admissions_total(), 1);
    assert_eq!(admission.profile_inflight_releases_total(), 2);
    assert_eq!(admission.profile_inflight_release_underflows_total(), 1);
}
