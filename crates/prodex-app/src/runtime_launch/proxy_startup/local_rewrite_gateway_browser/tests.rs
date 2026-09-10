use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use super::store::{
    BROWSER_SESSION_TTL_MS, BROWSER_TRANSACTION_TTL_MS, BrowserEphemeralPersistence,
    MAX_BROWSER_SESSIONS, MAX_BROWSER_TRANSACTIONS, RuntimeGatewayBrowserState, SESSION_KEY_PREFIX,
    browser_store_session_with_persistence, browser_store_transaction_with_persistence,
    mark_session_mutation,
};
use super::{
    BACKCHANNEL_LOGOUT_EVENT, LOGOUT_INDEX_KEY_PREFIX, RuntimeGatewayBrowserRoute,
    RuntimeGatewayBrowserSession, RuntimeGatewayBrowserTransaction, SESSION_COOKIE,
    browser_backchannel_logout_keys, browser_protect_id_token, browser_route,
    browser_session_cookie_parts, browser_unprotect_id_token, cookie_from_headers, parse_query,
};

#[test]
fn browser_routes_queries_and_cookies_are_exact() {
    assert!(matches!(
        browser_route("/v1/prodex/gateway/auth/login"),
        Some(RuntimeGatewayBrowserRoute::Login)
    ));
    assert!(browser_route("/v1/prodex/gateway/auth/login/extra").is_none());
    assert!(matches!(
        browser_route("/v1/prodex/gateway/auth/backchannel-logout"),
        Some(RuntimeGatewayBrowserRoute::BackchannelLogout)
    ));
    assert!(parse_query("code=a&state=b").is_ok());
    assert!(parse_query("state=a&state=b").is_err());
    let headers = vec![(
        "Cookie".to_string(),
        "other=x; prodex_gateway_session=session_1".to_string(),
    )];
    assert_eq!(
        cookie_from_headers(&headers, SESSION_COOKIE),
        Some("session_1")
    );
}

#[test]
fn browser_shared_records_round_trip_with_protected_authentication_state() {
    let transaction = RuntimeGatewayBrowserTransaction {
        nonce: "nonce".to_string(),
        code_verifier: "verifier".to_string(),
        expires_at_unix_ms: 123,
    };
    let transaction: RuntimeGatewayBrowserTransaction =
        serde_json::from_str(&serde_json::to_string(&transaction).unwrap()).unwrap();
    assert_eq!(transaction.nonce, "nonce");

    let raw_id_token = "fixture.id.token";
    let protection_key = [9; 32];
    let session = RuntimeGatewayBrowserSession {
        protected_id_token: browser_protect_id_token(
            "fixture-session",
            &protection_key,
            raw_id_token,
        )
        .ok()
        .unwrap(),
        csrf_digest: [7; 32],
        logout_keys: vec![format!("{LOGOUT_INDEX_KEY_PREFIX}fixture")],
        expires_at_unix_ms: 456,
    };
    assert_ne!(session.protected_id_token, raw_id_token);
    let serialized = serde_json::to_string(&session).unwrap();
    assert!(!serialized.contains(raw_id_token));
    let session: RuntimeGatewayBrowserSession = serde_json::from_str(&serialized).unwrap();
    let decrypted = browser_unprotect_id_token(
        "fixture-session",
        &protection_key,
        &session.protected_id_token,
    )
    .ok()
    .unwrap();
    assert_eq!(std::str::from_utf8(&decrypted).unwrap(), raw_id_token);
    assert!(browser_session_cookie_parts("session_1").is_none());
}

#[test]
fn backchannel_logout_claims_are_recent_event_bound_and_hashed() {
    let claims = BTreeMap::from([
        ("iat".to_string(), serde_json::json!(1_000)),
        (
            "jti".to_string(),
            serde_json::json!("logout-token-identifier"),
        ),
        (
            "sid".to_string(),
            serde_json::json!("private-session-identifier"),
        ),
        (
            "sub".to_string(),
            serde_json::json!("private-subject-identifier"),
        ),
        (
            "events".to_string(),
            serde_json::json!({BACKCHANNEL_LOGOUT_EVENT: {}}),
        ),
    ]);
    let keys = browser_backchannel_logout_keys(
        &claims,
        "https://identity.example.com",
        "prodex-gateway",
        1_001,
    )
    .ok()
    .unwrap();
    assert_eq!(keys.len(), 2);
    assert!(
        keys.iter()
            .all(|key| key.starts_with(LOGOUT_INDEX_KEY_PREFIX))
    );
    assert!(keys.iter().all(|key| {
        !key.contains("private-session-identifier") && !key.contains("private-subject-identifier")
    }));

    let mut invalid = claims;
    invalid.insert("nonce".to_string(), serde_json::json!("not-allowed"));
    assert!(
        browser_backchannel_logout_keys(
            &invalid,
            "https://identity.example.com",
            "prodex-gateway",
            1_001,
        )
        .is_err()
    );
}

