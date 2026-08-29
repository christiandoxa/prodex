use super::runtime_auth::{
    COPILOT_RUNTIME_API_VERSION, COPILOT_RUNTIME_INTEGRATION_ID, CopilotRuntimeApiAuth,
    copilot_runtime_model_catalog_from_token, refresh_copilot_runtime_api_auth_with_urls,
};
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

fn start_copilot_auth_test_server(
    routes: Vec<(&'static str, u16, serde_json::Value)>,
) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let base_url = format!(
        "http://{}",
        listener
            .local_addr()
            .expect("test server address should resolve")
    );
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_thread = Arc::clone(&observed);
    let handle = std::thread::spawn(move || {
        for (path, status, body) in routes {
            serve_copilot_auth_test_route(&listener, &observed_for_thread, path, status, body);
        }
    });
    (base_url, observed, handle)
}

fn serve_copilot_auth_test_route(
    listener: &TcpListener,
    observed: &Arc<Mutex<Vec<String>>>,
    path: &str,
    status: u16,
    body: serde_json::Value,
) {
    let (mut stream, _) = listener.accept().expect("test server should accept");
    let request = read_copilot_auth_test_request(&mut stream);
    let first_line = request.lines().next().unwrap_or_default().to_string();
    assert_eq!(first_line, format!("GET {path} HTTP/1.1"));
    observed
        .lock()
        .expect("observed requests lock should not be poisoned")
        .push(request);
    let body = body.to_string();
    let status_text = if status == 200 { "OK" } else { "Test" };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .expect("response should write");
}

fn read_copilot_auth_test_request(stream: &mut std::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).expect("request should read");
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..read]);
        if raw.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&raw).to_string()
}

fn request_header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request.lines().find_map(|line| {
        let (header, value) = line.split_once(':')?;
        header.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[test]
fn copilot_auth_debug_output_redacts_sensitive_fields() {
    let auth = CopilotRuntimeApiAuth {
        api_key: "copilot-runtime-key-secret".to_string(),
        model_catalog: vec![serde_json::json!({
            "id": "copilot-model-secret",
            "name": "Copilot Secret Model"
        })],
    };
    let rendered = format!("{auth:?}");

    assert!(rendered.contains("CopilotRuntimeApiAuth"));
    assert!(rendered.contains("<redacted>"));
    assert!(rendered.contains("<redacted:1>"));
    for raw in [
        "copilot-runtime-key-secret",
        "copilot-model-secret",
        "Copilot Secret Model",
    ] {
        assert!(!rendered.contains(raw), "{rendered}");
    }

    let context = CopilotImportContext {
        host: "https://github.enterprise-secret.test".to_string(),
        login: "alice-secret".to_string(),
        token: "copilot-import-token-secret".to_string(),
    };
    let rendered = format!("{context:?}");

    assert!(rendered.contains("CopilotImportContext"));
    assert!(rendered.contains("<redacted>"));
    for raw in [
        "https://github.enterprise-secret.test",
        "alice-secret",
        "copilot-import-token-secret",
    ] {
        assert!(!rendered.contains(raw), "{rendered}");
    }
}

#[test]
fn copilot_import_candidates_try_last_user_then_other_logged_in_users() {
    let config = CopilotConfigFile {
        last_logged_in_user: Some(prodex_profile_export::CopilotConfigUser {
            host: "https://github.com".to_string(),
            login: "missing-token".to_string(),
        }),
        logged_in_users: vec![
            prodex_profile_export::CopilotConfigUser {
                host: "https://github.com".to_string(),
                login: "missing-token".to_string(),
            },
            prodex_profile_export::CopilotConfigUser {
                host: "https://github.com".to_string(),
                login: "usable".to_string(),
            },
        ],
        copilot_tokens: Default::default(),
    };

    let users = copilot_import_candidate_users(&config);

    assert_eq!(users.len(), 2);
    assert_eq!(users[0].login, "missing-token");
    assert_eq!(users[1].login, "usable");
}

#[test]
fn copilot_runtime_auth_uses_oauth_models_before_legacy_exchange() {
    let (base_url, observed, handle) = start_copilot_auth_test_server(vec![(
        "/models",
        200,
        serde_json::json!({
            "data": [
                {
                    "id": "gpt-5.3-codex",
                    "name": "GPT-5.3 Codex",
                    "capabilities": {
                        "limits": {
                            "max_context_window_tokens": 400000,
                            "max_prompt_tokens": 272000
                        }
                    }
                }
            ]
        }),
    )]);
    let client = Client::new();

    let auth = refresh_copilot_runtime_api_auth_with_urls(
        &client,
        &format!("{base_url}/copilot_internal/v2/token"),
        &base_url,
        "oauth-token",
    )
    .expect("direct OAuth models request should succeed");

    handle.join().expect("test server should finish");
    let requests = observed
        .lock()
        .expect("observed requests lock should not be poisoned");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        request_header(&requests[0], "authorization"),
        Some("Bearer oauth-token")
    );
    assert_eq!(
        request_header(&requests[0], "copilot-integration-id"),
        Some(COPILOT_RUNTIME_INTEGRATION_ID)
    );
    assert_eq!(
        request_header(&requests[0], "x-github-api-version"),
        Some(COPILOT_RUNTIME_API_VERSION)
    );
    assert_eq!(auth.api_key, "oauth-token");
    assert_eq!(auth.model_catalog.len(), 1);
    assert_eq!(auth.model_catalog[0]["id"], "gpt-5.3-codex");
    assert_eq!(auth.model_catalog[0]["context_window"], 272000);
}

#[test]
fn copilot_runtime_auth_falls_back_to_legacy_exchange_when_models_fails() {
    let (base_url, observed, handle) = start_copilot_auth_test_server(vec![
        (
            "/models",
            404,
            serde_json::json!({
                "message": "Not Found"
            }),
        ),
        (
            "/copilot_internal/v2/token",
            200,
            serde_json::json!({
                "token": "runtime-token",
                "models": [
                    {
                        "id": "gpt-5.1-codex",
                        "name": "GPT-5.1 Codex",
                        "context_window": 400000
                    }
                ]
            }),
        ),
    ]);
    let client = Client::new();

    let auth = refresh_copilot_runtime_api_auth_with_urls(
        &client,
        &format!("{base_url}/copilot_internal/v2/token"),
        &base_url,
        "oauth-token",
    )
    .expect("legacy exchange should be used after models failure");

    handle.join().expect("test server should finish");
    let requests = observed
        .lock()
        .expect("observed requests lock should not be poisoned");
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /models HTTP/1.1"));
    assert!(requests[1].starts_with("GET /copilot_internal/v2/token HTTP/1.1"));
    assert_eq!(
        request_header(&requests[1], "authorization"),
        Some("token oauth-token")
    );
    assert_eq!(auth.api_key, "runtime-token");
    assert_eq!(auth.model_catalog.len(), 1);
    assert_eq!(auth.model_catalog[0]["id"], "gpt-5.1-codex");
}

#[test]
fn copilot_runtime_auth_uses_canonical_fallback_when_discovery_is_unavailable() {
    let (base_url, observed, handle) = start_copilot_auth_test_server(vec![
        (
            "/models",
            503,
            serde_json::json!({"message": "unavailable"}),
        ),
        (
            "/copilot_internal/v2/token",
            404,
            serde_json::json!({"message": "removed"}),
        ),
    ]);
    let client = Client::new();

    let auth = refresh_copilot_runtime_api_auth_with_urls(
        &client,
        &format!("{base_url}/copilot_internal/v2/token"),
        &base_url,
        "oauth-token",
    )
    .expect("optional discovery failure should keep direct OAuth usable");

    handle.join().expect("test server should finish");
    assert_eq!(
        observed
            .lock()
            .expect("observed requests lock should not be poisoned")
            .len(),
        2
    );
    assert_eq!(auth.api_key, "oauth-token");
    assert!(auth.model_catalog.is_empty());
}

#[test]
fn copilot_runtime_model_catalog_reads_token_models() {
    let value = serde_json::json!({
        "token": "runtime-token",
        "models": [
            {
                "id": "gpt-5.1-codex",
                "name": "GPT-5.1 Codex",
                "context_window": 400000,
                "capabilities": { "tool_calls": true }
            },
            {
                "model": "claude-sonnet-4.5",
                "display_name": "Claude Sonnet 4.5",
                "max_context_tokens": 200000
            }
        ]
    });

    let catalog = copilot_runtime_model_catalog_from_token(&value).unwrap();

    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[0]["id"], "gpt-5.1-codex");
    assert_eq!(catalog[0]["display_name"], "GPT-5.1 Codex");
    assert_eq!(catalog[0]["context_window"], 400000);
    assert_eq!(catalog[0]["capabilities"]["tool_calls"], true);
    assert_eq!(catalog[1]["id"], "claude-sonnet-4.5");
}

