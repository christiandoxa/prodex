use super::super::local_rewrite::RuntimeGatewayOidcHttpCacheEntry;
use super::super::local_rewrite_gateway_store_types::RuntimeGatewayScimUser;
use super::super::*;
use super::admin::*;
use super::cache::*;
use super::endpoint_policy::*;
use super::token_claims::*;
use super::transport::*;
use crate::{RuntimeConfig, read_blocking_response_body_with_limit};
use jsonwebtoken::Algorithm;
use prodex_authn::OidcEndpointPolicy;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV: &str = "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS: u64 = 2_000;
const RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV: &str = "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS: u64 = 300;
const RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV: &str =
    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS: u64 = 30_000;
const RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV: &str =
    "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS";
const DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS: u64 = 86_400;
const MAX_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS: u64 = 10_000;
const MAX_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS: u64 = 3_600_000;

#[test]
fn oidc_prefetch_timeout_uses_positive_env_override() {
    let _guard = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, "25");

    assert_eq!(
        runtime_gateway_oidc_prefetch_timeout(),
        Duration::from_millis(25)
    );
}

#[test]
fn oidc_prefetch_timeout_rejects_zero_and_invalid_values() {
    for value in ["0", "not-a-number"] {
        let _guard = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, value);

        assert_eq!(
            runtime_gateway_oidc_prefetch_timeout(),
            Duration::from_millis(DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS)
        );
    }
}

#[test]
fn oidc_http_cache_ttl_uses_positive_env_override() {
    let _guard = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "25");

    assert_eq!(
        runtime_gateway_oidc_http_cache_ttl(),
        Duration::from_secs(25)
    );
    assert_eq!(
        runtime_gateway_oidc_background_refresh_interval(runtime_gateway_oidc_http_cache_ttl()),
        Duration::from_secs(25)
    );
}

#[test]
fn oidc_http_cache_ttl_allows_zero_and_rejects_invalid_values() {
    let _zero = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "0");
    assert_eq!(runtime_gateway_oidc_http_cache_ttl(), Duration::ZERO);
    assert_eq!(
        runtime_gateway_oidc_background_refresh_interval(Duration::ZERO),
        Duration::from_millis(MIN_RUNTIME_GATEWAY_OIDC_BACKGROUND_REFRESH_MS)
    );

    drop(_zero);

    let _invalid =
        crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, "not-a-number");
    assert_eq!(
        runtime_gateway_oidc_http_cache_ttl(),
        Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS)
    );
}

#[test]
fn oidc_refresh_failure_backoff_uses_positive_env_override() {
    let _guard =
        crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, "75");

    assert_eq!(
        runtime_gateway_oidc_refresh_failure_backoff(),
        Duration::from_millis(75)
    );
}

#[test]
fn oidc_refresh_failure_backoff_rejects_zero_and_invalid_values() {
    for value in ["0", "not-a-number"] {
        let _guard =
            crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, value);

        assert_eq!(
            runtime_gateway_oidc_refresh_failure_backoff(),
            Duration::from_millis(DEFAULT_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS)
        );
    }
}

#[test]
fn oidc_last_known_good_window_allows_zero_and_rejects_invalid_values() {
    let _zero = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "0");
    assert_eq!(
        runtime_gateway_oidc_last_known_good_window(),
        Duration::ZERO
    );

    drop(_zero);

    let _valid = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "25");
    assert_eq!(
        runtime_gateway_oidc_last_known_good_window(),
        Duration::from_secs(25)
    );

    drop(_valid);

    let _invalid =
        crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, "not-a-number");
    assert_eq!(
        runtime_gateway_oidc_last_known_good_window(),
        Duration::from_secs(DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS)
    );
}

#[test]
fn oidc_cache_control_max_age_is_used_when_present() {
    assert_eq!(
        runtime_gateway_oidc_cache_control_max_age(Some("public, max-age=7")),
        Some(Duration::from_secs(7))
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_max_age(Some("max-age=\"8\", must-revalidate")),
        Some(Duration::from_secs(8))
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_max_age(Some("no-store")),
        None
    );
    assert_eq!(runtime_gateway_oidc_cache_control_max_age(None), None);
}