#[derive(Clone, Copy)]
enum PutOutcome {
    Stored,
    NotStored,
    Error,
}

#[derive(Clone, Copy)]
enum AddOutcome {
    Stored,
    Error,
}

struct FakePersistence {
    put_outcome: PutOutcome,
    add_outcome: AddOutcome,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
    add_keys: Vec<String>,
    add_ttls: Vec<Duration>,
    deleted_keys: Vec<String>,
    removed_members: Vec<(String, String)>,
    on_put: Option<Box<dyn FnMut()>>,
}

impl FakePersistence {
    fn new(put_outcome: PutOutcome, add_outcome: AddOutcome) -> Self {
        Self {
            put_outcome,
            add_outcome,
            put_keys: Vec::new(),
            put_ttls: Vec::new(),
            add_keys: Vec::new(),
            add_ttls: Vec::new(),
            deleted_keys: Vec::new(),
            removed_members: Vec::new(),
            on_put: None,
        }
    }
}

impl BrowserEphemeralPersistence for FakePersistence {
    fn put_ephemeral(&mut self, key: &str, _value: &str, ttl: Duration) -> Result<bool, ()> {
        self.put_keys.push(key.to_string());
        self.put_ttls.push(ttl);
        if let Some(on_put) = self.on_put.as_mut() {
            on_put();
        }
        match self.put_outcome {
            PutOutcome::Stored => Ok(true),
            PutOutcome::NotStored => Ok(false),
            PutOutcome::Error => Err(()),
        }
    }

    fn add_ephemeral_member(&mut self, key: &str, _member: &str, ttl: Duration) -> Result<(), ()> {
        self.add_keys.push(key.to_string());
        self.add_ttls.push(ttl);
        match self.add_outcome {
            AddOutcome::Stored => Ok(()),
            AddOutcome::Error => Err(()),
        }
    }

    fn delete_ephemeral(&mut self, key: &str) -> Result<(), ()> {
        self.deleted_keys.push(key.to_string());
        Ok(())
    }

    fn remove_ephemeral_member(&mut self, key: &str, member: &str) -> Result<(), ()> {
        self.removed_members
            .push((key.to_string(), member.to_string()));
        Ok(())
    }
}

fn transaction(nonce: &str, expires_at_unix_ms: u64) -> RuntimeGatewayBrowserTransaction {
    RuntimeGatewayBrowserTransaction {
        nonce: nonce.to_string(),
        code_verifier: format!("verifier-{nonce}"),
        expires_at_unix_ms,
    }
}

fn session(name: &str, expires_at_unix_ms: u64) -> RuntimeGatewayBrowserSession {
    RuntimeGatewayBrowserSession {
        protected_id_token: format!("protected-{name}"),
        csrf_digest: [0; 32],
        logout_keys: vec![format!("logout-{name}")],
        expires_at_unix_ms,
    }
}

#[test]
fn transaction_persistence_failure_restores_prior_state() {
    for outcome in [PutOutcome::NotStored, PutOutcome::Error] {
        let state = RuntimeGatewayBrowserState::default();
        let previous = Arc::new(transaction("previous", 100));
        state
            .transactions
            .lock()
            .unwrap()
            .insert("state".to_string(), Arc::clone(&previous));
        let mut persistence = FakePersistence::new(outcome, AddOutcome::Stored);

        let result = browser_store_transaction_with_persistence(
            &state,
            "state".to_string(),
            transaction("replacement", 100),
            10,
            Some(&mut persistence),
        );

        assert!(result.is_err());
        let transactions = state.transactions.lock().unwrap();
        assert!(
            transactions
                .get("state")
                .is_some_and(|current| Arc::ptr_eq(current, &previous))
        );
        assert_eq!(
            persistence.put_ttls,
            vec![Duration::from_millis(BROWSER_TRANSACTION_TTL_MS)]
        );
    }
}

#[test]
fn session_persistence_failure_restores_prior_state() {
    for outcome in [PutOutcome::NotStored, PutOutcome::Error] {
        let state = RuntimeGatewayBrowserState::default();
        let previous = Arc::new(session("previous", 100));
        state
            .sessions
            .lock()
            .unwrap()
            .insert("session".to_string(), Arc::clone(&previous));
        let mut persistence = FakePersistence::new(outcome, AddOutcome::Stored);

        let result = browser_store_session_with_persistence(
            &state,
            "session".to_string(),
            session("replacement", 100),
            10,
            Some(&mut persistence),
        );

        assert!(result.is_err());
        let sessions = state.sessions.lock().unwrap();
        assert!(
            sessions
                .get("session")
                .is_some_and(|current| Arc::ptr_eq(current, &previous))
        );
        assert_eq!(
            persistence.put_ttls,
            vec![Duration::from_millis(BROWSER_SESSION_TTL_MS)]
        );
    }
}

