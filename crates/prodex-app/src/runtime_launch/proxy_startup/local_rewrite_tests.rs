use super::local_rewrite::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken, RuntimeGatewayGuardrailWebhookConfig,
    RuntimeGatewayObservabilityConfig, RuntimeGatewayOidcConfig, RuntimeGatewaySsoConfig,
    RuntimeGatewayStateStore, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyStartOptions, start_runtime_gateway_rewrite_proxy,
    start_runtime_local_rewrite_proxy,
};
use super::local_rewrite_copilot::RuntimeCopilotProviderAuth;
use super::local_rewrite_kiro::RuntimeKiroProfileAuth;
use crate::AppState;
use std::fs;
use std::path::Path;
use std::time::Duration;
mod deepseek;
mod gateway_admin_auth;
mod gateway_admin_crud;
mod gateway_admin_governance_headers;
mod gateway_admin_ledger_query;
mod gateway_admin_scope;
mod gateway_admin_tenant_scope;
mod gateway_application_boundary;
mod gateway_grouped_budget;
mod gateway_health;
mod gateway_state;
mod gateway_usage;
mod model_memory;
mod provider_routes;
mod request_constraints;
mod support;
use support::*;

fn write_fake_kiro_runtime_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-runtime");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
prompt = third["params"]["prompt"][0]["text"]
assert "hello from prodex" in prompt
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"kiro says hi"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
    .expect("fake kiro runtime agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

fn write_fake_kiro_runtime_agent_no_prompt_assert(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-runtime-noassert");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"kiro says hi"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
    .expect("fake kiro runtime agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

fn write_fake_kiro_streaming_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-streaming");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys, time
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"kiro says hi"}}}}), flush=True)
time.sleep(0.3)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
    .expect("fake kiro streaming agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

fn write_fake_kiro_continuity_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-continuity");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
prompt = third["params"]["prompt"][0]["text"]
assert third["method"] == "session/prompt"
if "follow up" in prompt:
    assert "kiro says hi" in prompt, prompt
    text = "second turn"
else:
    assert "hello from prodex" in prompt, prompt
    text = "kiro says hi"
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":text}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
    .expect("fake kiro continuity agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

fn write_fake_kiro_tool_continuity_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-tool-continuity");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
prompt = third["params"]["prompt"][0]["text"]
assert third["method"] == "session/prompt"
if "Tool:\nok" in prompt:
    assert "Tool call read_file: {\"path\":\"/tmp/main.py\"}" in prompt, prompt
    print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_2","content":{"type":"text","text":"done after tool"}}}}), flush=True)
else:
    assert "start tool" in prompt, prompt
    print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","status":"completed","kind":"read","rawInput":{"path":"/tmp/main.py"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
    .expect("fake kiro tool continuity agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

fn write_fake_kiro_streaming_tool_update_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-streaming-tool-update");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys, time
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","status":"pending","kind":"read"}}}), flush=True)
time.sleep(0.05)
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call_update","toolCallId":"call_1","status":"completed","rawInput":{"path":"/tmp/main.py"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
    .expect("fake kiro streaming tool update agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

#[test]
fn kiro_responses_route_returns_translated_buffered_response() {
    let root = temp_root("kiro-responses");
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
    let fake_agent = write_fake_kiro_runtime_agent_no_prompt_assert(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
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
        .expect("kiro responses request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("response JSON should parse");
    assert_eq!(body["object"], "response");
    assert_eq!(body["model"], "claude-sonnet-4");
    assert_eq!(body["output"][0]["type"], "message");
    assert_eq!(body["output"][0]["content"][0]["text"], "kiro says hi");
    assert_eq!(body["metadata"]["kiro"]["stop_reason"], "end_turn");
    assert_eq!(body["metadata"]["kiro"]["profile_name"], "kiro-main");
}

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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
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

    let second: serde_json::Value = reqwest::blocking::Client::new()
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
        .expect("second kiro request should be sent")
        .json()
        .expect("second response JSON should parse");

    assert_eq!(second["output"][0]["content"][0]["text"], "second turn");
}

#[test]
fn kiro_responses_route_reuses_tool_history_by_call_id() {
    let root = temp_root("kiro-responses-tool-continuity");
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
    let fake_agent = write_fake_kiro_tool_continuity_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
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
                "content": [{"type": "input_text", "text": "start tool"}]
            }]
        }))
        .send()
        .expect("first tool continuity request should be sent")
        .json()
        .expect("first response JSON should parse");
    assert_eq!(first["output"][0]["type"], "function_call");
    assert_eq!(first["output"][0]["call_id"], "call_1");

    let second: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "ok"
            }]
        }))
        .send()
        .expect("second tool continuity request should be sent")
        .json()
        .expect("second response JSON should parse");

    assert_eq!(second["output"][0]["content"][0]["text"], "done after tool");
}

