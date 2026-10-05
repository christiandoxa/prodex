use super::*;

#[test]
fn kiro_responses_route_reuses_previous_response_history() {
    let root = temp_root("kiro-responses-continuity");
    let paths = app_paths_for_root(root.clone());
    let codex_home = root.join("kiro-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    write_private_test_secret(
        codex_home.join("kiro_auth.json"),
        serde_json::json!({
            "auth_key": "kirocli:social:token",
            "auth_kind": "social",
            "auth_json": "{\"token\":\"abc\"}",
            "email": "kiro@example.com",
            "profile_arn": null,
            "profile_name": null,
            "start_url": null,
            "region": "us-east-1"
        })
        .to_string(),
    )
    .expect("kiro auth secret should be written");
    let fake_agent = write_fake_kiro_continuity_agent(&root);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Kiro {
            auth: RuntimeKiroProfileAuth {
                profile_name: "kiro-main".to_string(),
                codex_home: codex_home.clone(),
                model_catalog: vec![serde_json::json!({
                    "id": "claude-sonnet-4",
                    "name": "claude-sonnet-4",
                    "object": "model",
                    "owned_by": "kiro-cli"
                })],
                command: Some(fake_agent),
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
    })
    .expect("kiro local rewrite proxy should start");

    let first: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello from prodex"}]
            }]
        }))
        .send()
        .expect("first kiro request should be sent")
        .json()
        .expect("first response JSON should parse");
    let previous_response_id = first["id"].as_str().expect("first response id");

    let second_response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "previous_response_id": previous_response_id,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "follow up"}]
            }]
        }))
        .send()
        .expect("second kiro request should be sent");
    assert_eq!(second_response.status().as_u16(), 200);
    let second: serde_json::Value = second_response
        .json()
        .expect("second response JSON should parse");

    assert_eq!(second["output"][0]["content"][0]["text"], "second turn");
}