#[test]
fn oidc_cache_control_stale_while_revalidate_is_used_when_present() {
    assert_eq!(
        runtime_gateway_oidc_cache_control_stale_while_revalidate(Some(
            "public, max-age=7, stale-while-revalidate=9"
        )),
        Some(Duration::from_secs(9))
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_stale_while_revalidate(Some(
            "stale-while-revalidate=\"10\", must-revalidate"
        )),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_stale_while_revalidate(Some("max-age=7")),
        None
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_stale_while_revalidate(None),
        None
    );
}

#[test]
fn oidc_cache_entry_ttl_prefers_http_max_age_except_zero_override() {
    let entry = RuntimeGatewayOidcHttpCacheEntry {
        fetched_at: Instant::now(),
        max_age: Some(Duration::from_secs(7)),
        stale_while_revalidate: None,
    };
    assert_eq!(
        runtime_gateway_oidc_cache_entry_ttl(&entry, Duration::from_secs(300)),
        Duration::from_secs(7)
    );
    assert_eq!(
        runtime_gateway_oidc_cache_entry_ttl(&entry, Duration::ZERO),
        Duration::ZERO
    );

    let without_header = RuntimeGatewayOidcHttpCacheEntry {
        fetched_at: Instant::now(),
        max_age: None,
        stale_while_revalidate: None,
    };
    assert_eq!(
        runtime_gateway_oidc_cache_entry_ttl(&without_header, Duration::from_secs(300)),
        Duration::from_secs(300)
    );
}

#[test]
fn oidc_cache_entry_usable_is_bounded_by_lkg_window() {
    let now = Instant::now();
    let fresh = RuntimeGatewayOidcHttpCacheEntry {
        fetched_at: now - Duration::from_secs(9),
        max_age: None,
        stale_while_revalidate: None,
    };
    assert!(runtime_gateway_oidc_cache_entry_usable(
        &fresh,
        Duration::from_secs(10),
        Duration::from_secs(5),
        now
    ));

    let lkg = RuntimeGatewayOidcHttpCacheEntry {
        fetched_at: now - Duration::from_secs(12),
        max_age: None,
        stale_while_revalidate: None,
    };
    assert!(runtime_gateway_oidc_cache_entry_usable(
        &lkg,
        Duration::from_secs(10),
        Duration::from_secs(5),
        now
    ));

    let expired = RuntimeGatewayOidcHttpCacheEntry {
        fetched_at: now - Duration::from_secs(16),
        max_age: None,
        stale_while_revalidate: None,
    };
    assert!(!runtime_gateway_oidc_cache_entry_usable(
        &expired,
        Duration::from_secs(10),
        Duration::from_secs(5),
        now
    ));

    let response_stale_window = RuntimeGatewayOidcHttpCacheEntry {
        fetched_at: now - Duration::from_secs(12),
        max_age: Some(Duration::from_secs(10)),
        stale_while_revalidate: Some(Duration::from_secs(1)),
    };
    assert!(!runtime_gateway_oidc_cache_entry_usable(
        &response_stale_window,
        Duration::from_secs(10),
        Duration::from_secs(60),
        now
    ));
}

#[test]
fn oidc_runtime_limits_clamp_untrusted_env_and_cache_headers() {
    let _timeout = crate::TestEnvVarGuard::set(
        RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV,
        "18446744073709551615",
    );
    let _ttl = crate::TestEnvVarGuard::set(
        RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV,
        "18446744073709551615",
    );
    let _backoff = crate::TestEnvVarGuard::set(
        RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV,
        "18446744073709551615",
    );
    let _lkg = crate::TestEnvVarGuard::set(
        RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV,
        "18446744073709551615",
    );

    assert_eq!(
        runtime_gateway_oidc_prefetch_timeout(),
        Duration::from_millis(MAX_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS)
    );
    assert_eq!(
        runtime_gateway_oidc_http_cache_ttl(),
        Duration::from_secs(MAX_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS)
    );
    assert_eq!(
        runtime_gateway_oidc_refresh_failure_backoff(),
        Duration::from_millis(MAX_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS)
    );
    assert_eq!(
        runtime_gateway_oidc_last_known_good_window(),
        Duration::from_secs(MAX_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS)
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_max_age(Some(&format!("max-age={}", u64::MAX))),
        Some(Duration::from_secs(
            MAX_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS
        ))
    );
}