#[test]
fn kiro_responses_route_supports_buffered_sse_streaming() {
    let root = temp_root("kiro-responses-streaming");
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
    let fake_agent = write_fake_kiro_streaming_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let mut response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": true,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello from prodex"}]
            }]
        }))
        .send()
        .expect("kiro streaming request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream; charset=utf-8")
    );
    let mut first_chunk = [0_u8; 512];
    let first_read = std::io::Read::read(&mut response, &mut first_chunk)
        .expect("first stream chunk should be readable");
    let first_body = String::from_utf8_lossy(&first_chunk[..first_read]);
    assert!(first_body.contains("event: response.created"));
    assert!(!first_body.contains("\"type\":\"response.completed\""));
    assert!(!first_body.contains("data: [DONE]"));

    let mut rest = String::new();
    std::io::Read::read_to_string(&mut response, &mut rest)
        .expect("remaining stream body should be readable");
    let body = format!("{first_body}{rest}");
    assert!(body.contains("\"type\":\"response.output_item.added\""));
    assert!(body.contains("\"type\":\"response.output_text.delta\""));
    assert!(body.contains("\"type\":\"response.output_item.done\""));
    assert!(body.contains("\"type\":\"response.completed\""));
    assert!(body.contains("kiro says hi"));
    assert!(body.contains("data: [DONE]"));
}

#[test]
fn kiro_remote_compact_uses_kiro_semantic_summary_when_available() {
    let root = temp_root("kiro-compact");
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
    let fake_agent = write_fake_kiro_runtime_agent_no_prompt_assert(&root);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Kiro {
            auth: RuntimeKiroProfileAuth {
                profile_name: "kiro-main".to_string(),
                codex_home,
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses/compact", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "keep implementing parity"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "read_file",
                    "arguments": "{\"path\":\"src/main.rs\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "fn main() {}"
                }
            ]
        }))
        .send()
        .expect("kiro compact request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("compact response JSON should parse");
    let summary = body["output"][0]["content"][0]["text"]
        .as_str()
        .expect("compact summary text should exist");
    assert!(summary.contains("kiro says hi"));
    assert!(!summary.contains("Local Gemini compact fallback summary."));
    assert!(!summary.contains("tool call read_file (call_1)"));
}

#[test]
fn kiro_models_route_does_not_fallback_to_openai_catalog_when_snapshot_is_empty() {
    let root = temp_root("kiro-models-empty");
    let paths = app_paths_for_root(root.clone());
    let codex_home = root.join("kiro-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Kiro {
            auth: RuntimeKiroProfileAuth {
                profile_name: "kiro-main".to_string(),
                codex_home,
                model_catalog: Vec::new(),
                command: None,
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let client = reqwest::blocking::Client::new();
    let list = client
        .get(format!("http://{}/v1/models", proxy.listen_addr))
        .send()
        .expect("kiro models list should be sent");
    assert_eq!(list.status().as_u16(), 200);
    let list_body: serde_json::Value = list.json().expect("models list JSON should parse");
    assert_eq!(list_body["object"], "list");
    assert_eq!(
        list_body["data"]
            .as_array()
            .expect("models data should be an array"),
        &Vec::<serde_json::Value>::new()
    );

    let model = client
        .get(format!("http://{}/v1/models/gpt-5.4", proxy.listen_addr))
        .send()
        .expect("kiro single model should be sent");
    assert_eq!(model.status().as_u16(), 404);
    let model_body: serde_json::Value = model.json().expect("single model JSON should parse");
    assert_eq!(model_body["error"]["code"], "model_not_found");
}

#[test]
fn kiro_chat_completions_route_reuses_responses_translation_surface() {
    let root = temp_root("kiro-chat-completions");
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
    let fake_agent = write_fake_kiro_streaming_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("response JSON should parse");
    assert_eq!(body["object"], "chat.completion");
    assert_eq!(body["choices"][0]["message"]["content"], "kiro says hi");
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
    assert_eq!(body["metadata"]["kiro"]["profile_name"], "kiro-main");
}

#[test]
fn kiro_messages_route_reuses_anthropic_translation_surface() {
    let root = temp_root("kiro-messages");
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
    let fake_agent = write_fake_kiro_streaming_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/messages", proxy.listen_addr))
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": false,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro messages request should be sent");
    let status = response.status().as_u16();
    let body_text = response.text().expect("response body should read");
    assert_eq!(status, 200, "{body_text}");
    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("response JSON should parse");
    assert_eq!(body["type"], "message");
    assert_eq!(body["role"], "assistant");
    assert_eq!(body["content"][0]["type"], "text");
    assert_eq!(body["content"][0]["text"], "kiro says hi");
}

#[test]
fn kiro_messages_route_preserves_anthropic_tool_use_blocks() {
    let root = temp_root("kiro-messages-tool-use");
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
    let fake_agent = write_fake_kiro_streaming_tool_update_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/messages", proxy.listen_addr))
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": false,
            "messages": [{
                "role": "user",
                "content": "start tool"
            }]
        }))
        .send()
        .expect("kiro messages tool-use request should be sent");
    let status = response.status().as_u16();
    let body_text = response.text().expect("response body should read");
    assert_eq!(status, 200, "{body_text}");
    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("response JSON should parse");
    assert_eq!(body["type"], "message");
    assert_eq!(body["stop_reason"], "tool_use");
    assert_eq!(body["content"][0]["type"], "tool_use");
    assert_eq!(body["content"][0]["id"], "call_1");
    assert_eq!(body["content"][0]["name"], "read_file");
    assert_eq!(body["content"][0]["input"]["path"], "/tmp/main.py");
}

