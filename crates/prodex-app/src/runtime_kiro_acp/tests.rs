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
    crate::test_support::write_test_python_executable(
        root,
        "fake-kiro",
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
}

fn write_fake_kiro_prompt_agent(root: &Path) -> std::path::PathBuf {
    crate::test_support::write_test_python_executable(
        root,
        "fake-kiro-prompt",
        r#"#!/usr/bin/env python3
import json, os, sys
if os.environ.get("EXPECT_MODEL"):
    assert sys.argv[1:] == ["acp", "--model", "claude-sonnet-4.5", "--effort", "medium"]
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
if os.environ.get("SERVER_REQUEST"):
    print(json.dumps({"jsonrpc":"2.0","id":9,"method":"fs/read_text_file","params":{"path":"private.txt"}}), flush=True)
    rejection = json.loads(sys.stdin.readline())
    assert rejection["id"] == 9
    assert rejection["error"]["code"] == -32601
print(json.dumps({"jsonrpc":"2.0","method":"_kiro.dev/metadata","params":{"sessionId":"session-1","turnDurationMs":8}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"status":"completed"},"id":2}), flush=True)
if os.environ.get("LINGER_AFTER_RESPONSE"):
    import time
    time.sleep(5)
"#,
    )
}

fn write_fake_kiro_activity_agent(root: &Path) -> std::path::PathBuf {
    crate::test_support::write_test_python_executable(
        root,
        "fake-kiro-activity",
        r#"#!/usr/bin/env python3
import json, os, sys
count_path = os.path.join(os.getcwd(), "activity-agent-invocations")
with open(count_path, "a", encoding="utf-8") as count:
    count.write("1\n")
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":False,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"test"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-example","models":{"currentModelId":"model-example","availableModels":[{"modelId":"model-example","name":"model-example"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-example","update":{"sessionUpdate":"tool_call","toolCallId":"activity-example","title":"Read file","status":"in_progress","kind":"read","rawInput":{"path":"/home/test-user/private.txt"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-example","update":{"sessionUpdate":"agent_message_chunk","messageId":"message-example","content":{"type":"text","text":"final answer"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-example","update":{"sessionUpdate":"tool_call_update","toolCallId":"activity-example","status":"completed","rawOutput":{"path":"/home/test-user/private.txt"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"stopReason":"end_turn"},"id":2}), flush=True)
"#,
    )
}
