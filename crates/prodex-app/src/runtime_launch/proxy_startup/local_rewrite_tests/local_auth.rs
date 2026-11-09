//! Local endpoint keys must cross only the trusted upstream transport boundary.
use super::{
    AppState, RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyStartOptions,
    TestUpstream, app_paths_for_root, start_runtime_local_rewrite_proxy, temp_root,
};
use serde_json::json;
use std::{fs, time::Duration};

const KEY: &str = "synthetic-local-auth-fixture";
const CHAT: &str = r#"{"id":"chatcmpl_auth_fixture","object":"chat.completion","model":"offrail-27b","choices":[{"index":0,"message":{"role":"assistant","content":"AUTH_OK"},"finish_reason":"stop"}]}"#;
const RESPONSES_SSE: &str = "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_local_fixture\",\"object\":\"response\",\"status\":\"in_progress\",\"output\":[]}}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_local_fixture\",\"object\":\"response\",\"status\":\"completed\",\"output\":[]}}\n\n";

fn proxy(paths: &crate::AppPaths, url: String, key: Option<&str>) -> crate::RuntimeRotationProxy {
    start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state: &AppState::default(),
        upstream_base_url: url,
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            local: true,
            api_keys: key.into_iter().map(str::to_owned).collect(),
        },
        upstream_no_proxy: true,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(169984),
        preferred_listen_addr: Some("127.0.0.1:0"),
    })
    .unwrap()
}

#[test]
fn local_api_key_authenticates_inference_and_keeps_catalog_local_without_cloud_headers() {
    for (model, route, expected, response_body) in [
        (
            "offrail-27b",
            "/v1/responses",
            "/prefix/v1/chat/completions",
            CHAT,
        ),
        (
            "generic-local",
            "/v1/responses",
            "/prefix/v1/responses",
            r#"{"id":"resp_auth_fixture","output":[]}"#,
        ),
        (
            "generic-local",
            "/v1/models",
            "/prefix/v1/models",
            r#"{"data":[]}"#,
        ),
    ] {
        for key in [None, Some(KEY)] {
            let paths = app_paths_for_root(temp_root("local-key-forwarding"));
            let upstream = TestUpstream::start_with_responses(
                if route.ends_with("models") {
                    Vec::new()
                } else {
                    vec![200]
                },
                "application/json",
                response_body,
            );
            let proxy = proxy(&paths, format!("http://{}/prefix/v1", upstream.addr), key);
            let client = reqwest::blocking::Client::new();
            let url = format!("http://{}{route}", proxy.listen_addr);
            let request = if route.ends_with("models") {
                client.get(url)
            } else {
                client
                    .post(url)
                    .json(&json!({"model":model,"input":"fixture","stream":false}))
            };
            let response = request
                .header("authorization", "Bearer unrelated-cloud-credential")
                .header("chatgpt-account-id", "unrelated-account")
                .send()
                .unwrap();
            assert_eq!(
                response.status().as_u16(),
                200,
                "{}",
                response.text().unwrap()
            );
            if route.ends_with("models") {
                // Codex model discovery uses Prodex's configured catalog, not a remote
                // authentication preflight; it must not leak any header upstream.
                assert!(upstream.path_rx.try_recv().is_err());
                assert!(upstream.headers_rx.try_recv().is_err());
                drop(proxy);
                fs::remove_dir_all(paths.root).unwrap();
                continue;
            }
            assert_eq!(
                upstream
                    .path_rx
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap(),
                expected
            );
            let headers = upstream
                .headers_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            let auth = headers
                .iter()
                .find(|(name, _)| name == "authorization")
                .map(|(_, value)| value.as_str());
            assert_eq!(
                auth.map(str::to_owned),
                key.map(|key| format!("Bearer {key}"))
            );
            assert!(
                !headers
                    .iter()
                    .any(|(name, value)| name == "chatgpt-account-id"
                        || value.contains("unrelated-cloud-credential"))
            );
            let log = crate::read_runtime_proxy_test_log(&proxy.log_path);
            assert!(!log.contains(KEY));
            assert!(!log.contains("unrelated-cloud-credential"));
            drop(proxy);
            fs::remove_dir_all(paths.root).unwrap();
        }
    }
}