#[test]
fn copilot_runtime_model_catalog_prefers_prompt_limit_for_codex_budget() {
    let value = serde_json::json!({
        "models": [
            {
                "id": "gpt-5.3-codex",
                "name": "GPT-5.3-Codex",
                "capabilities": {
                    "limits": {
                        "max_context_window_tokens": 400000,
                        "max_prompt_tokens": 272000,
                        "max_output_tokens": 128000
                    }
                }
            }
        ]
    });

    let catalog = copilot_runtime_model_catalog_from_token(&value).unwrap();

    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0]["id"], "gpt-5.3-codex");
    assert_eq!(catalog[0]["context_window"], 272000);
    assert_eq!(catalog[0]["max_context_window"], 400000);
    assert_eq!(catalog[0]["max_prompt_tokens"], 272000);
}

#[test]
fn copilot_runtime_model_catalog_reads_nested_available_models() {
    let value = serde_json::json!({
        "token": "runtime-token",
        "features": {
            "available_models": [
                { "slug": "gemini-3.1-pro-preview", "label": "Gemini 3.1 Pro Preview" }
            ]
        }
    });

    let catalog = copilot_runtime_model_catalog_from_token(&value).unwrap();

    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0]["id"], "gemini-3.1-pro-preview");
    assert_eq!(catalog[0]["display_name"], "Gemini 3.1 Pro Preview");
}

#[test]
fn copilot_runtime_model_catalog_rejects_oversized_payloads() {
    let value = serde_json::json!({
        "models": (0..=prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT)
            .map(|index| serde_json::json!({"id": format!("model-{index}")}))
            .collect::<Vec<_>>()
    });

    let error = copilot_runtime_model_catalog_from_token(&value).unwrap_err();

    assert!(error.to_string().contains("hard limit of 1024 entries"));
}