#[test]
fn oidc_network_policy_rejects_private_metadata_and_mapped_addresses() {
    for address in [
        "0.0.0.0",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.0.1",
        "224.0.0.1",
        "::",
        "::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "::ffff:127.0.0.1",
        "64:ff9b::a9fe:a9fe",
        "2001::1",
        "2002:7f00:1::",
    ] {
        assert!(
            runtime_gateway_oidc_ip_is_forbidden(address.parse().unwrap()),
            "forbidden OIDC address accepted: {address}"
        );
    }
    for address in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
        assert!(!runtime_gateway_oidc_ip_is_forbidden(
            address.parse().unwrap()
        ));
    }
}

#[test]
fn oidc_dns_resolution_rejects_loopback_answers_before_connect() {
    let policy =
        OidcEndpointPolicy::new("https://localhost", Some("https://localhost/jwks.json")).unwrap();
    let error =
        runtime_gateway_oidc_resolve(policy.configured_jwks().unwrap(), Duration::from_secs(1))
            .unwrap_err();
    assert!(error.to_string().contains("forbidden address"));
}

#[test]
fn oidc_connected_peer_must_match_pinned_dns_answer_and_port() {
    let resolved = ["1.1.1.1:443".parse().unwrap()];
    assert!(runtime_gateway_oidc_peer_is_allowed(
        "1.1.1.1:443".parse().unwrap(),
        &resolved,
    ));
    assert!(!runtime_gateway_oidc_peer_is_allowed(
        "8.8.8.8:443".parse().unwrap(),
        &resolved,
    ));
    assert!(!runtime_gateway_oidc_peer_is_allowed(
        "1.1.1.1:8443".parse().unwrap(),
        &resolved,
    ));
    assert!(!runtime_gateway_oidc_peer_is_allowed(
        "127.0.0.1:443".parse().unwrap(),
        &["127.0.0.1:443".parse().unwrap()],
    ));
}

#[test]
fn oidc_http_client_does_not_follow_redirects() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = server.server_addr().to_ip().unwrap();
    let worker = std::thread::spawn(move || {
        let request = server.recv().unwrap();
        let mut response = tiny_http::Response::empty(302);
        response.add_header(
            tiny_http::Header::from_bytes("location", "http://127.0.0.1/metadata").unwrap(),
        );
        request.respond(response).unwrap();
    });
    let endpoint = RuntimeGatewayOidcEndpoint::InsecureLoopback(format!("http://{address}/"));
    let error = runtime_gateway_oidc_send(
        &endpoint,
        "JWKS",
        &RuntimeConfig::compatibility_current().oidc,
    )
    .unwrap_err();
    worker.join().unwrap();

    assert!(error.to_string().contains("redirects are forbidden"));
}

#[test]
fn oidc_http_fetch_timeout_bounds_slow_peer_and_shutdown_work() {
    let _timeout = crate::TestEnvVarGuard::set(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, "50");
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = server.server_addr().to_ip().unwrap();
    let worker = std::thread::spawn(move || {
        let request = server.recv().unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let _ = request.respond(tiny_http::Response::from_string("{}"));
    });
    let endpoint = RuntimeGatewayOidcEndpoint::InsecureLoopback(format!("http://{address}/"));
    let started = Instant::now();
    let error = runtime_gateway_oidc_send(
        &endpoint,
        "JWKS",
        &RuntimeConfig::compatibility_current().oidc,
    )
    .unwrap_err();
    let elapsed = started.elapsed();
    worker.join().unwrap();

    assert!(elapsed < Duration::from_millis(250), "elapsed={elapsed:?}");
    assert!(error.to_string().contains("failed to fetch test OIDC"));
}

