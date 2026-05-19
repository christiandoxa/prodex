use super::*;

struct RuntimeCompactionV2Backend {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    responses_accounts: Arc<Mutex<Vec<String>>>,
    responses_headers: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: Arc<Mutex<Vec<String>>>,
    connection_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    thread: Option<JoinHandle<()>>,
}

impl RuntimeCompactionV2Backend {
    fn start() -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("failed to bind compaction v2 backend");
        let addr = listener
            .local_addr()
            .expect("failed to read compaction v2 backend address");
        listener
            .set_nonblocking(true)
            .expect("failed to set compaction v2 backend nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let responses_accounts = Arc::new(Mutex::new(Vec::new()));
        let responses_headers = Arc::new(Mutex::new(Vec::new()));
        let responses_bodies = Arc::new(Mutex::new(Vec::new()));
        let connection_threads = Arc::new(Mutex::new(Vec::new()));

        let shutdown_flag = Arc::clone(&shutdown);
        let responses_accounts_flag = Arc::clone(&responses_accounts);
        let responses_headers_flag = Arc::clone(&responses_headers);
        let responses_bodies_flag = Arc::clone(&responses_bodies);
        let connection_threads_flag = Arc::clone(&connection_threads);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let responses_accounts_flag = Arc::clone(&responses_accounts_flag);
                        let responses_headers_flag = Arc::clone(&responses_headers_flag);
                        let responses_bodies_flag = Arc::clone(&responses_bodies_flag);
                        let handler = thread::spawn(move || {
                            handle_compaction_v2_backend_request(
                                stream,
                                &responses_accounts_flag,
                                &responses_headers_flag,
                                &responses_bodies_flag,
                            );
                        });
                        connection_threads_flag
                            .lock()
                            .expect("connection_threads poisoned")
                            .push(handler);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            shutdown,
            responses_accounts,
            responses_headers,
            responses_bodies,
            connection_threads,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.addr)
    }

    fn responses_accounts(&self) -> Vec<String> {
        self.responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .clone()
    }

    fn responses_headers(&self) -> Vec<BTreeMap<String, String>> {
        self.responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .clone()
    }

    fn responses_bodies(&self) -> Vec<String> {
        self.responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .clone()
    }
}

impl Drop for RuntimeCompactionV2Backend {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let mut handlers = self
            .connection_threads
            .lock()
            .expect("connection_threads poisoned");
        while let Some(thread) = handlers.pop() {
            let _ = thread.join();
        }
    }
}

fn handle_compaction_v2_backend_request(
    mut stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: &Arc<Mutex<Vec<String>>>,
) {
    let Some(request) = read_http_request(&mut stream) else {
        return;
    };
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let request_body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    let account_id = request_header(&request, "ChatGPT-Account-Id").unwrap_or_default();

    if path.ends_with("/backend-api/codex/responses") {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(request_headers_map(&request));
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body);

        if account_id == "second-account" {
            write_compaction_v2_backend_response(
                stream,
                "HTTP/1.1 200 OK",
                "text/event-stream",
                compaction_v2_sse_body(),
                Some("turn-compact-v2"),
            );
        } else {
            write_compaction_v2_backend_response(
                stream,
                "HTTP/1.1 503 Service Unavailable",
                "application/json",
                serde_json::json!({"error":"wrong_profile"}).to_string(),
                None,
            );
        }
    } else if path.ends_with("/backend-api/wham/usage") {
        let email = runtime_proxy_backend_profile_name_for_account_id(&account_id)
            .map(|profile_name| format!("{profile_name}@example.com"))
            .unwrap_or_else(|| "unknown@example.com".to_string());
        write_compaction_v2_backend_response(
            stream,
            "HTTP/1.1 200 OK",
            "application/json",
            runtime_proxy_usage_body(&email),
            None,
        );
    } else {
        write_compaction_v2_backend_response(
            stream,
            "HTTP/1.1 404 Not Found",
            "application/json",
            serde_json::json!({"error":"not_found"}).to_string(),
            None,
        );
    }
}

