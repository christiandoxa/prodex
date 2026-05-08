use crate::{
    CLAUDE_MEM_CODEX_SCHEMA_NAME, RUNTIME_MEM_SUPER_SLIM_ASSISTANT_OMITTED,
    RUNTIME_MEM_SUPER_SLIM_PROMPT_OMITTED, RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED,
    RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE, RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_INPUT,
    RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_NAME, RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
    RuntimeMemTranscriptMode,
};
use serde_json::Value;

pub fn runtime_mem_default_codex_schema() -> serde_json::Value {
    runtime_mem_slim_codex_schema()
}

pub fn runtime_mem_full_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.3",
        "description": "Full schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            { "name": "session-meta", "match": { "path": "type", "equals": "session_meta" }, "action": "session_context", "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" } },
            { "name": "turn-context", "match": { "path": "type", "equals": "turn_context" }, "action": "session_context", "fields": { "cwd": "payload.cwd" } },
            { "name": "user-message", "match": { "path": "payload.type", "equals": "user_message" }, "action": "session_init", "fields": { "prompt": "payload.message" } },
            {
                "name": "response-user-message",
                "match": { "path": "payload.role", "equals": "user" },
                "action": "session_init",
                "fields": {
                    "prompt": {
                        "coalesce": [
                            "payload.content[0].text",
                            "payload.content[1].text",
                            "payload.content[2].text",
                            "payload.content[3].text",
                            "payload.content[4].text",
                            "payload.message"
                        ]
                    }
                }
            },
            { "name": "assistant-message", "match": { "path": "payload.type", "equals": "agent_message" }, "action": "assistant_message", "fields": { "message": "payload.message" } },
            {
                "name": "response-assistant-message",
                "match": { "path": "payload.role", "equals": "assistant" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.content[0].text",
                            "payload.content[1].text",
                            "payload.content[2].text",
                            "payload.content[3].text",
                            "payload.content[4].text",
                            "payload.message"
                        ]
                    }
                }
            },
            {
                "name": "tool-use",
                "match": { "path": "payload.type", "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command", "local_shell_call"] },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": { "coalesce": ["payload.name", "payload.type", { "value": "web_search" }] },
                    "toolInput": { "coalesce": ["payload.arguments", "payload.input", "payload.command", "payload.action.command", "payload.action"] }
                }
            },
            {
                "name": "tool-result",
                "match": { "path": "payload.type", "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"] },
                "action": "tool_result",
                "fields": { "toolId": "payload.call_id", "toolResponse": "payload.output" }
            },
            { "name": "session-end", "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed", "turn_complete"] }, "action": "session_end" }
        ]
    })
}

pub fn runtime_mem_super_slim_codex_schema() -> serde_json::Value {
    let mut schema = runtime_mem_super_slim_v1_codex_schema();
    let Some(object) = schema.as_object_mut() else {
        return schema;
    };
    let legacy_events = object
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    object.insert(
        "version".to_string(),
        serde_json::json!("0.7-super-slim-v2"),
    );
    object.insert(
        "description".to_string(),
        serde_json::json!(
            "Prodex super-slim v2 schema for Codex session JSONL files under ~/.codex/sessions."
        ),
    );
    let mut events = vec![
        serde_json::json!({
            "name": "prodex-v2-user-message",
            "match": { "path": "t", "equals": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE },
            "action": "session_init",
            "fields": {
                "prompt": {
                    "coalesce": ["s", "r", RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD, { "value": RUNTIME_MEM_SUPER_SLIM_PROMPT_OMITTED }]
                }
            }
        }),
        serde_json::json!({
            "name": "prodex-v2-assistant-message",
            "match": { "path": "t", "equals": RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE },
            "action": "assistant_message",
            "fields": {
                "message": {
                    "coalesce": ["s", { "value": RUNTIME_MEM_SUPER_SLIM_ASSISTANT_OMITTED }]
                }
            }
        }),
        serde_json::json!({
            "name": "prodex-v2-tool-use",
            "match": { "path": "t", "equals": RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE },
            "action": "tool_use",
            "fields": {
                "toolId": "i",
                "toolName": { "coalesce": ["n", { "value": RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_NAME }] },
                "toolInput": { "coalesce": ["c", "n", { "value": RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_INPUT }] }
            }
        }),
        serde_json::json!({
            "name": "prodex-v2-tool-result",
            "match": { "path": "t", "equals": RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE },
            "action": "tool_result",
            "fields": {
                "toolId": "i",
                "toolResponse": {
                    "coalesce": ["s", "r", RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD, { "value": RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED }]
                }
            }
        }),
        serde_json::json!({
            "name": "prodex-v2-dictionary-entry",
            "match": { "path": "t", "equals": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE },
            "action": "session_context",
            "fields": {
                "dictionary": "s",
                "dictionaryKey": "k",
                "dictionaryIndex": "i",
                "dictionaryMode": "m",
                "dictionaryValue": "v"
            }
        }),
    ];
    events.extend(legacy_events);
    object.insert("events".to_string(), Value::Array(events));
    schema
}