#[test]
fn session_persistence_failure_restores_evicted_entry() {
    for outcome in [PutOutcome::NotStored, PutOutcome::Error] {
        let state = RuntimeGatewayBrowserState::default();
        let oldest = Arc::new(session("oldest", 100));
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert("oldest".to_string(), Arc::clone(&oldest));
        for index in 1..MAX_BROWSER_SESSIONS {
            sessions.insert(
                format!("session-{index}"),
                Arc::new(session(&format!("session-{index}"), 100 + index as u64)),
            );
        }
        drop(sessions);
        let mut persistence = FakePersistence::new(outcome, AddOutcome::Stored);

        let result = browser_store_session_with_persistence(
            &state,
            "provisional".to_string(),
            session("provisional", 10_000),
            10,
            Some(&mut persistence),
        );

        assert!(result.is_err());
        let sessions = state.sessions.lock().unwrap();
        assert_eq!(sessions.len(), MAX_BROWSER_SESSIONS);
        assert!(
            sessions
                .get("oldest")
                .is_some_and(|current| Arc::ptr_eq(current, &oldest))
        );
        assert!(!sessions.contains_key("provisional"));
    }
}

#[test]
fn persistence_failure_preserves_replacement_owner() {
    let state = RuntimeGatewayBrowserState::default();
    let sessions = Arc::clone(&state.sessions);
    let replacement = Arc::new(session("replacement", 100));
    let replacement_for_hook = Arc::clone(&replacement);
    let mut persistence = FakePersistence::new(PutOutcome::Error, AddOutcome::Stored);
    persistence.on_put = Some(Box::new(move || {
        sessions
            .lock()
            .unwrap()
            .insert("session".to_string(), Arc::clone(&replacement_for_hook));
    }));

    let result = browser_store_session_with_persistence(
        &state,
        "session".to_string(),
        session("provisional", 100),
        10,
        Some(&mut persistence),
    );

    assert!(result.is_err());
    assert!(
        state
            .sessions
            .lock()
            .unwrap()
            .get("session")
            .is_some_and(|current| Arc::ptr_eq(current, &replacement))
    );
}

#[test]
fn persistence_failure_preserves_evicted_replacement_owner() {
    let state = RuntimeGatewayBrowserState::default();
    let oldest = Arc::new(session("oldest", 100));
    let replacement = Arc::new(session("replacement", 100));
    let replacement_for_hook = Arc::clone(&replacement);
    let mut sessions = state.sessions.lock().unwrap();
    sessions.insert("oldest".to_string(), Arc::clone(&oldest));
    for index in 1..MAX_BROWSER_SESSIONS {
        sessions.insert(
            format!("session-{index}"),
            Arc::new(session(&format!("session-{index}"), 100 + index as u64)),
        );
    }
    drop(sessions);
    let sessions = Arc::clone(&state.sessions);
    let mut persistence = FakePersistence::new(PutOutcome::Error, AddOutcome::Stored);
    persistence.on_put = Some(Box::new(move || {
        sessions
            .lock()
            .unwrap()
            .insert("oldest".to_string(), Arc::clone(&replacement_for_hook));
    }));

    let result = browser_store_session_with_persistence(
        &state,
        "provisional".to_string(),
        session("provisional", 10_000),
        10,
        Some(&mut persistence),
    );

    assert!(result.is_err());
    assert!(
        state
            .sessions
            .lock()
            .unwrap()
            .get("oldest")
            .is_some_and(|current| Arc::ptr_eq(current, &replacement))
    );
}