#[test]
fn oidc_document_body_limit_fails_closed() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = server.server_addr().to_ip().unwrap();
    let worker = std::thread::spawn(move || {
        let request = server.recv().unwrap();
        request
            .respond(tiny_http::Response::from_data(vec![
                b'x';
                MAX_RUNTIME_GATEWAY_OIDC_BODY_BYTES
                    + 1
            ]))
            .unwrap();
    });
    let endpoint = RuntimeGatewayOidcEndpoint::InsecureLoopback(format!("http://{address}/"));
    let response = runtime_gateway_oidc_send(
        &endpoint,
        "JWKS",
        &RuntimeConfig::compatibility_current().oidc,
    )
    .unwrap();
    let error = read_blocking_response_body_with_limit(
        response,
        MAX_RUNTIME_GATEWAY_OIDC_BODY_BYTES,
        "OIDC body too large",
    )
    .unwrap_err();
    worker.join().unwrap();

    assert!(error.to_string().contains("exceeded safe size limit"));
}

#[test]
fn oidc_algorithm_allowlist_rejects_symmetric_and_unknown_families() {
    for algorithm in [
        Algorithm::HS256,
        Algorithm::HS384,
        Algorithm::HS512,
        Algorithm::EdDSA,
    ] {
        assert!(runtime_gateway_oidc_algorithm(algorithm).is_err());
    }
    for algorithm in [
        Algorithm::RS256,
        Algorithm::RS384,
        Algorithm::RS512,
        Algorithm::ES256,
        Algorithm::ES384,
    ] {
        assert_eq!(
            runtime_gateway_oidc_algorithm(algorithm).unwrap(),
            algorithm
        );
    }
}

#[test]
fn oidc_cache_keys_are_fixed_length_and_secret_free() {
    let endpoint = RuntimeGatewayOidcEndpoint::InsecureLoopback(
        "http://127.0.0.1/jwks.json?token=do-not-cache".to_string(),
    );
    let key = endpoint.cache_key();
    assert_eq!(key.len(), 64);
    assert!(!key.contains("token"));
    assert!(!key.contains("do-not-cache"));
    assert_eq!(key, endpoint.cache_key());
}

#[test]
fn oidc_cache_evicts_oldest_entry_at_fixed_capacity() {
    let mut cache = BTreeMap::new();
    let started = Instant::now();
    for index in 0..=MAX_RUNTIME_GATEWAY_OIDC_CACHE_ENTRIES {
        runtime_gateway_oidc_insert_cache_entry(
            &mut cache,
            format!("key-{index}"),
            RuntimeGatewayOidcHttpCacheEntry {
                fetched_at: started + Duration::from_millis(index as u64),
                max_age: None,
                stale_while_revalidate: None,
            },
        );
    }
    assert_eq!(cache.len(), MAX_RUNTIME_GATEWAY_OIDC_CACHE_ENTRIES);
    assert!(!cache.contains_key("key-0"));
    assert!(cache.contains_key(&format!("key-{MAX_RUNTIME_GATEWAY_OIDC_CACHE_ENTRIES}")));
}

#[test]
fn oidc_jwks_key_count_is_bounded_before_publication() {
    let maximum = serde_json::json!({
        "keys": vec![serde_json::json!({}); MAX_RUNTIME_GATEWAY_OIDC_JWKS_KEYS],
    });
    assert!(runtime_gateway_oidc_validate_jwks_key_count(&maximum).is_ok());
    for invalid in [
        serde_json::json!({"keys": []}),
        serde_json::json!({
            "keys": vec![serde_json::json!({}); MAX_RUNTIME_GATEWAY_OIDC_JWKS_KEYS + 1],
        }),
        serde_json::json!({}),
    ] {
        assert!(runtime_gateway_oidc_validate_jwks_key_count(&invalid).is_err());
    }
}