pub fn runtime_mem_super_slim_v1_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.6-super-slim",
        "description": "Super-slim schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            { "name": "session-meta", "match": { "path": "type", "equals": "session_meta" }, "action": "session_context", "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" } },
            { "name": "turn-context", "match": { "path": "type", "equals": "turn_context" }, "action": "session_context", "fields": { "cwd": "payload.cwd" } },
            {
                "name": "user-message",
                "match": { "path": "payload.type", "equals": "user_message" },
                "action": "session_init",
                "fields": {
                    "prompt": {
                        "coalesce": [
                            "payload.prompt_summary",
                            "payload.metadata.prompt_summary",
                            "payload.metadata.artifact_ref",
                            "payload.metadata.artifact_id",
                            "payload.metadata.artifactId",
                            "payload.artifact.reference",
                            "payload.artifact.ref",
                            "payload.artifact.id",
                            "payload.artifact_id",
                            "payload.artifactId",
                            { "value": RUNTIME_MEM_SUPER_SLIM_PROMPT_OMITTED }
                        ]
                    }
                }
            },
            {
                "name": "response-user-message",
                "match": { "path": "payload.role", "equals": "user" },
                "action": "session_init",
                "fields": {
                    "prompt": {
                        "coalesce": [
                            "payload.prompt_summary",
                            "payload.metadata.prompt_summary",
                            "payload.metadata.artifact_ref",
                            "payload.metadata.artifact_id",
                            "payload.metadata.artifactId",
                            "payload.artifact.reference",
                            "payload.artifact.ref",
                            "payload.artifact.id",
                            "payload.artifact_id",
                            "payload.artifactId",
                            { "value": RUNTIME_MEM_SUPER_SLIM_PROMPT_OMITTED }
                        ]
                    }
                }
            },
            {
                "name": "assistant-message",
                "match": { "path": "payload.type", "equals": "agent_message" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.summary",
                            "payload.title",
                            { "value": RUNTIME_MEM_SUPER_SLIM_ASSISTANT_OMITTED }
                        ]
                    }
                }
            },
            {
                "name": "response-assistant-message",
                "match": { "path": "payload.role", "equals": "assistant" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.summary",
                            "payload.title",
                            { "value": RUNTIME_MEM_SUPER_SLIM_ASSISTANT_OMITTED }
                        ]
                    }
                }
            },
            {
                "name": "tool-use",
                "match": { "path": "payload.type", "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command", "local_shell_call"] },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": { "coalesce": ["payload.name", "payload.type", { "value": "web_search" }] },
                    "toolInput": { "coalesce": ["payload.command", "payload.action.command", "payload.action", "payload.name", { "value": "tool call" }] }
                }
            },
            {
                "name": "tool-result",
                "match": { "path": "payload.type", "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"] },
                "action": "tool_result",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolResponse": {
                        "coalesce": [
                            "payload.summary",
                            "payload.metadata.summary",
                            "payload.metadata.artifact_ref",
                            "payload.metadata.artifact_id",
                            "payload.metadata.artifactId",
                            "payload.artifact.reference",
                            "payload.artifact.ref",
                            "payload.artifact.id",
                            "payload.artifact_id",
                            "payload.artifactId",
                            { "value": RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED }
                        ]
                    }
                }
            },
            { "name": "session-end", "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed", "turn_complete"] }, "action": "session_end" }
        ]
    })
}

pub fn runtime_mem_codex_schema_for_mode(mode: RuntimeMemTranscriptMode) -> serde_json::Value {
    match mode {
        RuntimeMemTranscriptMode::Slim => runtime_mem_slim_codex_schema(),
        RuntimeMemTranscriptMode::SuperSlim => runtime_mem_super_slim_codex_schema(),
        RuntimeMemTranscriptMode::Full => runtime_mem_full_codex_schema(),
    }
}

fn runtime_mem_slim_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.4-slim",
        "description": "Slim schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            { "name": "session-meta", "match": { "path": "type", "equals": "session_meta" }, "action": "session_context", "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" } },
            { "name": "turn-context", "match": { "path": "type", "equals": "turn_context" }, "action": "session_context", "fields": { "cwd": "payload.cwd" } },
            { "name": "user-message", "match": { "path": "payload.type", "equals": "user_message" }, "action": "session_init", "fields": { "prompt": "payload.message" } },
            {
                "name": "response-user-message",
                "match": { "path": "payload.role", "equals": "user" },
                "action": "session_init",
                "fields": {
                    "prompt": {
                        "coalesce": [
                            "payload.content[0].text",
                            "payload.content[1].text",
                            "payload.content[2].text",
                            "payload.content[3].text",
                            "payload.content[4].text",
                            "payload.message"
                        ]
                    }
                }
            },
            {
                "name": "assistant-message",
                "match": { "path": "payload.type", "equals": "agent_message" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.summary",
                            "payload.title",
                            { "value": "assistant response recorded by prodex slim mem" }
                        ]
                    }
                }
            },
            {
                "name": "response-assistant-message",
                "match": { "path": "payload.role", "equals": "assistant" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.summary",
                            "payload.title",
                            "payload.content[0].text",
                            "payload.content[1].text",
                            "payload.content[2].text",
                            "payload.content[3].text",
                            "payload.content[4].text",
                            { "value": "assistant response recorded by prodex slim mem" }
                        ]
                    }
                }
            },
            {
                "name": "tool-use",
                "match": { "path": "payload.type", "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command", "local_shell_call"] },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": { "coalesce": ["payload.name", "payload.type", { "value": "web_search" }] },
                    "toolInput": { "coalesce": ["payload.command", "payload.action.command", "payload.action", "payload.name", { "value": "tool call" }] }
                }
            },
            {
                "name": "tool-result",
                "match": { "path": "payload.type", "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"] },
                "action": "tool_result",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolResponse": {
                        "coalesce": [
                            "payload.summary",
                            "payload.metadata.summary",
                            { "value": "tool result recorded by prodex slim mem; output omitted" }
                        ]
                    }
                }
            },
            { "name": "session-end", "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed", "turn_complete"] }, "action": "session_end" }
        ]
    })
}
