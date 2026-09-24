use super::*;
use prodex_mojo_core::runtime::{
    CONTINUATION_DEAD_PLAN, CONTINUATION_DEAD_SHADOWED, CONTINUATION_EVIDENCE_KEY,
    CONTINUATION_RECENTLY_SUSPECT, CONTINUATION_RETAIN_WITH_BINDING,
    CONTINUATION_RETAIN_WITHOUT_BINDING, CONTINUATION_RETENTION_KEY,
    CONTINUATION_SHOULD_PERSIST_TOUCH, CONTINUATION_SHOULD_REFRESH_VERIFIED,
    CONTINUATION_SHOULD_REPLACE, CONTINUATION_STALE_VERIFIED, CONTINUATION_SUSPECT_PLAN,
    CONTINUATION_TERMINAL_STATUS, CONTINUATION_TOUCH_PLAN, CONTINUATION_TOUCH_SHOULD_PERSIST,
    CONTINUATION_VERIFY_PLAN, continuation_status_transition,
};

const WIDTH: usize = 12;

fn state_tag(state: RuntimeContinuationBindingLifecycle) -> i64 {
    state as i64
}

fn state_from_tag(tag: i64) -> RuntimeContinuationBindingLifecycle {
    match tag {
        0 => RuntimeContinuationBindingLifecycle::Warm,
        1 => RuntimeContinuationBindingLifecycle::Verified,
        2 => RuntimeContinuationBindingLifecycle::Suspect,
        3 => RuntimeContinuationBindingLifecycle::Dead,
        _ => panic!("invalid Mojo continuation lifecycle"),
    }
}

fn fields(status: &RuntimeContinuationBindingStatus) -> [i64; WIDTH] {
    [
        state_tag(status.state),
        i64::from(status.confidence),
        i64::from(status.last_touched_at.is_some()),
        status.last_touched_at.unwrap_or(0),
        i64::from(status.last_verified_at.is_some()),
        status.last_verified_at.unwrap_or(0),
        i64::from(status.last_verified_route.is_some()),
        i64::from(status.last_not_found_at.is_some()),
        status.last_not_found_at.unwrap_or(0),
        i64::from(status.not_found_streak),
        i64::from(status.success_count),
        i64::from(status.failure_count),
    ]
}

fn call<const N: usize>(
    op: i64,
    status: &RuntimeContinuationBindingStatus,
    extra: &[i64],
) -> [i64; N] {
    let mut input = [0_i64; 32];
    input[..WIDTH].copy_from_slice(&fields(status));
    input[WIDTH..WIDTH + extra.len()].copy_from_slice(extra);
    continuation_status_transition(op, &input[..WIDTH + extra.len()])
        .expect("Mojo continuation policy rejected valid Rust state")
}

fn pair<const N: usize>(
    op: i64,
    left: &RuntimeContinuationBindingStatus,
    right: &RuntimeContinuationBindingStatus,
    extra: &[i64],
) -> [i64; N] {
    let mut input = [0_i64; 32];
    input[..WIDTH].copy_from_slice(&fields(left));
    input[WIDTH..WIDTH * 2].copy_from_slice(&fields(right));
    input[WIDTH * 2..WIDTH * 2 + extra.len()].copy_from_slice(extra);
    continuation_status_transition(op, &input[..WIDTH * 2 + extra.len()])
        .expect("Mojo continuation comparison rejected valid Rust state")
}

fn boolean(value: i64) -> bool {
    assert!(matches!(value, 0 | 1), "invalid Mojo continuation boolean");
    value == 1
}

fn optional_time(flag: i64, value: i64) -> Option<i64> {
    assert!(matches!(flag, 0 | 1), "invalid Mojo continuation timestamp");
    (flag == 1).then_some(value)
}