#[test]
fn kiro_messages_route_replays_anthropic_tool_result_followup() {
    let root = temp_root("kiro-messages-tool-result");
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
    let fake_agent = write_fake_kiro_tool_continuity_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let first: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/messages", proxy.listen_addr))
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": false,
            "messages": [{
                "role": "user",
                "content": "start tool"
            }]
        }))
        .send()
        .expect("first kiro messages request should be sent")
        .json()
        .expect("first kiro messages response should parse");
    assert_eq!(first["stop_reason"], "tool_use");
    assert_eq!(first["content"][0]["type"], "tool_use");

    let second: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/messages", proxy.listen_addr))
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": false,
            "messages": [{
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "read_file",
                    "input": {"path": "/tmp/main.py"}
                }]
            }, {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "call_1",
                    "content": "ok"
                }]
            }]
        }))
        .send()
        .expect("second kiro messages request should be sent")
        .json()
        .expect("second kiro messages response should parse");

    assert_eq!(second["type"], "message");
    assert_eq!(second["role"], "assistant");
    assert_eq!(second["content"][0]["type"], "text");
    assert_eq!(second["content"][0]["text"], "done after tool");
}

#[test]
fn kiro_messages_route_streams_anthropic_sse() {
    let root = temp_root("kiro-messages-stream");
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
    let fake_agent = write_fake_kiro_streaming_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let body = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/messages", proxy.listen_addr))
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "max_tokens": 128,
            "stream": true,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro messages streaming request should be sent")
        .text()
        .expect("kiro messages streaming body should read");

    assert!(body.contains("event: message_start"), "{body}");
    assert!(body.contains("event: content_block_delta"), "{body}");
    assert!(body.contains("kiro says hi"), "{body}");
    assert!(body.contains("event: message_stop"), "{body}");
}

#[test]
fn kiro_chat_completions_route_accepts_legacy_functions_and_function_call() {
    let root = temp_root("kiro-chat-legacy-functions");
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
    let fake_agent = write_fake_kiro_tool_continuity_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let first: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "function_call": {"name": "read_file"},
            "functions": [{
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            }],
            "messages": [{
                "role": "user",
                "content": "start tool"
            }]
        }))
        .send()
        .expect("legacy chat functions request should be sent")
        .json()
        .expect("legacy chat response JSON should parse");

    assert_eq!(first["object"], "chat.completion");
    assert_eq!(first["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(
        first["choices"][0]["message"]["tool_calls"][0]["id"],
        "call_1"
    );
    assert_eq!(
        first["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "read_file"
    );
}

#[test]
fn kiro_chat_completions_route_rejects_unsupported_response_format() {
    let root = temp_root("kiro-chat-response-format");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "answer",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "answer": {"type": "string"}
                        }
                    }
                }
            },
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_response_format");
}

#[test]
fn kiro_chat_completions_route_rejects_multiple_choices() {
    let root = temp_root("kiro-chat-choice-count");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "n": 2,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_choice_count");
}

#[test]
fn kiro_chat_completions_route_rejects_stop_sequences() {
    let root = temp_root("kiro-chat-stop");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "stop": ["DONE"],
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_stop");
}

#[test]
fn kiro_chat_completions_route_rejects_temperature() {
    let root = temp_root("kiro-chat-temperature");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "temperature": 0.2,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_temperature");
}

#[test]
fn kiro_chat_completions_route_rejects_top_p() {
    let root = temp_root("kiro-chat-top-p");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "top_p": 0.9,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_top_p");
}

#[test]
fn kiro_chat_completions_route_rejects_presence_penalty() {
    let root = temp_root("kiro-chat-presence-penalty");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "presence_penalty": 0.5,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_presence_penalty");
}

#[test]
fn kiro_chat_completions_route_rejects_frequency_penalty() {
    let root = temp_root("kiro-chat-frequency-penalty");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "frequency_penalty": 0.5,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_frequency_penalty");
}

#[test]
fn kiro_chat_completions_route_rejects_seed() {
    let root = temp_root("kiro-chat-seed");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "seed": 7,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_seed");
}

#[test]
fn kiro_chat_completions_route_tolerates_parallel_tool_calls_true() {
    let root = temp_root("kiro-chat-parallel-tool-calls");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "parallel_tool_calls": true,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("response JSON should parse");
    assert_eq!(body["object"], "chat.completion");
}

#[test]
fn kiro_chat_completions_route_ignores_user_metadata() {
    let root = temp_root("kiro-chat-user");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "user": "user-123",
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("response JSON should parse");
    assert_eq!(body["object"], "chat.completion");
}

#[test]
fn kiro_chat_completions_route_ignores_token_limit_controls() {
    let root = temp_root("kiro-chat-token-limit");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "max_tokens": 64,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("response JSON should parse");
    assert_eq!(body["object"], "chat.completion");
}

#[test]
fn kiro_chat_completions_route_rejects_invalid_token_limit_controls() {
    let root = temp_root("kiro-chat-token-limit-invalid");
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
    let fake_agent = write_fake_kiro_runtime_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": false,
            "max_tokens": 0,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat completions request should be sent");
    assert_eq!(response.status().as_u16(), 400);
    let body: serde_json::Value = response.json().expect("error JSON should parse");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "unsupported_token_limit");
}