#[test]
fn oidc_discovery_policy_rejects_issuer_and_jwks_origin_changes() {
    let config = RuntimeGatewayOidcConfig {
        issuer: "https://idp.example".to_string(),
        audience: "prodex-gateway".to_string(),
        jwks_url: None,
        jwks_origin_allowlist: Vec::new(),
        user_claim: "email".to_string(),
        role_claim: "prodex_role".to_string(),
        tenant_claim: "prodex_tenant".to_string(),
        key_prefixes_claim: "prodex_key_prefixes".to_string(),
    };
    let policy = RuntimeGatewayOidcEndpointPolicy::from_config(&config).unwrap();
    assert!(
        policy
            .validate_discovery_document(&serde_json::json!({
                "issuer": "https://evil.example",
                "jwks_uri": "https://idp.example/jwks.json"
            }))
            .is_err()
    );
    assert!(
        policy
            .validate_discovery_document(&serde_json::json!({
                "issuer": "https://idp.example",
                "jwks_uri": "https://evil.example/jwks.json"
            }))
            .is_err()
    );
}

#[test]
fn oidc_shutdown_wait_is_interruptible_and_bounded() {
    let shutdown = Arc::new(AtomicBool::new(false));
    let worker_shutdown = Arc::clone(&shutdown);
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        runtime_gateway_oidc_wait_for_shutdown(&worker_shutdown, Duration::from_secs(60))
    });
    std::thread::sleep(Duration::from_millis(10));
    shutdown.store(true, Ordering::SeqCst);

    assert!(worker.join().unwrap());
    assert!(started.elapsed() < Duration::from_millis(250));
}

#[test]
fn sso_role_resolution_never_defaults_missing_or_unknown_to_admin() {
    let scim_admin = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };

    assert_eq!(
        runtime_gateway_sso_resolved_role(None, None),
        RuntimeGatewayAdminRole::Viewer
    );
    assert_eq!(
        runtime_gateway_sso_resolved_role(None, Some(&scim_admin)),
        RuntimeGatewayAdminRole::Admin
    );
    assert_eq!(
        runtime_gateway_sso_resolved_role(Some("not-admin"), Some(&scim_admin)),
        RuntimeGatewayAdminRole::Viewer
    );
    assert_eq!(
        runtime_gateway_sso_resolved_role(Some(" admin "), Some(&scim_admin)),
        RuntimeGatewayAdminRole::Viewer
    );
}

#[test]
fn oidc_malformed_role_claim_does_not_fall_back_to_scim_admin() {
    let scim_admin = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };
    let malformed_claims =
        BTreeMap::from([("prodex_role".to_string(), serde_json::json!(" admin "))]);

    assert_eq!(
        runtime_gateway_sso_resolved_role(
            runtime_gateway_oidc_role_claim_string(&malformed_claims, "prodex_role").as_deref(),
            Some(&scim_admin),
        ),
        RuntimeGatewayAdminRole::Viewer
    );

    assert_eq!(
        runtime_gateway_sso_resolved_role(
            runtime_gateway_oidc_role_claim_string(&BTreeMap::new(), "prodex_role").as_deref(),
            Some(&scim_admin),
        ),
        RuntimeGatewayAdminRole::Admin
    );
}

#[test]
fn claimed_tenant_mismatch_does_not_reuse_scim_admin_role() {
    let user = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: vec!["tenant-a-".to_string()],
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };

    let matched =
        runtime_gateway_scim_user_for_claimed_tenant(Some(user.clone()), Some("tenant-a"));
    assert_eq!(
        runtime_gateway_sso_resolved_role(None, matched.as_ref()),
        RuntimeGatewayAdminRole::Admin
    );

    let mismatched = runtime_gateway_scim_user_for_claimed_tenant(Some(user), Some("tenant-b"));
    assert!(mismatched.is_none());
    assert_eq!(
        runtime_gateway_sso_resolved_role(None, mismatched.as_ref()),
        RuntimeGatewayAdminRole::Viewer
    );
}

#[test]
fn missing_tenant_claim_may_still_use_scim_tenant_fallback() {
    let user = RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("viewer".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
        created_at_epoch: 1,
        updated_at_epoch: 1,
    };

    assert!(
        runtime_gateway_scim_user_for_claimed_tenant(Some(user), None)
            .is_some_and(|user| user.tenant_id.as_deref() == Some("tenant-a"))
    );
}