fn apply(
    status: &mut RuntimeContinuationBindingStatus,
    op: i64,
    extra: &[i64],
    route: Option<Option<&str>>,
) -> bool {
    let previous = status.clone();
    let out = call::<12>(op, &previous, extra);
    status.state = state_from_tag(out[0]);
    status.confidence = u32::try_from(out[1]).expect("continuation confidence");
    status.last_touched_at = optional_time(out[2], out[3]);
    status.last_verified_at = optional_time(out[4], out[5]);
    status.last_verified_route = match out[6] {
        0 => None,
        1 => route
            .unwrap_or(previous.last_verified_route.as_deref())
            .map(str::to_owned),
        _ => panic!("invalid Mojo continuation route"),
    };
    status.last_not_found_at = optional_time(out[7], out[8]);
    status.not_found_streak = u32::try_from(out[9]).expect("continuation streak");
    status.success_count = u32::try_from(out[10]).expect("continuation successes");
    status.failure_count = u32::try_from(out[11]).expect("continuation failures");
    *status != previous
}

fn entry<'a>(
    statuses: &'a mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
) -> &'a mut RuntimeContinuationBindingStatus {
    super::runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_owned())
        .or_default()
}

macro_rules! compaction_bool {
    ($name:ident, $op:ident $(, $field:ident)*) => {
        pub fn $name(
            status: &RuntimeContinuationBindingStatus,
            now: i64,
            policy: RuntimeContinuationCompactionPolicy,
        ) -> bool {
            boolean(call::<1>($op, status, &[now $(, i64::from(policy.$field))*])[0])
        }
    };
}

macro_rules! optional_status_bool {
    ($name:ident, $op:ident, $($field:ident),+ $(,)?) => {
        pub fn $name(
            status: Option<&RuntimeContinuationBindingStatus>,
            now: i64,
            policy: RuntimeContinuationStatusPolicy,
        ) -> bool {
            let present = status.is_some();
            let default = RuntimeContinuationBindingStatus::default();
            boolean(call::<1>(
                $op,
                status.unwrap_or(&default),
                &[i64::from(present), now, $(i64::from(policy.$field)),+],
            )[0])
        }
    };
}

pub fn runtime_binding_touch_should_persist(bound_at: i64, now: i64, interval: i64) -> bool {
    boolean(
        continuation_status_transition::<1>(
            CONTINUATION_TOUCH_SHOULD_PERSIST,
            &[bound_at, now, interval],
        )
        .expect("Mojo continuation touch policy rejected valid timestamps")[0],
    )
}

optional_status_bool!(
    runtime_continuation_status_should_persist_touch,
    CONTINUATION_SHOULD_PERSIST_TOUCH,
    suspect_grace_seconds,
    touch_persist_interval_seconds,
);
optional_status_bool!(
    runtime_continuation_status_recently_suspect,
    CONTINUATION_RECENTLY_SUSPECT,
    suspect_grace_seconds,
    suspect_not_found_streak_limit,
);

pub fn runtime_continuation_status_should_refresh_verified(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    route: Option<&str>,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let present = status.is_some();
    let default = RuntimeContinuationBindingStatus::default();
    let status = status.unwrap_or(&default);
    boolean(
        call::<1>(
            CONTINUATION_SHOULD_REFRESH_VERIFIED,
            status,
            &[
                i64::from(present),
                now,
                policy.touch_persist_interval_seconds,
                i64::from(status.last_verified_route.as_deref() == route),
            ],
        )[0],
    )
}

pub fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    route: Option<&str>,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    apply(
        entry(statuses, kind, key),
        CONTINUATION_VERIFY_PLAN,
        &[
            now,
            i64::from(policy.confidence_max),
            i64::from(policy.verified_confidence_bonus),
            i64::from(route.is_some()),
        ],
        Some(route),
    )
}

