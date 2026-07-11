use super::*;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "tests_bootstrap.rs"]
mod tests_bootstrap;
#[path = "tests_turn.rs"]
mod tests_turn;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "prodex-kiro-acp-{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir should exist");
    dir
}

fn write_fake_kiro_acp_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":True,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login'."}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"_kiro.dev/subagent/list_update","params":{"subagents":[],"pendingStages":[]}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","modes":{"currentModeId":"kiro_default","availableModes":[{"id":"kiro_default","name":"kiro_default","description":"The default agent for Kiro CLI"}]},"models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"},{"modelId":"claude-sonnet-4.5","name":"claude-sonnet-4.5"}]}},"id":1}), flush=True)
"#,
    )
    .expect("fake agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}

fn write_fake_kiro_prompt_agent(root: &Path) -> std::path::PathBuf {
    let script = root.join("fake-kiro-prompt");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":True,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login'."}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
assert third["params"]["sessionId"] == "session-1"
assert third["params"]["prompt"][0]["text"] == "hello from prodex"
print(json.dumps({"jsonrpc":"2.0","method":"_kiro.dev/metadata","params":{"sessionId":"session-1","turnDurationMs":8}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"status":"completed"},"id":2}), flush=True)
"#,
    )
    .expect("fake prompt agent should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("permissions should update");
    }
    script
}
