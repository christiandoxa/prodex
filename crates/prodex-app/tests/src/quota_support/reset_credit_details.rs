use super::*;
use prodex_quota::RateLimitResetCreditExpiry;
use std::sync::mpsc;
use tiny_http::{Response, Server, StatusCode};

const FINITE_DETAILS: &str = r#"{
    "credits": [
        {"status":"consumed","expires_at":"2026-10-20T00:00:00Z"},
        {"status":"available","expires_at":"2026-10-25T00:00:00Z"},
        {"status":"available","expires_at":"2026-10-23T01:23:45+07:00"},
        {"status":"available","expires_at":null}
    ]
}"#;

fn usage_fixture(count: Option<i64>) -> serde_json::Value {
    let mut usage = serde_json::json!({
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {"used_percent": 20, "reset_at": 1792550000}
        }
    });
    if let Some(count) = count {
        usage["rate_limit_reset_credits"] = serde_json::json!({"available_count": count});
    }
    usage
}

// Exercise the real GET adapter with synthetic profile credentials and bounded
// loopback requests. Record every request, including an unexpected detail GET.
fn with_fixture<T>(
    usage: serde_json::Value,
    detail: (u16, &'static str),
    fetch: impl FnOnce(UsageFetchFlow<'_>) -> Result<T>,
) -> (T, Vec<String>) {
    let root = temp_dir("reset-credit-details");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("auth.json"),
        r#"{"tokens":{"access_token":"different-disk-token","account_id":"different-account"}}"#,
    )
    .unwrap();
    fs::write(root.join("config.toml"), "enforce_residency = 'us'\n").unwrap();

    let server = Server::http("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/backend-api", server.server_addr());
    let flow = UsageFetchFlow {
        codex_home: &root,
        usage_url: prodex_quota::usage_url(&base_url),
        reset_credits_url: prodex_quota::rate_limit_reset_credits_url(&base_url),
        client: Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        auth: UsageAuth {
            access_token: "accepted-fixture-token".to_string(),
            account_id: Some("accepted-fixture-account".to_string()),
            refresh_token: None,
            expires_at: None,
            last_refresh: None,
        },
        upstream_no_proxy: true,
    };
    let (stop, stopped) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        loop {
            if stopped.try_recv().is_ok() {
                break;
            }
            let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() else {
                continue;
            };
            let mut recorded = format!("{} {}", request.method(), request.url());
            for header in request.headers() {
                recorded.push_str(&format!("\n{}: {}", header.field, header.value));
            }
            let (status, body) = match request.url() {
                "/backend-api/wham/usage" => (200, usage.to_string()),
                "/backend-api/wham/rate-limit-reset-credits" => (detail.0, detail.1.to_string()),
                _ => (404, "{}".to_string()),
            };
            requests.push(recorded);
            request
                .respond(Response::from_string(body).with_status_code(StatusCode(status)))
                .unwrap();
        }
        requests
    });
    let result = fetch(flow);
    stop.send(()).unwrap();
    let requests = worker.join().unwrap();
    fs::remove_dir_all(root).unwrap();
    (result.unwrap(), requests)
}

#[test]
fn reset_credit_details_keep_the_usage_identity_and_read_only_profile_headers() {
    let (usage, requests) = with_fixture(usage_fixture(Some(3)), (200, FINITE_DETAILS), |flow| {
        flow.execute_for_display()
    });
    let credits = usage.rate_limit_reset_credits.unwrap();
    assert_eq!(credits.available_count, 3);
    assert_eq!(
        credits.next_expiry,
        RateLimitResetCreditExpiry::At(
            chrono::DateTime::parse_from_rfc3339("2026-10-23T01:23:45+07:00")
                .unwrap()
                .timestamp()
        )
    );
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /backend-api/wham/usage\n"));
    assert!(requests[1].starts_with("GET /backend-api/wham/rate-limit-reset-credits\n"));
    for request in requests {
        let headers = request.to_ascii_lowercase();
        assert!(headers.contains("authorization: bearer accepted-fixture-token"));
        assert!(headers.contains("chatgpt-account-id: accepted-fixture-account"));
        assert!(headers.contains("x-openai-internal-codex-residency: us"));
        assert!(!headers.contains("different-disk-token"));
        assert!(!headers.contains("different-account"));
    }
}

#[test]
fn reset_credit_details_are_not_requested_without_positive_credit_count() {
    for count in [None, Some(0), Some(-1)] {
        let (usage, requests) = with_fixture(usage_fixture(count), (200, FINITE_DETAILS), |flow| {
            flow.execute_for_display()
        });
        assert_eq!(requests.len(), 1, "{count:?}: {requests:?}");
        assert_eq!(
            usage
                .rate_limit_reset_credits
                .as_ref()
                .map(|credits| credits.available_count),
            count
        );
        if let Some(credits) = usage.rate_limit_reset_credits {
            assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Unavailable);
        }
    }
}

#[test]
fn reset_credit_details_failure_preserves_a_successful_usage_snapshot() {
    for detail in [
        (401, "{}"),
        (403, "{}"),
        (500, "{}"),
        (200, "invalid json"),
        (
            200,
            r#"{"credits":[{"status":"available","expires_at":"invalid"}]}"#,
        ),
        (200, r#"{"credits":[{"status":"available"}]}"#),
    ] {
        let (usage, requests) = with_fixture(usage_fixture(Some(3)), detail, |flow| {
            flow.execute_for_display()
        });
        assert_eq!(usage.plan_type.as_deref(), Some("plus"));
        assert_eq!(
            usage
                .rate_limit
                .unwrap()
                .primary_window
                .unwrap()
                .used_percent,
            Some(20)
        );
        let credits = usage.rate_limit_reset_credits.unwrap();
        assert_eq!(credits.available_count, 3);
        assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Unavailable);
        assert_eq!(requests.len(), 2, "{detail:?}");
    }
}

#[test]
fn reset_credit_details_transport_failure_is_optional() {
    let (usage, requests) =
        with_fixture(usage_fixture(Some(3)), (200, FINITE_DETAILS), |mut flow| {
            flow.reset_credits_url = "http://127.0.0.1:0/rate-limit-reset-credits".to_string();
            flow.execute_for_display()
        });
    let credits = usage.rate_limit_reset_credits.unwrap();
    assert_eq!(credits.available_count, 3);
    assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Unavailable);
    assert_eq!(requests.len(), 1);
}

#[test]
fn reset_credit_details_null_expiry_is_distinct_from_unavailable() {
    let (usage, requests) = with_fixture(
        usage_fixture(Some(1)),
        (
            200,
            r#"{"credits":[{"status":"available","expires_at":null}]}"#,
        ),
        |flow| flow.execute_for_display(),
    );
    let credits = usage.rate_limit_reset_credits.unwrap();
    assert_eq!(credits.available_count, 1);
    assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Never);
    assert_eq!(requests.len(), 2);
}

#[test]
fn reset_credit_details_do_not_change_raw_or_runtime_probe_contracts() {
    let expected = usage_fixture(Some(3));
    let (raw, requests) = with_fixture(expected.clone(), (200, FINITE_DETAILS), |flow| {
        flow.execute()
    });
    assert_eq!(raw, expected);
    assert_eq!(requests.len(), 1);

    let ((value, auth), requests) = with_fixture(expected.clone(), (200, FINITE_DETAILS), |flow| {
        flow.execute_with_authenticated_identity()
    });
    assert_eq!(value, expected);
    assert_eq!(auth.account_id.as_deref(), Some("accepted-fixture-account"));
    assert_eq!(requests.len(), 1);
}
