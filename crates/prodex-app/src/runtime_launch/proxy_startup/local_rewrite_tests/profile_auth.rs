use super::*;
use crate::{AppState, ProfileEntry, ProfileProvider};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};

#[test]
fn profile_auth_gateway_forwards_v1_responses_without_openai_api_key() {
    let _api_key = crate::TestEnvVarGuard::unset("OPENAI_API_KEY");
    let _api_keys = crate::TestEnvVarGuard::unset("OPENAI_API_KEYS");
    let root = temp_root("gateway-profile-auth-no-api-key");
    let paths = app_paths_for_root(root.clone());
    let state = profile_auth_state(&root, ["main", "second"], "main");
    let upstream = ScriptedProfileAuthUpstream::start(vec![scripted_json_response(
        200,
        serde_json::json!({"id":"resp_profile_auth","output":[]}),
    )]);
    let proxy = start_profile_auth_gateway(&paths, &state, upstream.base_url(), None);

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses?trace=1", proxy.listen_addr))
        .json(&serde_json::json!({"model":"gpt-5.5","input":"hello"}))
        .send()
        .expect("profile-auth gateway request should send");

    assert_response_status(response, 200);
    let requests = upstream.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/backend-api/codex/responses?trace=1");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer main-token")
    );
    assert_eq!(requests[0].account.as_deref(), Some("main-account"));
}

#[test]
fn profile_auth_gateway_caller_auth_does_not_select_upstream_profile() {
    let root = temp_root("gateway-profile-auth-caller-decoupled");
    let paths = app_paths_for_root(root.clone());
    let state = profile_auth_state(&root, ["main", "second"], "main");
    let open_upstream = ScriptedProfileAuthUpstream::start(vec![
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_open","output":[]}),
        ),
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_open_retry","output":[]}),
        ),
    ]);
    let open_proxy = start_profile_auth_gateway(&paths, &state, open_upstream.base_url(), None);
    let upstream = ScriptedProfileAuthUpstream::start(vec![
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth","output":[]}),
        ),
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_retry","output":[]}),
        ),
    ]);
    let proxy = start_profile_auth_gateway(
        &paths,
        &state,
        upstream.base_url(),
        Some("local-client-token"),
    );
    let virtual_key_upstream = ScriptedProfileAuthUpstream::start(vec![
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_alpha","output":[]}),
        ),
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_beta","output":[]}),
        ),
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_alpha_retry","output":[]}),
        ),
        scripted_json_response(
            200,
            serde_json::json!({"id":"resp_profile_auth_beta_retry","output":[]}),
        ),
    ]);
    let client = reqwest::blocking::Client::new();

    let open = client
        .post(format!("http://{}/v1/responses", open_proxy.listen_addr))
        .json(&serde_json::json!({"model":"gpt-5.5","input":"no caller auth"}))
        .send()
        .expect("request without caller auth should send");
    assert_response_status(open, 200);
    let open_requests = open_upstream.requests_after(Duration::from_secs(2), 1);
    assert!(!open_requests.is_empty());
    assert!(
        open_requests
            .iter()
            .all(|request| request.authorization.as_deref() == Some("Bearer main-token"))
    );

    let rejected = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("wrong-local-client-token")
        .json(&serde_json::json!({"model":"gpt-5.5","input":"reject me"}))
        .send()
        .expect("invalid caller request should send");
    assert_response_status(rejected, 401);
    assert!(
        upstream
            .requests_after(Duration::from_millis(100), 1)
            .is_empty(),
        "configured-invalid inbound auth must be rejected before profile selection"
    );

    let accepted = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth("local-client-token")
        .json(&serde_json::json!({"model":"gpt-5.5","input":"accept me"}))
        .send()
        .expect("valid caller request should send");
    assert_response_status(accepted, 200);

    let requests = upstream.requests_after(Duration::from_secs(2), 1);
    assert!(
        !requests.is_empty(),
        "the valid caller request reaches upstream"
    );
    assert!(
        requests
            .iter()
            .all(|request| request.authorization.as_deref() == Some("Bearer main-token"))
    );
    assert!(
        requests
            .iter()
            .all(|request| request.authorization.as_deref() != Some("Bearer local-client-token"))
    );

    let alpha_token = "alpha-local-client-token";
    let beta_token = "beta-local-client-token";
    let virtual_key_proxy = start_profile_auth_gateway_with_virtual_keys(
        &paths,
        &state,
        virtual_key_upstream.base_url(),
        None,
        vec![
            runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "alpha".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    alpha_token,
                ),
                allowed_models: Vec::new(),
                budget_microusd: None,
                request_budget: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "beta".to_string(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(beta_token),
                allowed_models: Vec::new(),
                budget_microusd: None,
                request_budget: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        ],
    );
    for (token, input) in [
        (alpha_token, "alpha virtual key"),
        (beta_token, "beta virtual key"),
    ] {
        let response = client
            .post(format!(
                "http://{}/v1/responses",
                virtual_key_proxy.listen_addr
            ))
            .bearer_auth(token)
            .json(&serde_json::json!({"model":"gpt-5.5","input":input}))
            .send()
            .expect("virtual-key caller request should send");
        assert_response_status(response, 200);
    }
    let virtual_key_requests = virtual_key_upstream.requests_after(Duration::from_secs(2), 2);
    assert!(
        virtual_key_requests.len() >= 2,
        "both virtual-key requests should reach upstream"
    );
    let first_profile_auth = virtual_key_requests
        .first()
        .and_then(|request| request.authorization.as_deref())
        .expect("virtual-key profile auth should be injected upstream");
    assert!(
        virtual_key_requests
            .iter()
            .all(|request| request.authorization.as_deref() == Some(first_profile_auth)),
        "different inbound virtual keys must not change upstream profile choice: {virtual_key_requests:?}"
    );
    assert!(
        virtual_key_requests.iter().all(|request| {
            !matches!(
                request.authorization.as_deref(),
                Some("Bearer alpha-local-client-token") | Some("Bearer beta-local-client-token")
            )
        }),
        "inbound virtual keys must not become upstream profile credentials: {virtual_key_requests:?}"
    );
}

