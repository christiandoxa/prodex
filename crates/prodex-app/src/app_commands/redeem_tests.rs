use super::*;

#[test]
fn manual_redeem_request_id_is_prodex_scoped() {
    let id = manual_redeem_request_id();
    let uuid = id
        .strip_prefix("prodex-manual-redeem-")
        .expect("manual redeem id should be prodex scoped");

    assert_eq!(
        uuid.parse::<RequestId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
}

#[test]
fn manual_redeem_outcome_labels_are_human_readable() {
    assert_eq!(
        manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::Reset),
        "reset"
    );
    assert_eq!(
        manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::NothingToReset),
        "nothing-to-reset"
    );
    assert_eq!(
        manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::NoCredit),
        "no-credit"
    );
    assert_eq!(
        manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::AlreadyRedeemed),
        "already-redeemed"
    );
}

fn usage_with_resets(five_hour_reset_at: i64, weekly_reset_at: i64) -> UsageResponse {
    serde_json::from_value(serde_json::json!({
        "rate_limit": {
            "primary_window": {
                "used_percent": 20,
                "reset_at": five_hour_reset_at,
                "limit_window_seconds": 18000
            },
            "secondary_window": {
                "used_percent": 30,
                "reset_at": weekly_reset_at,
                "limit_window_seconds": 604800
            }
        }
    }))
    .expect("usage response should parse")
}

#[test]
fn manual_redeem_allows_remaining_quota_when_reset_is_not_near() {
    let usage = usage_with_resets(1_000 + MANUAL_REDEEM_NEAR_RESET_SECONDS + 1, 604_800);
    assert_eq!(
        nearest_manual_redeem_reset(&usage, 1_000, MANUAL_REDEEM_NEAR_RESET_SECONDS),
        None
    );
}

#[test]
fn manual_redeem_detects_nearest_near_reset() {
    let usage = usage_with_resets(1_060, 1_030);
    assert_eq!(
        nearest_manual_redeem_reset(&usage, 1_000, MANUAL_REDEEM_NEAR_RESET_SECONDS),
        Some(ManualRedeemNearReset {
            label: "weekly",
            reset_at: 1_030
        })
    );
}

fn read_redeem_test_request(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut request = Vec::new();
    let mut content_length = None;
    loop {
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).expect("request read");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        if content_length.is_none() {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            content_length = Some(
                headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0),
            );
        }
        if request.len() >= header_end + 4 + content_length.unwrap_or(0) {
            break;
        }
    }
    String::from_utf8(request).expect("request utf8")
}

#[test]
fn manual_redeem_runs_usage_then_consume_against_selected_profile() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-manual-redeem-e2e-{}-{nonce}",
        std::process::id()
    ));
    let prodex_home = root.join("prodex-home");
    let codex_home = prodex_home.join("profiles/main");
    secret_store::ensure_private_directory(&root).expect("private test root");
    secret_store::ensure_private_directory(&prodex_home).expect("private prodex home");
    let _prodex_home = crate::TestEnvVarGuard::set(
        "PRODEX_HOME",
        prodex_home.to_str().expect("utf8 prodex home"),
    );
    let paths = AppPaths::discover().expect("paths");
    let mut state = AppState {
        active_profile: Some("main".to_string()),
        profiles: std::collections::BTreeMap::from([(
            "main".to_string(),
            crate::ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    crate::profile_commands::update_existing_profile_auth(
        &paths,
        &mut state,
        "main",
        Some("redeem@example.com"),
        &serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "manual-redeem-test-token",
                "account_id": "manual-redeem-account"
            }
        })
        .to_string(),
        false,
    )
    .expect("profile auth write");
    state.save(&paths).expect("state save");

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_redeem_test_request(&mut stream);
            requests.push(request);
            let body = if index == 0 {
                serde_json::json!({
                    "email": "redeem@example.com",
                    "plan_type": "plus",
                    "rate_limit": {
                        "primary_window": {
                            "used_percent": 100,
                            "reset_at": 4_102_444_800_i64,
                            "limit_window_seconds": 18_000
                        },
                        "secondary_window": {
                            "used_percent": 100,
                            "reset_at": 4_102_444_800_i64,
                            "limit_window_seconds": 604_800
                        }
                    }
                })
                .to_string()
            } else {
                serde_json::json!({"outcome": "reset"}).to_string()
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("response");
        }
        requests
    });

    handle_redeem(RedeemArgs {
        profile: "main".to_string(),
        yes: true,
        base_url: Some(format!("http://{address}")),
        no_proxy: true,
    })
    .expect("manual redeem");

    let requests = server.join().expect("server");
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /api/codex/usage HTTP/1.1\r\n"));
    assert!(
        requests[1].starts_with("POST /api/codex/rate-limit-reset-credits/consume HTTP/1.1\r\n")
    );
    assert!(requests[1].contains("prodex-manual-redeem-"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manual_redeem_confirmation_defaults_to_no() {
    assert_eq!(parse_manual_redeem_confirmation(""), Some(false));
    assert_eq!(parse_manual_redeem_confirmation("no"), Some(false));
    assert_eq!(parse_manual_redeem_confirmation("yes"), Some(true));
    assert_eq!(parse_manual_redeem_confirmation("wat"), None);
}
