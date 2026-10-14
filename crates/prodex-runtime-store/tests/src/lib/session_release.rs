use super::*;
use std::collections::BTreeMap;

fn status_policy() -> RuntimeContinuationStatusPolicy {
    RuntimeContinuationStatusPolicy {
        touch_persist_interval_seconds: 10,
        suspect_grace_seconds: 120,
        suspect_not_found_streak_limit: 2,
        confidence_max: 8,
        verified_confidence_bonus: 2,
        touch_confidence_bonus: 1,
        suspect_confidence_penalty: 1,
    }
}

fn owner(name: &str, at: i64) -> ResponseProfileBinding {
    ResponseProfileBinding {
        profile_name: name.to_owned(),
        bound_at: at,
        binding_identity: None,
    }
}

fn session_store(name: &str, at: i64) -> RuntimeContinuationStore<ResponseProfileBinding> {
    RuntimeContinuationStore {
        session_id_bindings: BTreeMap::from([("session".into(), owner(name, at))]),
        session_profile_bindings: BTreeMap::from([("session".into(), owner(name, at))]),
        ..Default::default()
    }
}

#[test]
fn released_session_does_not_conflict_with_stale_previous_owner_after_rebind() {
    let profiles = BTreeMap::from([("alpha".into(), profile()), ("beta".into(), profile())]);
    let stale = session_store("alpha", 100);
    let mut fresh = RuntimeContinuationStore::default();
    runtime_mark_continuation_status_dead(
        &mut fresh.statuses,
        RuntimeContinuationBindingKind::SessionId,
        "session",
        110,
        status_policy(),
    );
    fresh
        .session_id_bindings
        .insert("session".into(), owner("beta", 120));
    fresh
        .session_profile_bindings
        .insert("session".into(), owner("beta", 120));
    runtime_mark_continuation_status_verified(
        &mut fresh.statuses,
        RuntimeContinuationBindingKind::SessionId,
        "session",
        120,
        Some("responses"),
        status_policy(),
    );
    for (left, right) in [(&stale, &fresh), (&fresh, &stale)] {
        let merged = merge_runtime_continuation_store(
            left,
            right,
            &profiles,
            125,
            RuntimeContinuationCompactionPolicy::default(),
        );
        assert_eq!(
            merged.session_id_bindings["session"].profile_name, "beta",
            "stale worker snapshot must not poison explicitly released and successfully rebound session"
        );
        assert_eq!(
            merged.session_profile_bindings["session"].profile_name,
            "beta"
        );
        let serialized = serde_json::to_string(&merged).unwrap();
        let restored = serde_json::from_str(&serialized).unwrap();
        let replayed = merge_runtime_continuation_store(
            &restored,
            &stale,
            &profiles,
            126,
            RuntimeContinuationCompactionPolicy::default(),
        );
        assert_eq!(
            replayed.session_id_bindings["session"].profile_name, "beta",
            "release fence must survive persisted JSON and stale snapshot replay"
        );
    }
}

#[test]
fn explicit_session_release_can_remove_an_old_persisted_conflict_without_releasing_responses() {
    let profiles = BTreeMap::from([("alpha".into(), profile()), ("beta".into(), profile())]);
    let mut stale = session_store(prodex_state::HARD_BINDING_CONFLICT_PROFILE, 100);
    stale.response_profile_bindings.insert(
        "immutable-response".into(),
        owner(prodex_state::HARD_BINDING_CONFLICT_PROFILE, 100),
    );
    let mut released = RuntimeContinuationStore::default();
    runtime_mark_continuation_status_dead(
        &mut released.statuses,
        RuntimeContinuationBindingKind::SessionId,
        "session",
        110,
        status_policy(),
    );
    let merged = merge_runtime_continuation_store(
        &stale,
        &released,
        &profiles,
        120,
        RuntimeContinuationCompactionPolicy::default(),
    );
    assert!(
        !merged.session_id_bindings.contains_key("session"),
        "explicit root-session release must survive merge"
    );
    assert!(!merged.session_profile_bindings.contains_key("session"));
    assert_eq!(
        merged.response_profile_bindings["immutable-response"].profile_name,
        prodex_state::HARD_BINDING_CONFLICT_PROFILE,
        "never weaken immutable response conflict guard"
    );
}

#[test]
fn differing_session_owners_without_release_remain_a_genuine_conflict() {
    let profiles = BTreeMap::from([("alpha".into(), profile()), ("beta".into(), profile())]);
    let left = session_store("alpha", 100);
    let right = session_store("beta", 120);
    let merged = merge_runtime_continuation_store(
        &left,
        &right,
        &profiles,
        125,
        RuntimeContinuationCompactionPolicy::default(),
    );
    assert_eq!(
        merged.session_id_bindings["session"].profile_name,
        prodex_state::HARD_BINDING_CONFLICT_PROFILE
    );
}