macro_rules! mark_status {
    ($name:ident, $op:ident, $($field:ident),+ $(,)?) => {
        pub fn $name(
            statuses: &mut RuntimeContinuationStatuses,
            kind: RuntimeContinuationBindingKind,
            key: &str,
            now: i64,
            policy: RuntimeContinuationStatusPolicy,
        ) -> bool {
            apply(
                entry(statuses, kind, key),
                $op,
                &[now, $(i64::from(policy.$field)),+],
                None,
            )
        }
    };
}
mark_status!(
    runtime_mark_continuation_status_touched,
    CONTINUATION_TOUCH_PLAN,
    suspect_grace_seconds,
    confidence_max,
    touch_confidence_bonus,
);
mark_status!(
    runtime_mark_continuation_status_suspect,
    CONTINUATION_SUSPECT_PLAN,
    suspect_not_found_streak_limit,
    suspect_confidence_penalty,
);
mark_status!(
    runtime_mark_continuation_status_dead,
    CONTINUATION_DEAD_PLAN,
    suspect_not_found_streak_limit,
);

pub fn runtime_continuation_status_evidence_sort_key(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u32, u32, u32, u8, i64, i64, i64) {
    let out = call::<8>(
        CONTINUATION_EVIDENCE_KEY,
        status,
        &[i64::from(policy.confidence_max)],
    );
    (
        u8::try_from(out[0]).expect("continuation lifecycle rank"),
        u32::try_from(out[1]).expect("continuation confidence"),
        u32::try_from(out[2]).expect("continuation successes"),
        u32::try_from(out[3]).expect("continuation inverse streak"),
        u8::try_from(out[4]).expect("continuation route evidence"),
        out[5],
        out[6],
        out[7],
    )
}

macro_rules! compare_bool {
    ($name:ident, $op:ident, $($field:ident),+ $(,)?) => {
        pub fn $name(
            candidate: &RuntimeContinuationBindingStatus,
            current: &RuntimeContinuationBindingStatus,
            policy: RuntimeContinuationCompactionPolicy,
        ) -> bool {
            boolean(pair::<1>($op, candidate, current, &[$(i64::from(policy.$field)),+])[0])
        }
    };
}
compare_bool!(
    runtime_continuation_status_should_replace,
    CONTINUATION_SHOULD_REPLACE,
    suspect_not_found_streak_limit,
    confidence_max,
);

pub fn runtime_continuation_status_is_terminal(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    boolean(
        call::<1>(
            CONTINUATION_TERMINAL_STATUS,
            status,
            &[i64::from(policy.suspect_not_found_streak_limit)],
        )[0],
    )
}

compaction_bool!(
    runtime_continuation_status_is_stale_verified,
    CONTINUATION_STALE_VERIFIED,
    verified_stale_seconds
);
compaction_bool!(
    runtime_continuation_status_should_retain_with_binding,
    CONTINUATION_RETAIN_WITH_BINDING,
    suspect_grace_seconds,
    suspect_not_found_streak_limit
);
compaction_bool!(
    runtime_continuation_status_should_retain_without_binding,
    CONTINUATION_RETAIN_WITHOUT_BINDING,
    suspect_grace_seconds,
    suspect_not_found_streak_limit,
    dead_grace_seconds
);

pub fn runtime_continuation_dead_status_shadowed_by_binding(
    binding: &ResponseProfileBinding,
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    boolean(call::<1>(CONTINUATION_DEAD_SHADOWED, status, &[binding.bound_at])[0])
}

pub fn runtime_continuation_status_retention_sort_key(
    key: &str,
    status: &RuntimeContinuationBindingStatus,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let binding = bindings.get(key);
    let out = call::<10>(
        CONTINUATION_RETENTION_KEY,
        status,
        &[
            i64::from(policy.confidence_max),
            i64::from(binding.is_some()),
            binding.map(|value| value.bound_at).unwrap_or(0),
        ],
    );
    (
        u8::try_from(out[0]).expect("continuation binding evidence"),
        u8::try_from(out[1]).expect("continuation lifecycle rank"),
        u32::try_from(out[2]).expect("continuation confidence"),
        u32::try_from(out[3]).expect("continuation successes"),
        u32::try_from(out[4]).expect("continuation inverse streak"),
        u8::try_from(out[5]).expect("continuation route evidence"),
        out[6],
        out[7],
        out[8],
        out[9],
    )
}