#[test]
fn local_responses_tool_followup_keeps_unkeyed_and_keyed_sessions_working() {
    for key in [None, Some(KEY)] {
        let paths = app_paths_for_root(temp_root("local-responses-followup"));
        let upstream =
            TestUpstream::start_with_responses(vec![200, 200], "text/event-stream", RESPONSES_SSE);
        let proxy = proxy(&paths, format!("http://{}/v1", upstream.addr), key);
        let client = reqwest::blocking::Client::new();
        for input in [
            json!([{"type":"message","role":"user","content":"run a command"}]),
            json!([
                {"type":"message","role":"user","content":"run a command"},
                {"type":"function_call","call_id":"call_fixture","name":"exec_command","arguments":"{}"},
                {"type":"function_call_output","call_id":"call_fixture","output":"ok"}
            ]),
        ] {
            let response = client
                .post(format!("http://{}/v1/responses", proxy.listen_addr))
                .header("session_id", "session_fixture")
                .json(&json!({"model":"generic-local","input":input,"stream":true}))
                .send()
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            assert!(response.text().unwrap().contains("response.completed"));
            let headers = upstream
                .headers_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            let auth = headers.iter().find(|(name, _)| name == "authorization");
            assert_eq!(
                auth.map(|(_, value)| value.clone()),
                key.map(|v| format!("Bearer {v}"))
            );
        }
        drop(proxy);
        fs::remove_dir_all(paths.root).unwrap();
    }
}

#[test]
fn unkeyed_local_responses_rejects_bindings_for_other_keys_or_endpoints() {
    use prodex_provider_core::{ProviderId, RuntimeProviderBindingIdentity};
    use prodex_state::ResponseProfileBinding;

    let paths = app_paths_for_root(temp_root("local-unkeyed-binding-boundary"));
    let upstream = TestUpstream::start_with_responses(Vec::new(), "application/json", "{}");
    let url = format!("http://{}/v1", upstream.addr);
    for identity in [
        RuntimeProviderBindingIdentity::from_raw_key(ProviderId::OpenAi, KEY, &url, None),
        RuntimeProviderBindingIdentity::from_public_credential_identity(
            ProviderId::OpenAi,
            "prodex-local",
            "https://other.example.com/v1",
            None,
        ),
    ] {
        let mut state = AppState::default();
        state.session_profile_bindings.insert(
            "session_fixture".into(),
            ResponseProfileBinding {
                profile_name: "local".into(),
                bound_at: chrono::Local::now().timestamp(),
                binding_identity: Some(identity.unwrap()),
            },
        );
        let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
            paths: &paths,
            state: &state,
            upstream_base_url: url.clone(),
            provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
                local: true,
                api_keys: Vec::new(),
            },
            upstream_no_proxy: true,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: Some(131072),
            preferred_listen_addr: Some("127.0.0.1:0"),
        })
        .unwrap();
        let response = reqwest::blocking::Client::new()
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .header("session_id", "session_fixture")
            .json(&json!({"model":"generic-local","input":"fixture","stream":true}))
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 503);
        assert!(upstream.body_rx.try_recv().is_err());
        drop(proxy);
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn local_api_key_authentication_errors_remain_401_without_rotation_or_key_disclosure() {
    let paths = app_paths_for_root(temp_root("local-key-rejection"));
    let upstream = TestUpstream::start_with_responses(
        vec![401],
        "application/json",
        r#"{"error":{"message":"Invalid API key","type":"authentication_error"}}"#,
    );
    let proxy = proxy(&paths, format!("http://{}/v1", upstream.addr), Some(KEY));
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&json!({"model":"offrail-27b","input":"fixture","stream":true}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    assert!(!response.text().unwrap().contains(KEY));
    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        headers
            .iter()
            .any(|(name, value)| name == "authorization" && value == &format!("Bearer {KEY}"))
    );
    assert!(!crate::read_runtime_proxy_test_log(&proxy.log_path).contains(KEY));
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn local_api_key_does_not_follow_upstream_redirects() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let paths = app_paths_for_root(temp_root("local-key-redirect"));
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let redirected = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&redirected);
    let worker = std::thread::spawn(move || {
        let mut request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let mut body = Vec::new();
        request.as_reader().read_to_end(&mut body).unwrap();
        request
            .respond(
                tiny_http::Response::empty(307).with_header(
                    tiny_http::Header::from_bytes("Location", format!("http://{addr}/unexpected"))
                        .unwrap(),
                ),
            )
            .unwrap();
        if let Some(request) = server.recv_timeout(Duration::from_millis(500)).unwrap() {
            seen.store(true, Ordering::SeqCst);
            let _ = request.respond(tiny_http::Response::from_string(CHAT));
        }
    });
    let proxy = proxy(&paths, format!("http://{addr}/v1"), Some(KEY));
    let response = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&json!({"model":"offrail-27b","input":"fixture","stream":true}))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 307);
    worker.join().unwrap();
    assert!(!redirected.load(Ordering::SeqCst));
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}
