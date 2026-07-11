use super::*;
use std::net::{Shutdown, SocketAddr};

fn expose_test_tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    (client, server)
}

fn expose_parse_test_request(
    raw: &[u8],
) -> std::result::Result<ExposeParsedRequest, ExposeHttpParseError> {
    let (mut client, mut server) = expose_test_tcp_pair();
    client.write_all(raw).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    expose_read_http_request(&mut server)
}

fn expose_start_test_server(
    bootstrap: &str,
    max_clients: u16,
) -> (SocketAddr, Arc<ExposeShared>, ExposeHttpServer) {
    let args = ExposeArgs {
        command: Some("sleep 30".to_string()),
        cols: 80,
        rows: 24,
        max_clients,
        tunnel: false,
        no_tunnel: false,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let listen_addr = listener.local_addr().unwrap();
    let shared = Arc::new(ExposeShared {
        sessions: Mutex::new(ExposeSessionStore::new(
            bootstrap,
            usize::from(max_clients),
            Instant::now(),
        )),
        allowed_hosts: Mutex::new(BTreeSet::from([listen_addr.to_string()])),
        pty: ExposePty::spawn(&args).unwrap(),
        shutdown: Arc::new(AtomicBool::new(false)),
        active_clients: AtomicUsize::new(0),
        active_requests: AtomicUsize::new(0),
        peak_requests: AtomicUsize::new(0),
        next_client_id: AtomicU64::new(1),
        max_clients: usize::from(max_clients),
    });
    let server = ExposeHttpServer::start(listener, Arc::clone(&shared)).unwrap();
    (listen_addr, shared, server)
}

fn expose_send_test_request(listen_addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(listen_addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn expose_test_cookie(response: &str) -> String {
    response
        .split("\r\n")
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .and_then(|value| value.split(';').next())
        .unwrap()
        .to_string()
}

fn expose_test_csrf(response: &str) -> String {
    response
        .split_once("\r\n\r\n")
        .unwrap()
        .1
        .strip_prefix("{\"csrf\":\"")
        .and_then(|value| value.strip_suffix("\"}"))
        .unwrap()
        .to_string()
}

#[test]
fn expose_access_url_uses_one_time_fragment_without_path_or_query_secret() {
    let url = expose_access_url("http://127.0.0.1:7777", "bootstrap-capability");
    let (request_target, fragment) = url.split_once('#').unwrap();

    assert_eq!(request_target, "http://127.0.0.1:7777/expose");
    assert_eq!(fragment, "bootstrap=bootstrap-capability");
    assert!(!request_target.contains("bootstrap-capability"));
    assert!(!url.contains("access_token"));
}

#[test]
fn expose_bootstrap_is_single_use_and_expires() {
    let now = Instant::now();
    let mut sessions = ExposeSessionStore::new("bootstrap-capability", 2, now);
    assert!(
        sessions
            .exchange_bootstrap("bootstrap-capability", now)
            .is_ok()
    );
    assert!(matches!(
        sessions.exchange_bootstrap("bootstrap-capability", now),
        Err(ExposeSessionError::Unauthorized)
    ));

    let mut expired = ExposeSessionStore::new("expired-bootstrap", 2, now);
    assert!(matches!(
        expired.exchange_bootstrap("expired-bootstrap", now + EXPOSE_BOOTSTRAP_TTL),
        Err(ExposeSessionError::Unauthorized)
    ));
}

#[test]
fn expose_session_rotation_revokes_old_cookie_and_revoke_removes_new_cookie() {
    let now = Instant::now();
    let mut sessions = ExposeSessionStore::new("bootstrap-capability", 2, now);
    let issued = sessions
        .exchange_bootstrap("bootstrap-capability", now)
        .unwrap();
    let rotated = sessions.rotate(&issued.session, &issued.csrf, now).unwrap();

    assert!(!sessions.authenticate(&issued.session, Some(&issued.csrf), now, false));
    assert!(sessions.authenticate(&rotated.session, Some(&rotated.csrf), now, false));
    assert!(sessions.revoke(&rotated.session, &rotated.csrf, now));
    assert!(!sessions.authenticate(&rotated.session, None, now, false));
}

#[test]
fn expose_session_idle_timeout_and_input_rate_are_enforced() {
    let now = Instant::now();
    let mut sessions = ExposeSessionStore::new("bootstrap-capability", 1, now);
    let issued = sessions
        .exchange_bootstrap("bootstrap-capability", now)
        .unwrap();
    for _ in 0..EXPOSE_INPUT_RATE_REQUESTS {
        sessions
            .admit_input(&issued.session, &issued.csrf, 1, now)
            .unwrap();
    }
    assert_eq!(
        sessions.admit_input(&issued.session, &issued.csrf, 1, now),
        Err(ExposeSessionError::RateLimited)
    );
    assert!(!sessions.authenticate(
        &issued.session,
        None,
        now + EXPOSE_SESSION_IDLE_TIMEOUT,
        false,
    ));
}

#[test]
fn expose_session_debug_redacts_capabilities() {
    let sessions =
        ExposeSessionStore::new("bootstrap-capability-that-must-not-leak", 1, Instant::now());
    let debug = format!("{sessions:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("bootstrap-capability-that-must-not-leak"));
}

#[test]
fn expose_origin_requires_exact_host_and_reports_https() {
    assert_eq!(
        expose_origin_matches_host("127.0.0.1:7777", "http://127.0.0.1:7777"),
        Some(false)
    );
    assert_eq!(
        expose_origin_matches_host("shell.trycloudflare.com", "https://shell.trycloudflare.com"),
        Some(true)
    );
    assert_eq!(
        expose_origin_matches_host("127.0.0.1:7777", "https://attacker.example.com"),
        None
    );
    assert_eq!(expose_origin_matches_host("127.0.0.1:7777", ""), None);
    assert_eq!(expose_origin_policy(Some("127.0.0.1:7777"), None), None);
    assert_eq!(
        expose_origin_policy(None, Some("http://127.0.0.1:7777")),
        None
    );
}

#[test]
fn expose_cookie_is_http_only_strict_narrow_and_secure_for_https() {
    let local = expose_session_cookie_header("session-capability", false);
    let remote = expose_session_cookie_header("session-capability", true);

    assert!(local.contains("HttpOnly"));
    assert!(local.contains("SameSite=Strict"));
    assert!(local.contains("Path=/expose"));
    assert!(!local.contains("Secure"));
    assert!(remote.contains("; Secure"));
}

#[test]
fn expose_broadcast_drops_clients_with_full_output_queues() {
    let scrollback = Arc::new(Mutex::new(VecDeque::new()));
    let (sender, receiver) = mpsc::sync_channel(1);
    let clients = Arc::new(Mutex::new(vec![ExposeOutputClient { id: 1, sender }]));

    expose_broadcast_output(&scrollback, &clients, b"first");
    expose_broadcast_output(&scrollback, &clients, b"second");

    assert!(clients.lock().unwrap().is_empty());
    assert_eq!(receiver.recv().unwrap(), b"first");
    assert!(receiver.recv().is_err());
}

#[test]
fn expose_worker_and_queue_bounds_are_fixed() {
    assert_eq!(expose_worker_count(1), 1 + EXPOSE_SHORT_REQUEST_WORKERS);
    assert_eq!(
        expose_worker_count(usize::MAX),
        EXPOSE_MAX_CLIENTS_LIMIT + EXPOSE_SHORT_REQUEST_WORKERS
    );
    assert_eq!(EXPOSE_REQUEST_QUEUE_CAPACITY, 64);
    assert_eq!(EXPOSE_CLIENT_QUEUE_CAPACITY, 64);

    let active = AtomicUsize::new(0);
    assert!(expose_try_acquire(&active, 1));
    assert!(!expose_try_acquire(&active, 1));
    active.fetch_sub(1, Ordering::SeqCst);
    assert!(expose_try_acquire(&active, 1));

    let (sender, _receiver) = mpsc::sync_channel(EXPOSE_REQUEST_QUEUE_CAPACITY);
    for _ in 0..EXPOSE_REQUEST_QUEUE_CAPACITY {
        sender.try_send(()).unwrap();
    }
    assert!(matches!(sender.try_send(()), Err(TrySendError::Full(()))));
}

#[test]
fn expose_http_parser_bounds_and_rejects_ambiguous_framing() {
    let parsed = expose_parse_test_request(
        b"POST /expose/input HTTP/1.1\r\nHost: 127.0.0.1:7777\r\nContent-Length: 5\r\n\r\ninput",
    )
    .unwrap();
    assert_eq!(parsed.method, "POST");
    assert_eq!(parsed.target, "/expose/input");
    assert_eq!(parsed.body, b"input");

    for request in [
            b"POST /expose/input HTTP/1.1\r\nHost: 127.0.0.1:7777\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx".as_slice(),
            b"POST /expose/input HTTP/1.1\r\nHost: 127.0.0.1:7777\r\nTransfer-Encoding: chunked\r\n\r\n".as_slice(),
            b"GET http://attacker.example.com/expose HTTP/1.1\r\nHost: 127.0.0.1:7777\r\n\r\n".as_slice(),
            b"GET /expose HTTP/1.1\r\n Host: 127.0.0.1:7777\r\n\r\n".as_slice(),
            b"GET /expose HTTP/1.1\r\nHost: 127.0.0.1:7777\r\n\r\nsmuggled".as_slice(),
        ] {
            assert!(expose_parse_test_request(request).is_err());
        }

    let oversized = format!(
        "POST /expose/input HTTP/1.1\r\nHost: 127.0.0.1:7777\r\nContent-Length: {}\r\n\r\n",
        EXPOSE_MAX_INPUT_BYTES + 1
    );
    let error = match expose_parse_test_request(oversized.as_bytes()) {
        Ok(_) => panic!("oversized body should fail"),
        Err(error) => error,
    };
    assert_eq!(error.status, 413);
}

#[test]
fn expose_connection_flood_keeps_fixed_worker_count() {
    let (listen_addr, shared, mut server) =
        expose_start_test_server("bootstrap_capability_for_expose_test_0001", 1);
    let worker_count = expose_worker_count(1);
    assert_eq!(server.worker_threads.len(), worker_count);

    let mut clients = Vec::new();
    for _ in 0..(worker_count + EXPOSE_REQUEST_QUEUE_CAPACITY + 24) {
        if let Ok(client) = TcpStream::connect(listen_addr) {
            clients.push(client);
        }
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    while shared.peak_requests.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(shared.peak_requests.load(Ordering::SeqCst) > 0);
    assert!(shared.peak_requests.load(Ordering::SeqCst) <= worker_count);

    drop(clients);
    server.shutdown();
    shared.pty.shutdown();
    assert_eq!(shared.active_requests.load(Ordering::SeqCst), 0);
}

#[test]
fn expose_http_session_flow_fails_closed_and_revokes_rotated_session() {
    let bootstrap = "bootstrap_capability_for_expose_test_0001";
    let (listen_addr, shared, mut server) = expose_start_test_server(bootstrap, 1);
    let host = listen_addr.to_string();

    let missing_origin = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/session HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {bootstrap}\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(missing_origin.starts_with("HTTP/1.1 403"));

    let cross_origin = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/session HTTP/1.1\r\nHost: {host}\r\nOrigin: https://attacker.example.com\r\nAuthorization: Bearer {bootstrap}\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(cross_origin.starts_with("HTTP/1.1 403"));

    let authorized = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/session HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nAuthorization: Bearer {bootstrap}\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(authorized.starts_with("HTTP/1.1 201"));
    let cookie = expose_test_cookie(&authorized);
    let csrf = expose_test_csrf(&authorized);

    let replay = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/session HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nAuthorization: Bearer {bootstrap}\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(replay.starts_with("HTTP/1.1 401"));

    let mut first_stream = TcpStream::connect(listen_addr).unwrap();
    first_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    first_stream
        .write_all(
            format!("GET /expose/stream HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\n\r\n")
                .as_bytes(),
        )
        .unwrap();
    let mut stream_headers = [0_u8; 512];
    let stream_header_len = first_stream.read(&mut stream_headers).unwrap();
    assert!(stream_headers[..stream_header_len].starts_with(b"HTTP/1.1 200"));
    let second_stream = expose_send_test_request(
        listen_addr,
        &format!("GET /expose/stream HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\n\r\n"),
    );
    assert!(second_stream.starts_with("HTTP/1.1 429"));
    drop(first_stream);

    for origin in [None, Some("https://attacker.example.com")] {
        let origin = origin
            .map(|origin| format!("Origin: {origin}\r\n"))
            .unwrap_or_default();
        let rejected = expose_send_test_request(
            listen_addr,
            &format!(
                "POST /expose/input HTTP/1.1\r\nHost: {host}\r\n{origin}Cookie: {cookie}\r\nX-Prodex-CSRF: {csrf}\r\nContent-Type: application/octet-stream\r\nContent-Length: 1\r\n\r\nx"
            ),
        );
        assert!(rejected.starts_with("HTTP/1.1 403"));
    }

    let malformed_input = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/input HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\nX-Prodex-CSRF: {csrf}\r\nContent-Type: text/plain\r\nContent-Length: 1\r\n\r\nx"
        ),
    );
    assert!(malformed_input.starts_with("HTTP/1.1 415"));

    let empty_input = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/input HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\nX-Prodex-CSRF: {csrf}\r\nContent-Type: application/octet-stream\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(empty_input.starts_with("HTTP/1.1 400"));

    let input = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/input HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\nX-Prodex-CSRF: {csrf}\r\nContent-Type: application/octet-stream\r\nContent-Length: 1\r\n\r\nx"
        ),
    );
    assert!(input.starts_with("HTTP/1.1 204"));

    let rotated = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/session/rotate HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\nX-Prodex-CSRF: {csrf}\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(rotated.starts_with("HTTP/1.1 201"));
    let rotated_cookie = expose_test_cookie(&rotated);
    let rotated_csrf = expose_test_csrf(&rotated);

    let old_session = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/input HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\nX-Prodex-CSRF: {csrf}\r\nContent-Type: application/octet-stream\r\nContent-Length: 1\r\n\r\nx"
        ),
    );
    assert!(old_session.starts_with("HTTP/1.1 401"));

    let revoked = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/session/revoke HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {rotated_cookie}\r\nX-Prodex-CSRF: {rotated_csrf}\r\nContent-Length: 0\r\n\r\n"
        ),
    );
    assert!(revoked.starts_with("HTTP/1.1 204"));

    let after_revoke = expose_send_test_request(
        listen_addr,
        &format!(
            "POST /expose/input HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {rotated_cookie}\r\nX-Prodex-CSRF: {rotated_csrf}\r\nContent-Type: application/octet-stream\r\nContent-Length: 1\r\n\r\nx"
        ),
    );
    assert!(after_revoke.starts_with("HTTP/1.1 401"));

    server.shutdown();
    shared.pty.shutdown();
}