#[test]
fn kiro_chat_completions_stream_translates_to_chat_chunks() {
    let root = temp_root("kiro-chat-stream");
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
    let fake_agent = write_fake_kiro_streaming_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": true,
            "messages": [{
                "role": "user",
                "content": "hello from prodex"
            }]
        }))
        .send()
        .expect("kiro chat stream request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body = response
        .text()
        .expect("chat stream body should be readable");
    assert!(body.contains("\"object\":\"chat.completion.chunk\""));
    assert!(body.contains("\"role\":\"assistant\""));
    assert!(body.contains("\"content\":\"kiro says hi\""));
    assert!(body.contains("\"finish_reason\":\"stop\""));
    assert!(body.contains("data: [DONE]"));
    assert!(!body.contains("event: response.created"));
    assert!(!body.contains("\"type\":\"response.completed\""));
}

#[test]
fn kiro_chat_completions_stream_emits_tool_calls() {
    let root = temp_root("kiro-chat-stream-tool");
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
    let fake_agent = write_fake_kiro_streaming_tool_update_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": true,
            "function_call": {"name": "read_file"},
            "functions": [{
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            }],
            "messages": [{
                "role": "user",
                "content": "start tool"
            }]
        }))
        .send()
        .expect("kiro chat tool stream request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body = response
        .text()
        .expect("chat tool stream body should be readable");
    assert!(body.contains("\"object\":\"chat.completion.chunk\""));
    assert!(body.contains("\"tool_calls\":[{"));
    assert!(body.contains("\"id\":\"call_1\""));
    assert!(body.contains("\"name\":\"read_file\""));
    assert!(body.contains("\"arguments\":"));
    assert!(body.contains("/tmp/main.py"));
    assert!(body.contains("\"finish_reason\":\"tool_calls\""));
    assert!(body.contains("data: [DONE]"));
    assert!(!body.contains("\"type\":\"response.function_call_arguments.delta\""));
}

#[test]
fn kiro_streaming_emits_tool_argument_delta_from_tool_call_update() {
    let root = temp_root("kiro-streaming-tool-update");
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
    let fake_agent = write_fake_kiro_streaming_tool_update_agent(&root);
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
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("kiro local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4",
            "stream": true,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "start tool"}]
            }]
        }))
        .send()
        .expect("kiro streaming request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("stream body should be readable");
    assert!(body.contains("\"type\":\"response.function_call_arguments.delta\""));
    assert!(body.contains("\"delta\":"));
    assert!(body.contains("/tmp/main.py"));
    assert!(body.contains("\"type\":\"response.output_item.done\""));
}

#[test]
fn copilot_transport_uses_copilot_api_headers_for_chat_completions() {
    let root = temp_root("copilot-chat-headers");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Profiles {
                profiles: vec![super::local_rewrite_copilot::RuntimeCopilotProfileAuth {
                    profile_name: "copilot-business".to_string(),
                    api_key: "copilot-runtime-token".to_string(),
                    api_url: format!("http://{}", upstream.addr),
                    model_catalog: Vec::new(),
                }],
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("copilot local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .header(reqwest::header::USER_AGENT, "codex-cli/0.1-test")
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .header("tracestate", "prodex=test")
        .header("baggage", "tenant_tier=premium")
        .json(&serde_json::json!({
            "model": "codex",
            "stream": false,
            "messages": [
                {"role": "user", "content": "run a tool"},
                {"role": "assistant", "content": "done"}
            ]
        }))
        .send()
        .expect("copilot chat request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive Copilot headers");
    let header = |name: &str| {
        headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    };
    assert_eq!(
        header("authorization"),
        Some("Bearer copilot-runtime-token")
    );
    assert_eq!(
        header("user-agent"),
        Some("copilot/1.0.65 (client/github/cli)")
    );
    assert_eq!(
        header("copilot-integration-id"),
        Some("copilot-developer-cli")
    );
    assert_eq!(header("editor-plugin-version"), None);
    assert_eq!(header("openai-intent"), Some("conversation-panel"));
    assert_eq!(header("x-github-api-version"), Some("2025-04-01"));
    assert_eq!(header("x-vscode-user-agent-library-version"), None);
    assert_eq!(header("x-initiator"), Some("agent"));
    let request_id = header("x-request-id")
        .and_then(|id| id.strip_prefix("prodex-"))
        .expect("Copilot request ID should keep prodex prefix");
    assert_eq!(
        request_id
            .parse::<prodex_domain::RequestId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_eq!(
        header("traceparent"),
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    );
    assert_eq!(header("tracestate"), Some("prodex=test"));
    assert_eq!(header("baggage"), Some("tenant_tier=premium"));
}

#[test]
fn openai_compatible_transport_preserves_trace_context_for_chat_completions() {
    let root = temp_root("openai-compatible-trace-context");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("OpenAI-compatible local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .header(reqwest::header::USER_AGENT, "codex-cli/0.1-test")
        .header("authorization", "Bearer caller-token")
        .header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .header("tracestate", "prodex=test")
        .header("baggage", "tenant_tier=premium")
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "stream": false,
            "messages": [{"role": "user", "content": "trace me"}]
        }))
        .send()
        .expect("OpenAI-compatible chat request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let headers = upstream
        .headers_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive OpenAI-compatible headers");
    let header = |name: &str| {
        headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    };
    assert_eq!(header("authorization"), Some("Bearer upstream-key"));
    assert_eq!(header("user-agent"), Some("codex-cli/0.1-test"));
    assert_eq!(
        header("traceparent"),
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    );
    assert_eq!(header("tracestate"), Some("prodex=test"));
    assert_eq!(header("baggage"), Some("tenant_tier=premium"));
}

#[test]
fn copilot_chat_completions_strip_encrypted_reasoning_content() {
    let root = temp_root("copilot-chat-encrypted-content");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Profiles {
                profiles: vec![super::local_rewrite_copilot::RuntimeCopilotProfileAuth {
                    profile_name: "copilot-business".to_string(),
                    api_key: "copilot-runtime-token".to_string(),
                    api_url: format!("http://{}", upstream.addr),
                    model_catalog: Vec::new(),
                }],
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("copilot local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/chat/completions", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "codex",
            "stream": false,
            "messages": [
                {"role": "user", "content": "hello"},
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [],
                    "reasoning": {
                        "id": "rs_1",
                        "encrypted_content": "qAAA.VDK="
                    }
                }
            ]
        }))
        .send()
        .expect("copilot chat request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive Copilot chat body"),
    )
    .unwrap();
    let messages = body["messages"]
        .as_array()
        .expect("messages should be an array");
    assert!(messages.iter().all(|message| {
        message
            .get("reasoning")
            .and_then(|reasoning| reasoning.get("encrypted_content"))
            .is_none()
    }));
}