#[test]
fn profile_auth_gateway_uses_fresh_selection_and_continuation_affinity() {
    let root = temp_root("gateway-profile-auth-affinity");
    let paths = app_paths_for_root(root.clone());
    let mut state = profile_auth_state(&root, ["main", "second"], "second");
    state.response_profile_bindings.insert(
        "resp-main".to_string(),
        crate::ResponseProfileBinding {
            profile_name: "main".to_string(),
            bound_at: chrono::Local::now().timestamp(),
        },
    );
    let upstream = ScriptedProfileAuthUpstream::start(vec![
        scripted_json_response(200, serde_json::json!({"id":"resp-second","output":[]})),
        scripted_json_response(200, serde_json::json!({"id":"resp-main-next","output":[]})),
    ]);
    let proxy = start_profile_auth_gateway(&paths, &state, upstream.base_url(), None);
    let client = reqwest::blocking::Client::new();

    let fresh = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({"model":"gpt-5.5","input":"fresh"}))
        .send()
        .expect("fresh request should send");
    assert_response_status(fresh, 200);

    let continuation = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model":"gpt-5.5",
            "previous_response_id":"resp-main",
            "input":"continue"
        }))
        .send()
        .expect("continuation request should send");
    assert_response_status(continuation, 200);

    let requests = upstream.requests_after(Duration::from_secs(2), 2);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.authorization.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("Bearer second-token"), Some("Bearer main-token")]
    );
}