#[test]
fn sso_proxy_token_matches_exactly_without_trimming() {
    let token_hash = runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("sso-proxy-token");

    assert!(runtime_gateway_sso_proxy_token_matches(
        &token_hash,
        "sso-proxy-token"
    ));
    assert!(!runtime_gateway_sso_proxy_token_matches(
        &token_hash,
        " sso-proxy-token "
    ));
}

#[test]
fn sso_user_names_match_exactly_without_trimming() {
    assert_eq!(runtime_gateway_sso_user_name(None), None);
    assert_eq!(
        runtime_gateway_sso_user_name(Some("alice@example.com")),
        Some("alice@example.com")
    );
    assert_eq!(
        runtime_gateway_sso_user_name(Some(" alice@example.com ")),
        None
    );
    assert_eq!(runtime_gateway_sso_user_name(Some("")), None);
}

#[test]
fn oidc_principal_claims_match_exactly_without_trimming() {
    let claims = BTreeMap::from([
        (
            "email".to_string(),
            serde_json::json!(" alice@example.com "),
        ),
        (
            "preferred_username".to_string(),
            serde_json::json!("bob@example.com"),
        ),
    ]);

    assert_eq!(runtime_gateway_oidc_claim_string(&claims, "email"), None);
    assert_eq!(
        runtime_gateway_oidc_claim_string(&claims, "preferred_username"),
        Some("bob@example.com".to_string())
    );
}

#[test]
fn oidc_configured_user_claim_must_be_exact_when_present() {
    let config = RuntimeGatewayOidcConfig {
        issuer: "https://idp.example".to_string(),
        audience: "prodex-gateway".to_string(),
        jwks_url: None,
        jwks_origin_allowlist: Vec::new(),
        user_claim: "prodex_user".to_string(),
        role_claim: "prodex_role".to_string(),
        tenant_claim: "prodex_tenant".to_string(),
        key_prefixes_claim: "prodex_key_prefixes".to_string(),
    };
    let malformed_configured_claim = BTreeMap::from([
        (
            "prodex_user".to_string(),
            serde_json::json!(" alice@example.com "),
        ),
        (
            "email".to_string(),
            serde_json::json!("fallback@example.com"),
        ),
    ]);

    assert_eq!(
        runtime_gateway_oidc_admin_name(&malformed_configured_claim, &config),
        None
    );

    let missing_configured_claim = BTreeMap::from([(
        "email".to_string(),
        serde_json::json!("fallback@example.com"),
    )]);
    assert_eq!(
        runtime_gateway_oidc_admin_name(&missing_configured_claim, &config),
        Some("fallback@example.com".to_string())
    );

    let malformed_first_fallback = BTreeMap::from([
        (
            "email".to_string(),
            serde_json::json!(" fallback@example.com "),
        ),
        (
            "preferred_username".to_string(),
            serde_json::json!("preferred@example.com"),
        ),
    ]);
    assert_eq!(
        runtime_gateway_oidc_admin_name(&malformed_first_fallback, &config),
        None
    );
}

#[test]
fn sso_key_prefixes_reject_empty_or_whitespace_scope_parts() {
    assert_eq!(
        runtime_gateway_parse_sso_prefixes("team-a-,team-b-"),
        Some(vec!["team-a-".to_string(), "team-b-".to_string()])
    );
    assert_eq!(runtime_gateway_parse_sso_prefixes(""), None);
    assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-, team-b-"), None);
    assert_eq!(runtime_gateway_parse_sso_prefixes("team-a-,"), None);
}

#[test]
fn oidc_key_prefix_claim_rejects_whitespace_scope_parts() {
    let claims = BTreeMap::from([(
        "prodex_key_prefixes".to_string(),
        serde_json::json!(["team-a-", " team-b-"]),
    )]);

    assert!(runtime_gateway_oidc_claim_string_vec(&claims, "prodex_key_prefixes").is_err());
}