#[test]
fn copilot_responses_route_preserves_responses_endpoint() {
    let root = temp_root("copilot-responses-native");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Profiles {
                profiles: vec![super::local_rewrite_copilot::RuntimeCopilotProfileAuth {
                    profile_name: "copilot-business".to_string(),
                    api_key: "copilot-runtime-token".to_string(),
                    api_url: format!("http://{}", upstream.addr),
                    model_catalog: Vec::new(),
                }],
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("copilot local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "gpt-5.3-codex",
            "stream": false,
            "input": [
                {
                    "type": "reasoning",
                    "id": "rs_1",
                    "encrypted_content": "qAAA.VDK="
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "hello"},
                        {
                            "type": "input_image",
                            "image_url": "data:image/png;base64,iVBORw0KGgo="
                        }
                    ]
                }
            ]
        }))
        .send()
        .expect("copilot responses request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let path = upstream
        .path_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive Copilot request path");
    assert_eq!(path, "/responses");
    let body: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive Copilot request body"),
    )
    .unwrap();
    assert_eq!(body["model"], "gpt-5.3-codex");
    assert_eq!(body["input"][0]["type"], "reasoning");
    assert!(body["input"][0].get("encrypted_content").is_none());
    assert_eq!(body["input"][1]["content"][0]["text"], "hello");
    assert_eq!(
        body["input"][1]["content"][1]["image_url"],
        "data:image/png;base64,iVBORw0KGgo="
    );
}

#[test]
fn copilot_responses_route_preserves_codex_tool_surface() {
    let root = temp_root("copilot-responses-tools");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Profiles {
                profiles: vec![super::local_rewrite_copilot::RuntimeCopilotProfileAuth {
                    profile_name: "copilot-business".to_string(),
                    api_key: "copilot-runtime-token".to_string(),
                    api_url: format!("http://{}", upstream.addr),
                    model_catalog: Vec::new(),
                }],
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("copilot local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "gpt-5.3-codex",
            "stream": false,
            "previous_response_id": "resp_previous",
            "tool_choice": "auto",
            "tools": [
                {
                    "type": "web_search_preview",
                    "search_context_size": "medium"
                },
                {
                    "type": "function",
                    "name": "shell",
                    "description": "Run a shell command.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "cmd": {"type": "string"}
                        },
                        "required": ["cmd"],
                        "additionalProperties": false
                    }
                },
                {
                    "type": "mcp_tool",
                    "name": "mcp__prodex_sqz__compress",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "text": {"type": "string"}
                        },
                        "required": ["text"]
                    }
                },
                {
                    "type": "custom",
                    "name": "apply_patch",
                    "description": "Apply a patch with raw grammar input."
                }
            ],
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "use tools"}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_shell",
                    "output": "ok"
                },
                {
                    "type": "mcp_call_output",
                    "call_id": "call_sqz",
                    "output": "compressed"
                }
            ]
        }))
        .send()
        .expect("copilot responses tool request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let path = upstream
        .path_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive Copilot request path");
    assert_eq!(path, "/responses");
    let body: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive Copilot request body"),
    )
    .unwrap();

    assert_eq!(body["model"], "gpt-5.3-codex");
    assert_eq!(body["previous_response_id"], "resp_previous");
    assert_eq!(body["tool_choice"], "auto");
    assert_eq!(body["tools"][0]["type"], "web_search_preview");
    assert_eq!(body["tools"][1]["name"], "shell");
    assert_eq!(body["tools"][1]["parameters"]["required"][0], "cmd");
    assert_eq!(body["tools"][2]["type"], "mcp_tool");
    assert_eq!(body["tools"][2]["name"], "mcp__prodex_sqz__compress");
    assert_eq!(body["tools"][3]["type"], "custom");
    assert_eq!(body["tools"][3]["name"], "apply_patch");
    assert_eq!(body["input"][1]["type"], "function_call_output");
    assert_eq!(body["input"][2]["type"], "mcp_call_output");
}