fn write_compaction_v2_backend_response(
    mut stream: TcpStream,
    status_line: &'static str,
    content_type: &'static str,
    body: String,
    turn_state: Option<&str>,
) {
    let mut headers = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len(),
    );
    if let Some(turn_state) = turn_state {
        headers.push_str(&format!("x-codex-turn-state: {turn_state}\r\n"));
    }
    headers.push_str("\r\n");
    let _ = stream.write_all(headers.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn compaction_v2_sse_body() -> String {
    concat!(
        "event: response.created\r\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-compact-v2\",\"headers\":{\"x-codex-turn-state\":\"turn-compact-v2\"}}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction\",\"encrypted_content\":\"enc-compact-v2\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-compact-v2\"}}\r\n",
        "\r\n",
    )
    .to_string()
}

#[test]
fn runtime_proxy_http_tool_output_with_session_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_quota_then_tool_output_fresh_fallback_error();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-main",
                "session_id": "sess-replayable",
                "input": [{
                    "type": "function_call_output",
                    "call_id": "call_tk0AjVbCh1EZCS0XTVva002N",
                    "output": "ok",
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("insufficient_quota"),
        "quota failure should pass through instead of degrading into fresh tool-output retry: {body}"
    );
    assert!(
        !body.contains("No tool call found"),
        "proxy should not create a fresh tool-output request that loses call context: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["main-account".to_string()],
        "tool-output continuations must not rotate away from the owning profile: {responses_accounts:?}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should observe only the original continuation: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-main\""),
        "first request should preserve previous_response_id: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "original request should preserve session_id: {}",
        responses_bodies[0]
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("upstream_usage_limit_passthrough") || log.contains("quota_blocked"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        !log.contains("previous_response_fresh_fallback reason=quota_blocked"),
        "quota-blocked tool-output path must not drop previous_response_id: {log}"
    );
}

#[test]
fn runtime_proxy_http_tool_output_with_session_surfaces_stale_continuation_without_fresh_retry() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_previous_response_tool_context_missing();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-second",
                "session_id": "sess-replayable",
                "input": [{
                    "type": "function_call_output",
                    "call_id": "call_J7U3Kdc539EyfWU4nZj9LCWQZ",
                    "output": "ok",
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 409);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "HTTP should translate tool-context loss into stale_continuation: {body}"
    );
    assert!(
        !body.contains("No tool call found"),
        "HTTP should not surface the upstream tool-context string after classification: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should not observe a second fresh tool-output replay"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-second\""),
        "first request should preserve previous_response_id: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "original request should preserve session_id: {}",
        responses_bodies[0]
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("previous_response_not_found"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        log.contains("previous_response_not_found"),
        "runtime log should classify upstream tool-context loss as a continuation miss: {log}"
    );
    assert!(
        !log.contains("previous_response_fresh_fallback reason=previous_response_not_found"),
        "tool-output-only context misses must not trigger fresh replay: {log}"
    );
}

#[test]
fn runtime_proxy_http_compaction_v2_stream_uses_session_bound_profile() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeCompactionV2Backend::start();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-compact-v2".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");
    save_runtime_usage_snapshots(
        &paths,
        &BTreeMap::from([(
            "second".to_string(),
            ready_runtime_usage_snapshot(now, 95),
        )]),
    )
    .expect("failed to save usage snapshots");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("session-id", "sess-compact-v2")
        .body(
            serde_json::json!({
                "model": "gpt-5",
                "input": [
                    {
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "compact the session"}],
                    },
                    {"type": "compaction_trigger"},
                ],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"type\":\"response.output_item.done\"")
            && body.contains("\"type\":\"compaction\"")
            && body.contains("\"encrypted_content\":\"enc-compact-v2\""),
        "compaction v2 output item should be forwarded unchanged: {body}"
    );
    assert!(
        body.contains("\"type\":\"response.completed\""),
        "terminal response.completed should be forwarded: {body}"
    );
    assert!(
        !body.contains("insufficient_quota")
            && !body.contains("previous_response_not_found")
            && !body.contains("stale_continuation"),
        "compaction v2 stream should not be translated into retry failure: {body}"
    );

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()],
        "session-bound compaction stream should stay on the owning profile"
    );
    let responses_headers = backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should observe one responses request: {responses_headers:?}"
    );
    assert_eq!(
        responses_headers[0].get("session-id").map(String::as_str),
        Some("sess-compact-v2")
    );
    assert_eq!(
        responses_headers[0]
            .get("chatgpt-account-id")
            .map(String::as_str),
        Some("second-account")
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should observe one responses body: {responses_bodies:?}"
    );
    let request_body = serde_json::from_str::<serde_json::Value>(&responses_bodies[0])
        .expect("upstream request body should be JSON");
    assert_eq!(
        request_body
            .get("input")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.last())
            .and_then(|item| item.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("compaction_trigger")
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("sse_commit") || log.contains("sse_quota_blocked"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        log.contains("sse_commit profile=second"),
        "compaction v2 SSE should commit on the session-bound profile: {log}"
    );
    assert!(
        !log.contains("sse_quota_blocked")
            && !log.contains("previous_response_not_found")
            && !log.contains("stale_continuation"),
        "compaction v2 SSE should not be classified as retry failure: {log}"
    );
}

#[test]
fn runtime_proxy_http_compact_previous_response_not_found_surfaces_stale_continuation() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_compact_previous_response_not_found();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            runtime_compact_session_lineage_key("sess-compact"),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            turn_state_bindings: BTreeMap::from([(
                "turn-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                turn_state: BTreeMap::from([(
                    "turn-second".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 1,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save initial continuations");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-openai-subagent", "compact")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-compact-missing",
                "session_id": "sess-compact",
                "input": [],
                "instructions": "compact",
            })
            .to_string(),
        )
        .send()
        .expect("compact request should succeed");

    assert_eq!(response.status().as_u16(), 409);
    let body = response.text().expect("compact body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "compact previous_response miss should surface stale_continuation: {body}"
    );
    assert!(
        !body.contains("previous_response_not_found"),
        "compact path should not leak raw previous_response_not_found: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "compact path should not retry after a stale continuation: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-compact-missing\""),
        "compact request should preserve the original previous_response_id: {}",
        responses_bodies[0]
    );
}
