#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeProxyContinuationBindingLifecycle {
    #[default]
    Warm,
    Verified,
    Suspect,
    Dead,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProxyContinuationBindingStatus {
    pub state: RuntimeProxyContinuationBindingLifecycle,
    pub confidence: u32,
    pub last_touched_at: Option<i64>,
    pub last_verified_at: Option<i64>,
    pub last_verified_route: Option<String>,
    pub last_not_found_at: Option<i64>,
    pub not_found_streak: u32,
    pub success_count: u32,
    pub failure_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyContinuationPolicy {
    pub touch_persist_interval_seconds: i64,
    pub suspect_grace_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
    pub verified_confidence_bonus: u32,
    pub touch_confidence_bonus: u32,
    pub suspect_confidence_penalty: u32,
}

pub fn runtime_proxy_binding_touch_should_persist(
    bound_at: i64,
    now: i64,
    touch_persist_interval_seconds: i64,
) -> bool {
    // Timestamps use second precision. Require strictly more than the interval
    // so a boundary-crossing lookup does not persist nearly a second early.
    now.saturating_sub(bound_at) > touch_persist_interval_seconds
}

pub fn runtime_proxy_continuation_status_last_event_at(
    status: &RuntimeProxyContinuationBindingStatus,
) -> Option<i64> {
    [
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

pub fn runtime_proxy_continuation_status_is_terminal(
    status: &RuntimeProxyContinuationBindingStatus,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    status.state == RuntimeProxyContinuationBindingLifecycle::Dead
        || status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (status.state == RuntimeProxyContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub fn runtime_proxy_continuation_next_event_at(
    status: &RuntimeProxyContinuationBindingStatus,
    now: i64,
) -> i64 {
    runtime_proxy_continuation_status_last_event_at(status)
        .filter(|last| *last >= now)
        .map_or(now, |last| last.saturating_add(1))
}

pub fn runtime_proxy_continuation_status_touches(
    status: &mut RuntimeProxyContinuationBindingStatus,
    now: i64,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_proxy_continuation_next_event_at(&previous, now);
    status.last_touched_at = Some(event_at);
    if status.state == RuntimeProxyContinuationBindingLifecycle::Suspect {
        if status
            .last_not_found_at
            .is_some_and(|last| event_at.saturating_sub(last) >= policy.suspect_grace_seconds)
        {
            status.state = RuntimeProxyContinuationBindingLifecycle::Warm;
            status.not_found_streak = 0;
            status.last_not_found_at = None;
        }
        status.confidence = status
            .confidence
            .saturating_add(policy.touch_confidence_bonus)
            .min(policy.confidence_max);
    } else if status.state != RuntimeProxyContinuationBindingLifecycle::Dead {
        status.confidence = status
            .confidence
            .saturating_add(policy.touch_confidence_bonus)
            .min(policy.confidence_max);
    }
    *status != previous
}

pub fn runtime_proxy_continuation_status_should_refresh_verified(
    status: Option<&RuntimeProxyContinuationBindingStatus>,
    now: i64,
    verified_route_label: Option<&str>,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.state != RuntimeProxyContinuationBindingLifecycle::Verified {
        return true;
    }

    if status.last_verified_route.as_deref() != verified_route_label {
        return true;
    }

    status.last_verified_at.is_none_or(|last_verified_at| {
        runtime_proxy_binding_touch_should_persist(
            last_verified_at,
            now,
            policy.touch_persist_interval_seconds,
        )
    })
}

pub fn runtime_proxy_continuation_status_should_persist_touch(
    status: Option<&RuntimeProxyContinuationBindingStatus>,
    now: i64,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.state == RuntimeProxyContinuationBindingLifecycle::Suspect
        && status.last_not_found_at.is_some_and(|last_not_found_at| {
            now.saturating_sub(last_not_found_at) >= policy.suspect_grace_seconds
        })
    {
        return true;
    }

    status.last_touched_at.is_none_or(|last_touched_at| {
        runtime_proxy_binding_touch_should_persist(
            last_touched_at,
            now,
            policy.touch_persist_interval_seconds,
        )
    })
}

pub fn runtime_proxy_mark_continuation_status_verified(
    status: &mut RuntimeProxyContinuationBindingStatus,
    now: i64,
    verified_route_label: Option<&str>,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_proxy_continuation_next_event_at(&previous, now);
    status.state = RuntimeProxyContinuationBindingLifecycle::Verified;
    status.last_touched_at = Some(event_at);
    status.last_verified_at = Some(event_at);
    status.last_verified_route = verified_route_label.map(str::to_string);
    status.last_not_found_at = None;
    status.not_found_streak = 0;
    status.success_count = status.success_count.saturating_add(1);
    status.failure_count = 0;
    status.confidence = status
        .confidence
        .saturating_add(policy.verified_confidence_bonus)
        .min(policy.confidence_max);
    *status != previous
}

pub fn runtime_proxy_mark_continuation_status_suspect(
    status: &mut RuntimeProxyContinuationBindingStatus,
    now: i64,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_proxy_continuation_next_event_at(&previous, now);
    status.not_found_streak = status.not_found_streak.saturating_add(1);
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.failure_count = status.failure_count.saturating_add(1);
    let previous_confidence = status.confidence;
    status.confidence = status
        .confidence
        .saturating_sub(policy.suspect_confidence_penalty);
    if previous_confidence == 0 {
        status.confidence = 1;
    }
    status.state = if status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (previous_confidence > 0 && status.confidence == 0)
    {
        RuntimeProxyContinuationBindingLifecycle::Dead
    } else {
        RuntimeProxyContinuationBindingLifecycle::Suspect
    };
    *status != previous
}

pub fn runtime_proxy_mark_continuation_status_dead(
    status: &mut RuntimeProxyContinuationBindingStatus,
    now: i64,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_proxy_continuation_next_event_at(&previous, now);
    status.state = RuntimeProxyContinuationBindingLifecycle::Dead;
    status.confidence = 0;
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.not_found_streak = status
        .not_found_streak
        .max(policy.suspect_not_found_streak_limit);
    status.failure_count = status.failure_count.saturating_add(1);
    *status != previous
}

pub fn runtime_proxy_continuation_status_recently_suspect(
    status: Option<&RuntimeProxyContinuationBindingStatus>,
    now: i64,
    policy: RuntimeProxyContinuationPolicy,
) -> bool {
    status.is_some_and(|status| {
        status.state == RuntimeProxyContinuationBindingLifecycle::Suspect
            && !runtime_proxy_continuation_status_is_terminal(status, policy)
            && status
                .last_not_found_at
                .is_some_and(|last| now.saturating_sub(last) < policy.suspect_grace_seconds)
    })
}

pub fn runtime_proxy_continuation_status_label(
    status: &RuntimeProxyContinuationBindingStatus,
) -> &'static str {
    match status.state {
        RuntimeProxyContinuationBindingLifecycle::Warm => "warm",
        RuntimeProxyContinuationBindingLifecycle::Verified => "verified",
        RuntimeProxyContinuationBindingLifecycle::Suspect => "suspect",
        RuntimeProxyContinuationBindingLifecycle::Dead => "dead",
    }
}

pub fn runtime_proxy_continuation_dead_status_shadowed_by_binding_bound_at(
    binding_bound_at: i64,
    status: &RuntimeProxyContinuationBindingStatus,
) -> bool {
    (status.state == RuntimeProxyContinuationBindingLifecycle::Dead)
        .then(|| status.last_not_found_at.or(status.last_touched_at))
        .flatten()
        .is_some_and(|dead_at| binding_bound_at > dead_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RuntimeProxyContinuationPolicy {
        RuntimeProxyContinuationPolicy {
            touch_persist_interval_seconds: 1,
            suspect_grace_seconds: 5,
            suspect_not_found_streak_limit: 2,
            confidence_max: 8,
            verified_confidence_bonus: 2,
            touch_confidence_bonus: 1,
            suspect_confidence_penalty: 1,
        }
    }

    #[test]
    fn suspect_status_recovers_after_grace_touch() {
        let mut status = RuntimeProxyContinuationBindingStatus {
            state: RuntimeProxyContinuationBindingLifecycle::Suspect,
            confidence: 1,
            last_not_found_at: Some(10),
            not_found_streak: 1,
            failure_count: 1,
            ..RuntimeProxyContinuationBindingStatus::default()
        };

        assert!(runtime_proxy_continuation_status_touches(
            &mut status,
            15,
            policy(),
        ));

        assert_eq!(status.state, RuntimeProxyContinuationBindingLifecycle::Warm);
        assert_eq!(status.not_found_streak, 0);
        assert_eq!(status.last_not_found_at, None);
        assert_eq!(status.confidence, 2);
    }

    #[test]
    fn repeated_suspect_marks_dead_at_policy_limit() {
        let mut status = RuntimeProxyContinuationBindingStatus {
            confidence: 2,
            ..RuntimeProxyContinuationBindingStatus::default()
        };

        assert!(runtime_proxy_mark_continuation_status_suspect(
            &mut status,
            10,
            policy(),
        ));
        assert_eq!(
            status.state,
            RuntimeProxyContinuationBindingLifecycle::Suspect
        );
        assert!(runtime_proxy_mark_continuation_status_suspect(
            &mut status,
            11,
            policy(),
        ));
        assert_eq!(status.state, RuntimeProxyContinuationBindingLifecycle::Dead);
    }

    #[test]
    fn verified_refresh_checks_route_and_touch_interval() {
        let status = RuntimeProxyContinuationBindingStatus {
            state: RuntimeProxyContinuationBindingLifecycle::Verified,
            last_verified_at: Some(10),
            last_verified_route: Some("responses".to_string()),
            ..RuntimeProxyContinuationBindingStatus::default()
        };

        assert!(!runtime_proxy_continuation_status_should_refresh_verified(
            Some(&status),
            11,
            Some("responses"),
            policy(),
        ));
        assert!(runtime_proxy_continuation_status_should_refresh_verified(
            Some(&status),
            12,
            Some("responses"),
            policy(),
        ));
        assert!(runtime_proxy_continuation_status_should_refresh_verified(
            Some(&status),
            11,
            Some("compact"),
            policy(),
        ));
    }

    #[test]
    fn dead_status_shadowed_by_newer_binding() {
        let status = RuntimeProxyContinuationBindingStatus {
            state: RuntimeProxyContinuationBindingLifecycle::Dead,
            last_not_found_at: Some(10),
            ..RuntimeProxyContinuationBindingStatus::default()
        };

        assert!(runtime_proxy_continuation_dead_status_shadowed_by_binding_bound_at(11, &status));
        assert!(!runtime_proxy_continuation_dead_status_shadowed_by_binding_bound_at(10, &status));
    }
}