#[test]
fn session_persistence_failure_does_not_restore_concurrently_deleted_evicted_entry() {
    for outcome in [PutOutcome::NotStored, PutOutcome::Error] {
        for recreate in [false, true] {
            let state = RuntimeGatewayBrowserState::default();
            let oldest = Arc::new(session("oldest", 100));
            let mut sessions = state.sessions.lock().unwrap();
            sessions.insert("oldest".to_string(), Arc::clone(&oldest));
            for index in 1..MAX_BROWSER_SESSIONS {
                sessions.insert(
                    format!("session-{index}"),
                    Arc::new(session(&format!("session-{index}"), 100 + index as u64)),
                );
            }
            drop(sessions);

            let recreated = Arc::new(session("recreated", 20_000));
            let recreated_for_hook = Arc::clone(&recreated);
            let state_for_hook = state.clone();
            let mut persistence = FakePersistence::new(outcome, AddOutcome::Stored);
            persistence.on_put = Some(Box::new(move || {
                let _ = mark_session_mutation(&state_for_hook, "oldest");
                if recreate {
                    state_for_hook
                        .sessions
                        .lock()
                        .unwrap()
                        .insert("oldest".to_string(), Arc::clone(&recreated_for_hook));
                }
            }));

            let result = browser_store_session_with_persistence(
                &state,
                "provisional".to_string(),
                session("provisional", 10_000),
                10,
                Some(&mut persistence),
            );

            assert!(result.is_err());
            let sessions = state.sessions.lock().unwrap();
            assert!(!sessions.contains_key("provisional"));
            assert!(
                sessions
                    .values()
                    .all(|current| !Arc::ptr_eq(current, &oldest))
            );
            if recreate {
                assert_eq!(sessions.len(), MAX_BROWSER_SESSIONS);
                assert!(
                    sessions
                        .get("oldest")
                        .is_some_and(|current| Arc::ptr_eq(current, &recreated))
                );
            } else {
                assert_eq!(sessions.len(), MAX_BROWSER_SESSIONS - 1);
                assert!(!sessions.contains_key("oldest"));
            }
        }
    }
}

#[test]
fn session_index_failure_cleans_remote_state_and_rolls_back_local_state() {
    let state = RuntimeGatewayBrowserState::default();
    let mut persistence = FakePersistence::new(PutOutcome::Stored, AddOutcome::Error);

    let result = browser_store_session_with_persistence(
        &state,
        "session".to_string(),
        session("session", 100),
        10,
        Some(&mut persistence),
    );

    assert!(result.is_err());
    assert!(!state.sessions.lock().unwrap().contains_key("session"));
    assert_eq!(
        persistence.deleted_keys,
        vec![format!("{SESSION_KEY_PREFIX}session")]
    );
    assert_eq!(
        persistence.removed_members,
        vec![("logout-session".to_string(), "session".to_string())]
    );
    assert_eq!(
        persistence.add_ttls,
        vec![Duration::from_millis(BROWSER_SESSION_TTL_MS)]
    );
}

#[test]
fn browser_shadow_expiry_capacity_and_ttl_contracts_match() {
    let transactions = RuntimeGatewayBrowserState::default();
    transactions.transactions.lock().unwrap().extend([
        ("expired".to_string(), Arc::new(transaction("expired", 10))),
        ("live".to_string(), Arc::new(transaction("live", 20))),
    ]);
    let mut transaction_persistence = FakePersistence::new(PutOutcome::Stored, AddOutcome::Stored);
    browser_store_transaction_with_persistence(
        &transactions,
        "new".to_string(),
        transaction("new", 30),
        15,
        Some(&mut transaction_persistence),
    )
    .ok()
    .unwrap();
    let transaction_entries = transactions.transactions.lock().unwrap();
    assert!(!transaction_entries.contains_key("expired"));
    assert!(transaction_entries.contains_key("live"));
    assert_eq!(
        transaction_persistence.put_ttls,
        vec![Duration::from_millis(BROWSER_TRANSACTION_TTL_MS)]
    );

    let sessions = RuntimeGatewayBrowserState::default();
    sessions.sessions.lock().unwrap().extend([
        ("expired".to_string(), Arc::new(session("expired", 10))),
        ("live".to_string(), Arc::new(session("live", 20))),
    ]);
    let mut session_persistence = FakePersistence::new(PutOutcome::Stored, AddOutcome::Stored);
    browser_store_session_with_persistence(
        &sessions,
        "new".to_string(),
        session("new", 30),
        15,
        Some(&mut session_persistence),
    )
    .ok()
    .unwrap();
    let session_entries = sessions.sessions.lock().unwrap();
    assert!(!session_entries.contains_key("expired"));
    assert!(session_entries.contains_key("live"));
    assert_eq!(
        session_persistence.put_ttls,
        vec![Duration::from_millis(BROWSER_SESSION_TTL_MS)]
    );
    assert_eq!(
        session_persistence.add_ttls,
        vec![Duration::from_millis(BROWSER_SESSION_TTL_MS)]
    );

    let full_transactions = RuntimeGatewayBrowserState::default();
    full_transactions
        .transactions
        .lock()
        .unwrap()
        .extend((0..MAX_BROWSER_TRANSACTIONS).map(|index| {
            (
                format!("state-{index}"),
                Arc::new(transaction(&format!("nonce-{index}"), 100)),
            )
        }));
    let mut rejected_persistence = FakePersistence::new(PutOutcome::Stored, AddOutcome::Stored);
    assert!(
        browser_store_transaction_with_persistence(
            &full_transactions,
            "rejected".to_string(),
            transaction("rejected", 100),
            10,
            Some(&mut rejected_persistence),
        )
        .is_err()
    );
    assert!(rejected_persistence.put_keys.is_empty());
}