#[test]
fn profile_auth_gateway_rotates_precommit_failures_and_pins_stream_success() {
    let root = temp_root("gateway-profile-auth-rotation");
    let paths = app_paths_for_root(root.clone());
    let state = profile_auth_state(&root, ["main", "second"], "main");
    let upstream = ScriptedProfileAuthUpstream::start(vec![
        scripted_json_response(
            429,
            serde_json::json!({"error":{"code":"rate_limit_exceeded","message":"rate limited"}}),
        ),
        ScriptedResponse {
            status: 200,
            content_type: "text/event-stream",
            body: concat!(
                "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-stream\"}}\n\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-stream\",\"output\":[]}}\n\n",
                "data: [DONE]\n\n"
            )
            .to_string(),
        },
    ]);
    let proxy = start_profile_auth_gateway(&paths, &state, upstream.base_url(), None);

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({"model":"gpt-5.5","stream":true,"input":"hello"}))
        .send()
        .expect("streaming profile-auth request should send");
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_else(|err| err.to_string());
    let response_requests = upstream.requests_after(Duration::from_secs(2), 2);
    assert_eq!(
        status, 200,
        "response body: {body}; response requests: {response_requests:?}"
    );

    let requests = upstream.requests_after(Duration::from_secs(2), 2);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.account.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("main-account"), Some("second-account")]
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| request.authorization.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("Bearer main-token"), Some("Bearer second-token")]
    );
    assert_eq!(
        requests.len(),
        2,
        "stream success remains pinned after commit"
    );
}

#[test]
fn profile_auth_gateway_rejects_out_of_scope_endpoints() {
    let root = temp_root("gateway-profile-auth-scope");
    let paths = app_paths_for_root(root.clone());
    let state = profile_auth_state(&root, ["main"], "main");
    let upstream = ScriptedProfileAuthUpstream::start(Vec::new());
    let proxy = start_profile_auth_gateway(&paths, &state, upstream.base_url(), None);

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({"model":"gpt-5.5","messages":[]}))
        .send()
        .expect("out-of-scope request should send");
    assert_response_status(response, 404);
    assert!(upstream.requests().is_empty());
}

fn start_profile_auth_gateway(
    paths: &crate::AppPaths,
    state: &AppState,
    upstream_base_url: String,
    gateway_token: Option<&str>,
) -> crate::RuntimeRotationProxy {
    start_profile_auth_gateway_with_virtual_keys(
        paths,
        state,
        upstream_base_url,
        gateway_token,
        Vec::new(),
    )
}

