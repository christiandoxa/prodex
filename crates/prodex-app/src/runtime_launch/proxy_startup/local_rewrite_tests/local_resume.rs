use super::*;
use crate::AppStateIoExt;
use prodex_state::ResponseProfileBinding;
use serde_json::json;

const SESSION: &str = "01900000-0000-7000-8000-000000000901";
const CHAT: &str = r#"{"id":"chatcmpl_resume_fixture","object":"chat.completion","model":"offrail-27b","choices":[{"index":0,"message":{"role":"assistant","content":"RESUMED_OK"},"finish_reason":"stop"}]}"#;

#[test]
fn local_resume_replays_full_history_despite_legacy_session_tombstone() {
    let paths = app_paths_for_root(temp_root("local-resume-tombstone"));
    let mut state = AppState::default();
    state.session_profile_bindings.insert(
        SESSION.into(),
        ResponseProfileBinding {
            profile_name: prodex_state::HARD_BINDING_CONFLICT_PROFILE.into(),
            bound_at: chrono::Local::now().timestamp(),
            binding_identity: None,
        },
    );
    state.save(&paths).unwrap();
    let upstream = TestUpstream::start_with_responses(vec![200], "application/json", CHAT);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &state,
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
            local: true,
        },
        upstream_no_proxy: true,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(169984),
        preferred_listen_addr: Some("127.0.0.1:0"),
    })
    .unwrap();
    let response = reqwest::blocking::Client::new().post(format!("http://{}/v1/responses", proxy.listen_addr))
        .header("session_id", SESSION).json(&json!({"model":"offrail-27b", "stream":false,
            "input":[{"role":"user","content":"earlier question"},
                {"role":"assistant","content":"earlier answer"},{"role":"user","content":"continue"}]}))
        .send().unwrap();
    let status = response.status().as_u16();
    let body = response.text().unwrap();
    assert_eq!(status, 200, "{body}");
    let sent: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        sent["messages"],
        json!([{"role":"user","content":"earlier question"},
        {"role":"assistant","content":"earlier answer"},{"role":"user","content":"continue"}])
    );
    drop(proxy);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn local_resume_keeps_opaque_continuations_cloud_and_other_models_fail_closed() {
    for (local, model, previous, turn) in [
        (true, "offrail-27b", true, false),
        (true, "offrail-27b", false, true),
        (false, "offrail-27b", false, false),
        (true, "other-model", false, false),
    ] {
        let paths = app_paths_for_root(temp_root("local-resume-boundaries"));
        let mut state = AppState::default();
        let conflict = ResponseProfileBinding {
            profile_name: prodex_state::HARD_BINDING_CONFLICT_PROFILE.into(),
            bound_at: chrono::Local::now().timestamp(),
            binding_identity: None,
        };
        state
            .session_profile_bindings
            .insert(SESSION.into(), conflict.clone());
        state
            .response_profile_bindings
            .insert("resp_private_chain".into(), conflict);
        let upstream = TestUpstream::start_with_responses(Vec::new(), "application/json", CHAT);
        let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
            paths: &paths,
            state: &state,
            upstream_base_url: format!("http://{}/v1", upstream.addr),
            provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
                api_keys: Vec::new(),
                local,
            },
            upstream_no_proxy: true,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: Some(169984),
            preferred_listen_addr: Some("127.0.0.1:0"),
        })
        .unwrap();
        let mut body =
            json!({"model":model,"input":[{"role":"user","content":"fixture"}],"stream":false});
        if previous {
            body["previous_response_id"] = "resp_private_chain".into();
        }
        let mut request = reqwest::blocking::Client::new()
            .post(format!("http://{}/v1/responses", proxy.listen_addr))
            .header("session_id", SESSION);
        if turn {
            request = request.header("x-codex-turn-state", "opaque-fixture-state");
        }
        let response = request.json(&body).send().unwrap();
        assert_eq!(
            response.status().as_u16(),
            503,
            "local={local} model={model} previous={previous} turn={turn}"
        );
        assert!(upstream.body_rx.try_recv().is_err());
        drop(proxy);
        fs::remove_dir_all(paths.root).unwrap();
    }
}

#[test]
fn local_resume_replays_native_reasoning_with_custom_tool_results() {
    let value = json!({"model":"offrail-27b","reasoning":{"effort":"medium"},"stream":true,
      "input":[{"role":"user","content":"inspect fixture"},
        {"type":"reasoning","id":"rs_fixture","summary":[{"type":"summary_text","text":"Inspect safely."}],"content":null,"encrypted_content":null},
        {"type":"custom_tool_call","id":"call_fixture","call_id":"call_fixture","name":"apply_patch","input":"*** Begin Patch\n*** Add File: fixture.txt\n+ok\n*** End Patch"},
        {"type":"custom_tool_call_output","id":"ctco_fixture","call_id":"call_fixture","output":"Success"}],
      "tools":[{"type":"custom","name":"apply_patch","format":{"type":"text"}}]});
    let result =
        super::super::super::chat_compatible_request::runtime_provider_chat_compatible_request_body(
            &serde_json::to_vec(&value).unwrap(),
            &super::super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore::default(),
            super::super::super::provider_bridge::RuntimeProviderBridgeKind::OpenAiResponses,
            "offrail-27b",
            false,
            super::super::super::deepseek_rewrite::RuntimeDeepSeekRewriteOptions::default(),
        );
    let translated =
        result.unwrap_or_else(|error| panic!("native reasoning replay rejected: {error:#}"));
    let chat: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    assert!(
        chat["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["role"] == "tool" && item["content"] == "Success")
    );
}