#[test]
fn copilot_responses_compact_route_forwards_upstream() {
    let root = temp_root("copilot-responses-compact");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start();
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: "http://127.0.0.1:9".to_string(),
        provider: RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Profiles {
                profiles: vec![super::local_rewrite_copilot::RuntimeCopilotProfileAuth {
                    profile_name: "copilot-business".to_string(),
                    api_key: "copilot-runtime-token".to_string(),
                    api_url: format!("http://{}", upstream.addr),
                    model_catalog: Vec::new(),
                }],
            },
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("copilot local rewrite proxy should start");

    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses/compact", proxy.listen_addr))
        .json(&serde_json::json!({
            "model": "gpt-5.3-codex",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "compact this"}]
            }]
        }))
        .send()
        .expect("copilot compact request should be sent");
    assert_eq!(response.status().as_u16(), 200);

    let path = upstream
        .path_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive Copilot compact path");
    assert_eq!(path, "/responses/compact");
    let body: serde_json::Value = serde_json::from_slice(
        &upstream
            .body_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upstream should receive Copilot compact body"),
    )
    .unwrap();
    assert_eq!(body["model"], "gpt-5.3-codex");
    assert_eq!(body["input"][0]["content"][0]["text"], "compact this");
}

#[test]
fn gateway_admin_viewer_token_can_read_but_not_mutate_keys() {
    let root = temp_root("gateway-admin-rbac");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let admin_token = "admin-token";
    let viewer_token = "viewer-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: None,
        gateway_admin_tokens: vec![
            RuntimeGatewayAdminToken {
                name: "admin".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    admin_token,
                ),
                role: RuntimeGatewayAdminRole::Admin,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
            RuntimeGatewayAdminToken {
                name: "viewer".to_string(),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    viewer_token,
                ),
                role: RuntimeGatewayAdminRole::Viewer,
                allowed_key_prefixes: Vec::new(),
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
            },
        ],
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    let admin_data_plane = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"model": "gpt-5.4", "input": "admin token"}))
        .send()
        .expect("admin token data-plane request should be sent");
    assert_eq!(admin_data_plane.status().as_u16(), 401);

    let viewed = client
        .get(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .send()
        .expect("viewer list keys request should be sent");
    assert_eq!(viewed.status().as_u16(), 200);

    let viewer_create = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .json(&serde_json::json!({"name": "team-rbac"}))
        .send()
        .expect("viewer create key request should be sent");
    assert_eq!(viewer_create.status().as_u16(), 403);
    assert_eq!(
        viewer_create.json::<serde_json::Value>().unwrap()["error"]["code"],
        "gateway_admin_role_forbidden"
    );

    let created = client
        .post(format!(
            "http://{}/v1/prodex/gateway/keys",
            proxy.listen_addr
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "team-rbac"}))
        .send()
        .expect("admin create key request should be sent");
    assert_eq!(created.status().as_u16(), 201);

    let viewer_patch = client
        .patch(format!(
            "http://{}/v1/prodex/gateway/keys/team-rbac",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .expect("viewer patch key request should be sent");
    assert_eq!(viewer_patch.status().as_u16(), 403);

    let viewer_delete = client
        .delete(format!(
            "http://{}/v1/prodex/gateway/keys/team-rbac",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .send()
        .expect("viewer delete key request should be sent");
    assert_eq!(viewer_delete.status().as_u16(), 403);

    let metrics = client
        .get(format!(
            "http://{}/v1/prodex/gateway/metrics",
            proxy.listen_addr
        ))
        .bearer_auth(viewer_token)
        .send()
        .expect("viewer metrics request should be sent");
    assert_eq!(metrics.status().as_u16(), 200);
}

#[test]
fn gateway_guardrail_pii_redaction_rewrites_request_before_upstream() {
    let root = temp_root("gateway-pii-redaction");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let gateway_token = "gateway-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            pii_redaction: true,
            ..runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default()
        },
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "gpt-5.4",
            "input": "email alice@example.com card 4111-1111-1111-1111"
        }))
        .send()
        .expect("gateway request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let upstream_body = upstream
        .body_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("upstream should receive redacted request");
    let upstream_body = String::from_utf8(upstream_body).expect("upstream body should be utf8");
    assert!(upstream_body.contains("<redacted>"));
    assert!(!upstream_body.contains("alice@example.com"));
    assert!(!upstream_body.contains("4111-1111-1111-1111"));
}

#[test]
fn local_embeddings_only_serves_embeddings_and_rejects_generation() {
    let root = temp_root("local-embeddings-only");
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "gateway-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::LocalEmbeddingsOnly {
            embedding_model: "text-embedding-3-small".to_string(),
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");
    let client = reqwest::blocking::Client::new();

    let embeddings = client
        .post(format!("http://{}/v1/embeddings", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "text-embedding-3-small",
            "input": ["hello world", "hello world"]
        }))
        .send()
        .expect("embedding request should be sent");
    assert_eq!(embeddings.status().as_u16(), 200);
    let embeddings: serde_json::Value = embeddings.json().expect("embedding response is JSON");
    assert_eq!(embeddings["object"], "list");
    assert_eq!(embeddings["model"], "text-embedding-3-small");
    let first = embeddings["data"][0]["embedding"]
        .as_array()
        .expect("embedding should be an array");
    let second = embeddings["data"][1]["embedding"]
        .as_array()
        .expect("embedding should be an array");
    assert_eq!(first.len(), 1536);
    assert_eq!(first, second);

    let generation = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "gpt-4.1-nano",
            "input": "should not be forwarded"
        }))
        .send()
        .expect("generation request should be sent");
    assert_eq!(generation.status().as_u16(), 503);
    let generation: serde_json::Value = generation.json().expect("error response is JSON");
    assert_eq!(generation["error"]["code"], "local_embeddings_only");
}