#[test]
fn expose_input_rejects_empty_and_oversized_batches() {
    let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(io::sink())));

    assert_eq!(
        expose_write_input(&writer, b"").unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        expose_write_input(&writer, &vec![0; EXPOSE_MAX_INPUT_BYTES + 1])
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    expose_write_input(&writer, b"batched input").unwrap();
}

#[test]
fn expose_status_has_no_separate_long_lived_bearer() {
    let fields = expose_status_fields(
        "http://127.0.0.1:7777/expose#bootstrap=one-time-capability",
        1,
        "ready",
        Some("https://shell.trycloudflare.com/expose#bootstrap=one-time-capability"),
        true,
    );

    assert!(!fields.iter().any(|(label, _)| label.contains("token")));
    assert!(
        !fields
            .iter()
            .any(|(_, value)| value.contains("access_token"))
    );
    assert!(fields.iter().all(|(label, value)| {
        !value.contains("one-time-capability") || label.starts_with("One-time ")
    }));
    assert!(fields.iter().any(|(label, _)| label == "WARNING"));
}

#[test]
fn cloudflared_url_parser_extracts_quick_tunnel_url() {
    let line = "2026 INF tunnel ready https://shell.trycloudflare.com";
    assert_eq!(
        expose_find_trycloudflare_url(line).as_deref(),
        Some("https://shell.trycloudflare.com")
    );
    assert_eq!(
        expose_public_host("https://shell.trycloudflare.com"),
        Some("shell.trycloudflare.com".to_string())
    );
}

#[test]
fn expose_tunnel_status_redacts_secret_like_error() {
    let err = anyhow::anyhow!("failed: Authorization: Bearer expose-tunnel-token");
    let status = expose_tunnel_unavailable_status(&err);

    assert!(status.contains("unavailable:"));
    assert!(status.contains("Authorization: Bearer <redacted>"));
    assert!(!status.contains("expose-tunnel-token"));
}

#[cfg(unix)]
#[test]
fn expose_pty_shutdown_terminates_and_joins_shell_threads() {
    let args = ExposeArgs {
        command: Some("sleep 30".to_string()),
        cols: 80,
        rows: 24,
        max_clients: 1,
        tunnel: false,
        no_tunnel: false,
    };
    let pty = ExposePty::spawn(&args).unwrap();

    pty.shutdown();

    assert!(!pty.running.load(Ordering::SeqCst));
    assert!(pty.reader_thread.lock().unwrap().is_none());
    assert!(pty.wait_thread.lock().unwrap().is_none());
}
