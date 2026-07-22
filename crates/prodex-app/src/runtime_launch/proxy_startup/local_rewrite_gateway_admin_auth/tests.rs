use super::super::local_rewrite::RuntimeGatewayOidcHttpCacheEntry;
use super::super::local_rewrite_gateway_store_types::RuntimeGatewayScimUser;
use super::super::*;
use super::admin::*;
use super::cache::*;
use super::endpoint_policy::*;
use super::token_claims::*;
use super::transport::*;
use crate::{AppPaths, RuntimeConfig, read_blocking_response_body_with_limit};
use jsonwebtoken::Algorithm;
use prodex_authn::OidcEndpointPolicy;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod oidc_sso_claims;

const RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV: &str = "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS";
const RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV: &str = "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS";
const RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV: &str =
    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS";
const RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV: &str =
    "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS";

fn assert_oidc_runtime_config_rejected(values: &[(&'static str, &str)], expected: &str) {
    let _lock = crate::TestEnvVarGuard::lock();
    let mut guards = [
        RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV,
        RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV,
        RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV,
        RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV,
    ]
    .into_iter()
    .map(crate::TestEnvVarGuard::unset)
    .collect::<Vec<_>>();
    guards.extend(
        values
            .iter()
            .map(|(key, value)| crate::TestEnvVarGuard::set(key, value)),
    );
    let root = std::env::temp_dir().join(format!(
        "prodex-oidc-runtime-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    std::fs::create_dir_all(&root).unwrap();
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root: root.clone(),
    };

    let rendered = RuntimeConfig::from_env_policy_and_cli(&paths)
        .unwrap_err()
        .to_string();

    assert_eq!(
        rendered,
        format!("runtime configuration is invalid; {expected}")
    );
    for (_, value) in values {
        assert!(!rendered.contains(value), "{rendered}");
    }
    let _ = std::fs::remove_dir_all(root);
}

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
    for (value, message) in [
        ("0", "must be greater than zero"),
        (
            "oidc-prefetch-secret-sentinel",
            "must be an unsigned integer",
        ),
    ] {
        assert_oidc_runtime_config_rejected(
            &[(RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, value)],
            &format!("{RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV} {message}"),
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

    assert_oidc_runtime_config_rejected(
        &[(
            RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV,
            "oidc-cache-secret-sentinel",
        )],
        &format!("{RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV} must be an unsigned integer"),
    );
}

#[test]
fn oidc_background_refresh_uses_the_authn_planner_and_origin_allowlist() {
    let config = RuntimeGatewayOidcConfig {
        issuer: "https://issuer.example.com/tenant".to_string(),
        audience: "prodex".to_string(),
        jwks_url: Some("https://keys.example.com/jwks.json".to_string()),
        jwks_origin_allowlist: vec!["https://keys.example.com".to_string()],
        user_claim: "sub".to_string(),
        role_claim: "role".to_string(),
        tenant_claim: "tenant_id".to_string(),
        key_prefixes_claim: "key_prefixes".to_string(),
        authentication_strength: None,
        reauthentication_max_age_seconds: None,
    };
    let now = Instant::now();
    let now_unix_ms = 1_000_000;
    let fresh = RuntimeGatewayOidcJwksSnapshot {
        jwks: jsonwebtoken::jwk::JwkSet {
            keys: vec![jsonwebtoken::jwk::Jwk {
                common: Default::default(),
                algorithm: jsonwebtoken::jwk::AlgorithmParameters::RSA(
                    jsonwebtoken::jwk::RSAKeyParameters {
                        n: "AQAB".to_string(),
                        e: "AQAB".to_string(),
                        ..Default::default()
                    },
                ),
            }],
        },
        fetched_at: now,
        fresh_for: Duration::from_secs(60),
        stale_for: Duration::from_secs(60),
    };
    let expired = RuntimeGatewayOidcJwksSnapshot {
        fetched_at: now - Duration::from_secs(121),
        ..fresh
    };

    assert!(
        !runtime_gateway_oidc_refresh_plan_due(
            &config,
            Some(&expired),
            expired.fetched_at + Duration::from_secs(1),
            now_unix_ms,
        )
        .unwrap()
    );
    assert!(
        runtime_gateway_oidc_refresh_plan_due(&config, Some(&expired), now, now_unix_ms,).unwrap()
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
    for (value, message) in [
        ("0", "must be greater than zero"),
        (
            "oidc-backoff-secret-sentinel",
            "must be an unsigned integer",
        ),
    ] {
        assert_oidc_runtime_config_rejected(
            &[(RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, value)],
            &format!("{RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV} {message}"),
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

    assert_oidc_runtime_config_rejected(
        &[(
            RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV,
            "oidc-lkg-secret-sentinel",
        )],
        &format!("{RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV} must be an unsigned integer"),
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
fn oidc_domain_snapshot_preserves_immutable_cache_deadlines() {
    let fetched_at = Instant::now();
    let runtime = RuntimeGatewayOidcJwksSnapshot {
        jwks: jsonwebtoken::jwk::JwkSet { keys: Vec::new() },
        fetched_at,
        fresh_for: Duration::from_secs(10),
        stale_for: Duration::from_secs(20),
    };
    let snapshot = runtime.domain_snapshot_at(fetched_at + Duration::from_secs(5), 100_000);

    assert_eq!(snapshot.fetched_at_unix_ms, 95_000);
    assert_eq!(snapshot.expires_at_unix_ms, 105_000);
    assert_eq!(snapshot.stale_until_unix_ms, 125_000);
    assert_eq!(snapshot.key_count, 0);
}

#[test]
fn oidc_runtime_config_rejects_overflow_and_cache_headers_clamp() {
    let overflow = u64::MAX.to_string();
    assert_oidc_runtime_config_rejected(
        &[
            (RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV, &overflow),
            (RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV, &overflow),
            (RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV, &overflow),
            (RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV, &overflow),
        ],
        &format!(
            "{RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_ENV} must not exceed maximum; \
             {RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_ENV} must not exceed maximum; \
             {RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_ENV} must not exceed maximum; \
             {RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_ENV} must not exceed maximum"
        ),
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_max_age(Some(&format!("max-age={}", u64::MAX))),
        Some(Duration::from_secs(
            MAX_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS
        ))
    );
    assert_eq!(
        runtime_gateway_oidc_cache_control_stale_while_revalidate(Some(&format!(
            "stale-while-revalidate={}",
            u64::MAX
        ))),
        Some(Duration::from_secs(
            MAX_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS
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
    for (algorithm, domain) in [
        (Algorithm::RS256, prodex_domain::JwtAlgorithm::Rs256),
        (Algorithm::RS384, prodex_domain::JwtAlgorithm::Rs384),
        (Algorithm::RS512, prodex_domain::JwtAlgorithm::Rs512),
        (Algorithm::ES256, prodex_domain::JwtAlgorithm::Es256),
        (Algorithm::ES384, prodex_domain::JwtAlgorithm::Es384),
    ] {
        assert_eq!(
            runtime_gateway_domain_oidc_algorithm(algorithm).unwrap(),
            domain
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
        authentication_strength: None,
        reauthentication_max_age_seconds: None,
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