fn start_profile_auth_gateway_with_virtual_keys(
    paths: &crate::AppPaths,
    state: &AppState,
    upstream_base_url: String,
    gateway_token: Option<&str>,
    gateway_virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
) -> crate::RuntimeRotationProxy {
    seed_profile_auth_usage_snapshots(paths, state);
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider: RuntimeLocalRewriteProviderOptions::ProfileAuthOpenAiResponses,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: gateway_token
            .map(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(paths),
        gateway_virtual_keys,
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("profile-auth gateway should start")
}

fn seed_profile_auth_usage_snapshots(paths: &crate::AppPaths, state: &AppState) {
    let now = chrono::Local::now().timestamp();
    let snapshots = state
        .profiles
        .keys()
        .map(|name| {
            (
                name.clone(),
                crate::RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: crate::RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: crate::RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 80,
                    weekly_reset_at: now + 86_400,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    crate::save_runtime_usage_snapshots_for_profiles(paths, &snapshots, &state.profiles)
        .expect("profile-auth quota fixture snapshots should save");
    let saved = crate::load_runtime_usage_snapshots(paths, &state.profiles)
        .expect("profile-auth quota fixture snapshots should reload");
    assert_eq!(
        saved.len(),
        state.profiles.len(),
        "profile-auth quota fixture snapshots should be available before gateway startup"
    );
}

fn profile_auth_state<const N: usize>(
    root: &std::path::Path,
    names: [&str; N],
    active_profile: &str,
) -> AppState {
    let profiles = names
        .into_iter()
        .map(|name| {
            let codex_home = root.join(format!("homes/{name}"));
            write_profile_auth_json(
                &codex_home.join("auth.json"),
                &format!("{name}-token"),
                &format!("{name}-account"),
            );
            (
                name.to_string(),
                ProfileEntry {
                    codex_home,
                    managed: true,
                    email: Some(format!("{name}@example.test")),
                    provider: ProfileProvider::Openai,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    AppState {
        active_profile: Some(active_profile.to_string()),
        profiles,
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    }
}

fn write_profile_auth_json(path: &std::path::Path, access_token: &str, account_id: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("auth parent should be created");
    }
    fs::write(
        path,
        serde_json::json!({
            "tokens": {
                "access_token": access_token,
                "account_id": account_id,
            }
        })
        .to_string(),
    )
    .expect("auth.json should be written");
}

fn scripted_json_response(status: u16, body: serde_json::Value) -> ScriptedResponse {
    ScriptedResponse {
        status,
        content_type: "application/json",
        body: body.to_string(),
    }
}

fn assert_response_status(response: reqwest::blocking::Response, expected: u16) {
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_else(|err| err.to_string());
    assert_eq!(status, expected, "response body: {body}");
}

struct ScriptedResponse {
    status: u16,
    content_type: &'static str,
    body: String,
}

#[derive(Clone, Debug)]
struct ScriptedRequest {
    path: String,
    authorization: Option<String>,
    account: Option<String>,
}

struct ScriptedProfileAuthUpstream {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<ScriptedRequest>>>,
    _thread: thread::JoinHandle<()>,
}

impl ScriptedProfileAuthUpstream {
    fn start(responses: Vec<ScriptedResponse>) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("scripted upstream should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("scripted upstream should expose addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let requests_thread = Arc::clone(&requests);
        let mut responses = VecDeque::from(responses);
        let thread = thread::spawn(move || {
            loop {
                let Ok(mut request) = server.recv() else {
                    break;
                };
                let path = request.url().to_string();
                let authorization = request
                    .headers()
                    .iter()
                    .find(|header| header.field.equiv("Authorization"))
                    .map(|header| header.value.as_str().to_string());
                let account = request
                    .headers()
                    .iter()
                    .find(|header| header.field.equiv("ChatGPT-Account-Id"))
                    .map(|header| header.value.as_str().to_string());
                drain_fixed_request_body(&mut request);
                if !path.contains("/codex/responses") {
                    let mut response =
                        TinyResponse::from_string(scripted_usage_response_body(account.as_deref()))
                            .with_status_code(200);
                    response.add_header(
                        TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                    );
                    let _ = request.respond(response);
                    continue;
                }
                let Some(scripted) = responses.pop_front() else {
                    let mut response =
                        TinyResponse::from_string(r#"{"error":"unexpected_request"}"#)
                            .with_status_code(500);
                    response.add_header(
                        TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                    );
                    let _ = request.respond(response);
                    continue;
                };
                requests_thread
                    .lock()
                    .expect("requests should lock")
                    .push(ScriptedRequest {
                        path,
                        authorization,
                        account,
                    });
                let mut response =
                    TinyResponse::from_string(scripted.body).with_status_code(scripted.status);
                response.add_header(
                    TinyHeader::from_bytes("content-type", scripted.content_type).unwrap(),
                );
                let _ = request.respond(response);
            }
        });

        Self {
            addr,
            requests,
            _thread: thread,
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.addr)
    }

    fn requests(&self) -> Vec<ScriptedRequest> {
        self.requests.lock().expect("requests should lock").clone()
    }

    fn requests_after(&self, timeout: Duration, expected: usize) -> Vec<ScriptedRequest> {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            let requests = self.requests();
            if requests.len() >= expected {
                return requests;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.requests()
    }
}

fn drain_fixed_request_body(request: &mut tiny_http::Request) {
    if let Some(content_len) = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Content-Length"))
        .and_then(|header| header.value.as_str().parse::<usize>().ok())
    {
        let mut body = vec![0; content_len];
        request
            .as_reader()
            .read_exact(&mut body)
            .expect("request body should read fixed content length");
    }
}

fn scripted_usage_response_body(account: Option<&str>) -> String {
    let now = chrono::Local::now().timestamp();
    serde_json::json!({
        "email": format!("{}@example.test", account.unwrap_or("profile")),
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 20,
                "reset_at": now + 18_000,
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 20,
                "reset_at": now + 604_800,
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}
