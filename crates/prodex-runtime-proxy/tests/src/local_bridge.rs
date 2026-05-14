use super::*;

#[test]
fn classifies_only_bridge_paths_and_methods() {
    let health = local_bridge_classify_request("GET", "/health?ready=1").unwrap();
    assert_eq!(health.route, LocalBridgeRoute::Health);
    assert_eq!(health.method, "GET");
    assert_eq!(health.path, "/health");

    let models = local_bridge_classify_request("head", "/v1/models").unwrap();
    assert_eq!(models.route, LocalBridgeRoute::Models);
    assert_eq!(models.method, "HEAD");

    let responses = local_bridge_classify_request("post", "/v1/responses").unwrap();
    assert_eq!(responses.route, LocalBridgeRoute::Responses);
    assert_eq!(responses.method, "POST");

    assert_eq!(
        local_bridge_classify_request("POST", "/v1/models").unwrap_err(),
        LocalBridgeRequestRejection::MethodNotAllowed
    );
    assert_eq!(
        local_bridge_classify_request("GET", "/v1/responses/extra").unwrap_err(),
        LocalBridgeRequestRejection::PathNotFound
    );
}

#[test]
fn verifies_bearer_tokens_by_hash() {
    let record = LocalBridgeBearerTokenHash::from_token("secret-token");

    assert_eq!(record.algorithm(), "sha256");
    assert_eq!(record.hash_bytes().len(), 32);
    assert_eq!(
        record.hash_base64(),
        "kwu9xRtq7VwqVnj9bije56BeiktkPPwLRCfD77hsDZQ="
    );
    assert!(record.verify_bearer_token("secret-token"));
    assert!(record.verify_authorization_header("Bearer secret-token"));
    assert!(record.verify_authorization_header("bearer   secret-token"));
    assert!(!record.verify_authorization_header("Bearer wrong-token"));
    assert!(!record.verify_authorization_header("Basic secret-token"));
    assert!(!record.verify_authorization_header("Bearer secret token"));
}

#[test]
fn filters_hop_by_hop_and_decoded_content_headers() {
    let headers = [
        ("Content-Type", "application/json"),
        ("Connection", "keep-alive, X-Hop"),
        ("Keep-Alive", "timeout=5"),
        ("Transfer-Encoding", "chunked"),
        ("Upgrade", "websocket"),
        ("Content-Encoding", "gzip"),
        ("Content-Length", "123"),
        ("X-Hop", "strip-me"),
        ("X-Trace-Id", "abc"),
    ];

    assert_eq!(
        local_bridge_filter_text_response_headers(
            headers,
            LocalBridgeHeaderFilter::FOR_ENCODED_BODY
        ),
        vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Encoding".to_string(), "gzip".to_string()),
            ("Content-Length".to_string(), "123".to_string()),
            ("X-Trace-Id".to_string(), "abc".to_string()),
        ]
    );

    assert_eq!(
        local_bridge_filter_text_response_headers(
            headers,
            LocalBridgeHeaderFilter::FOR_DECODED_BODY
        ),
        vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Trace-Id".to_string(), "abc".to_string()),
        ]
    );
}

#[test]
fn validates_loopback_hosts_only() {
    for host in [
        "localhost",
        "LOCALHOST:8080",
        "127.0.0.1",
        "127.0.0.1:9090",
        "::1",
        "[::1]",
        "[::1]:9090",
    ] {
        assert!(local_bridge_host_is_loopback(host), "{host}");
    }

    for host in [
        "",
        "example.com",
        "192.168.1.10",
        "10.0.0.1:8080",
        "[2001:db8::1]:8080",
        "localhost@example.com",
        "localhost/path",
        "[::1]bad",
        "127.0.0.1:notaport",
    ] {
        assert!(!local_bridge_host_is_loopback(host), "{host}");
    }
}