#[test]
fn gateway_request_guardrail_blocked_keywords_are_audited_without_token_leakage() {
    let root = temp_root("gateway-request-guardrail-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "guardrail-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            blocked_keywords: vec!["do-not-send".to_string()],
            blocked_output_keywords: Vec::new(),
            allowed_models: Vec::new(),
            prompt_injection_detection: false,
            pii_redaction: false,
        },
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let client = reqwest::blocking::Client::new();
    let denied = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"please do-not-send this"}))
        .send()
        .expect("guardrail-denied request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "blocked_keyword");

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("guardrail denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"guardrail_blocked""#));
    assert!(audit_log.contains(r#""reason":"blocked_keyword""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains("do-not-send"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_guardrail_blocked"));
    assert!(runtime_log.contains("matched_value_redacted"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains("do-not-send"));
}

#[test]
fn gateway_response_guardrail_blocked_output_is_audited_without_token_or_value_leakage() {
    let root = temp_root("gateway-response-guardrail-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_with_response_body(
        r#"{"id":"resp_test","output":[{"content":"do-not-emit-sensitive-marker"}],"usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#,
    );
    let gateway_token = "response-guardrail-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            blocked_keywords: Vec::new(),
            blocked_output_keywords: vec!["do-not-emit-sensitive-marker".to_string()],
            allowed_models: Vec::new(),
            prompt_injection_detection: false,
            pii_redaction: false,
        },
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })
    .expect("gateway proxy should start");

    let denied = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("response-guardrail request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "policy_violation");

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("response guardrail denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"response_guardrail_blocked""#));
    assert!(audit_log.contains(r#""reason":"blocked_output_keyword""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains("do-not-emit-sensitive-marker"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_guardrail_response_blocked"));
    assert!(runtime_log.contains("matched_value_redacted"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains("do-not-emit-sensitive-marker"));
}

#[test]
fn gateway_streaming_response_guardrail_blocked_output_is_audited_without_value_leakage() {
    let root = temp_root("gateway-stream-response-guardrail-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_with_response(
        "text/event-stream; charset=utf-8",
        concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"safe chunk\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"do-not-stream-sensitive-marker\"}\n\n",
            "data: [DONE]\n\n",
        ),
    );
    let gateway_token = "stream-guardrail-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
            blocked_keywords: Vec::new(),
            blocked_output_keywords: vec!["do-not-stream-sensitive-marker".to_string()],
            allowed_models: Vec::new(),
            prompt_injection_detection: false,
            pii_redaction: false,
        },
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("proxy should start");

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({
            "model": "gpt-5",
            "input": "hello",
            "stream": true,
        }))
        .send()
        .expect("streaming request should be sent");
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("stream body should be readable");
    assert!(!body.contains("do-not-stream-sensitive-marker"));

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("stream guardrail denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"response_guardrail_blocked""#));
    assert!(audit_log.contains(r#""reason":"blocked_output_keyword""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains("do-not-stream-sensitive-marker"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_guardrail_stream_blocked"));
    assert!(runtime_log.contains("matched_value_redacted"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains("do-not-stream-sensitive-marker"));
}

#[test]
fn gateway_pre_guardrail_webhook_denial_is_audited_without_secret_leakage() {
    let root = temp_root("gateway-pre-webhook-guardrail-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let webhook = TestGuardrailWebhook::start_deny("tenant_policy_denied");
    let gateway_token = "pre-webhook-gateway-token";
    let webhook_token = "webhook-secret-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
            url: Some(format!("http://{}", webhook.addr)),
            phases: vec!["pre".to_string()],
            bearer_token: Some(webhook_token.to_string()),
            fail_closed: true,
        },
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");

    let denied = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("webhook-denied request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "policy_violation");

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("webhook denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"guardrail_webhook_blocked""#));
    assert!(audit_log.contains(r#""phase":"pre""#));
    assert!(audit_log.contains(r#""reason":"tenant_policy_denied""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains(webhook_token));
    assert!(!audit_log.contains("do-not-log-webhook-message"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_guardrail_webhook_blocked"));
    assert!(runtime_log.contains("matched_value_redacted"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains(webhook_token));
    assert!(!runtime_log.contains("do-not-log-webhook-message"));
}

#[test]
fn gateway_post_guardrail_webhook_denial_is_audited_without_secret_leakage() {
    let root = temp_root("gateway-post-webhook-guardrail-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(1);
    let webhook = TestGuardrailWebhook::start_deny("output_policy_denied");
    let gateway_token = "post-webhook-gateway-token";
    let webhook_token = "post-webhook-secret-token";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
            url: Some(format!("http://{}", webhook.addr)),
            phases: vec!["post".to_string()],
            bearer_token: Some(webhook_token.to_string()),
            fail_closed: true,
        },
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");

    let denied = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("webhook-denied request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "policy_violation");

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("webhook response denial should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"response_guardrail_webhook_blocked""#));
    assert!(audit_log.contains(r#""phase":"post""#));
    assert!(audit_log.contains(r#""reason":"output_policy_denied""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains(webhook_token));
    assert!(!audit_log.contains("do-not-log-webhook-message"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_guardrail_webhook_blocked"));
    assert!(runtime_log.contains("matched_value_redacted"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains(webhook_token));
    assert!(!runtime_log.contains("do-not-log-webhook-message"));
}

#[test]
fn gateway_pre_guardrail_webhook_failure_redacts_url_and_is_audited() {
    let root = temp_root("gateway-pre-webhook-failure-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "pre-webhook-failure-gateway-token";
    let webhook_url_secret = "webhook-url-query-secret";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig {
            url: Some(format!(
                "http://127.0.0.1:9/deny?token={webhook_url_secret}"
            )),
            phases: vec!["pre".to_string()],
            bearer_token: Some("failure-webhook-bearer-secret".to_string()),
            fail_closed: true,
        },
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");

    let denied = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":"hello"}))
        .send()
        .expect("webhook-failure request should be sent");
    assert_eq!(denied.status().as_u16(), 403);
    let denied: serde_json::Value = denied.json().expect("denied response should be json");
    assert_eq!(denied["error"]["code"], "policy_violation");

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("webhook failure should be audited");
    assert!(audit_log.contains(r#""action":"guardrail_webhook_blocked""#));
    assert!(audit_log.contains(r#""phase":"pre""#));
    assert!(audit_log.contains(r#""reason":"webhook_error""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains(webhook_url_secret));
    assert!(!audit_log.contains("failure-webhook-bearer-secret"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("gateway_guardrail_webhook_failed"));
    assert!(runtime_log.contains("endpoint=redacted"));
    assert!(runtime_log.contains("error_kind="));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains(webhook_url_secret));
    assert!(!runtime_log.contains("failure-webhook-bearer-secret"));
    assert!(!runtime_log.contains("127.0.0.1:9/deny"));
}

#[test]
fn gateway_presidio_redaction_failure_is_audited_without_payload_or_endpoint_leakage() {
    let root = temp_root("gateway-presidio-redaction-failure-audit");
    let audit_dir = root.join("audit");
    std::fs::create_dir_all(&audit_dir).expect("audit dir should be created");
    let _audit_env = crate::TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.to_str().expect("audit dir should be utf8"),
    );
    let paths = app_paths_for_root(root);
    let upstream = TestUpstream::start_n(0);
    let gateway_token = "presidio-failure-gateway-token";
    let sensitive_input = "alice@example.com";
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &AppState::default(),
        upstream_base_url: format!("http://{}/v1", upstream.addr),
        provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["upstream-key".to_string()],
        },
        upstream_no_proxy: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: Some("127.0.0.1:0"),
        gateway_auth_token_hash: Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
            gateway_token,
        )),
        gateway_admin_tokens: Vec::new(),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(&paths),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_call_id_header: Some("x-prodex-call-id".to_string()),
    })
    .expect("gateway proxy should start");
    crate::register_runtime_presidio_redaction_proxy_state(
        &proxy.log_path,
        Some(crate::RuntimePresidioRedactionConfig {
            analyzer_url: "http://127.0.0.1:9/analyze?secret=presidio-endpoint-secret".to_string(),
            anonymizer_url: "http://127.0.0.1:9/anonymize?secret=presidio-endpoint-secret"
                .to_string(),
            languages: vec!["en".to_string()],
            language_mode: crate::PresidioLanguageMode::Fixed,
            fail_closed: true,
        }),
    );

    let rejected = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":sensitive_input}))
        .send()
        .expect("presidio failure request should be sent");
    assert_eq!(rejected.status().as_u16(), 502);
    let body = rejected.text().expect("rejection should be text");
    assert_eq!(body, "gateway PII redaction failed");
    assert!(
        upstream
            .body_rx
            .recv_timeout(Duration::from_millis(200))
            .is_err()
    );

    let audit_log = std::fs::read_to_string(audit_dir.join("prodex-audit.log"))
        .expect("presidio failure should be audited");
    assert!(audit_log.contains(r#""component":"gateway_data_plane""#));
    assert!(audit_log.contains(r#""action":"presidio_redaction_failed""#));
    assert!(audit_log.contains(r#""reason":"presidio_redaction_failed""#));
    assert!(audit_log.contains(r#""path":"/v1/responses""#));
    assert!(!audit_log.contains(gateway_token));
    assert!(!audit_log.contains(sensitive_input));
    assert!(!audit_log.contains("presidio-endpoint-secret"));
    assert!(!audit_log.contains("127.0.0.1:9"));

    let runtime_log =
        std::fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("local_rewrite_presidio_redaction_failed"));
    assert!(runtime_log.contains("reason=presidio_redaction_failed"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains(sensitive_input));
    assert!(!runtime_log.contains("presidio-endpoint-secret"));
    assert!(!runtime_log.contains("127.0.0.1:9"));
}
