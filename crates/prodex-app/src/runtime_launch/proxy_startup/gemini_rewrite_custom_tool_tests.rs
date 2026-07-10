use super::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
    runtime_gemini_generate_request_body, runtime_gemini_responses_value_from_generate_value,
};
use prodex_provider_core::gemini_provider_core_custom_tool_input_from_arguments;

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
}

#[test]
fn gemini_request_translation_maps_custom_apply_patch_to_function_declaration() {
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Edit a file",
        "tools": [{
            "type": "custom",
            "name": "apply_patch",
            "description": "Use the apply_patch tool to edit files.",
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": "start: begin_patch hunk+ end_patch"
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["name"],
        "apply_patch"
    );
    assert_eq!(
        value["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]["input"]["type"],
        "string"
    );
}

#[test]
fn gemini_request_translation_describes_apply_patch_add_file_grammar_for_gemini3() {
    let body = serde_json::json!({
        "model": "auto",
        "input": "Edit a file",
        "tools": [{
            "type": "custom",
            "name": "apply_patch",
            "description": "Use the apply_patch tool to edit files.",
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": "start: begin_patch hunk+ end_patch"
            }
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let declaration = &value["tools"][0]["functionDeclarations"][0];

    assert!(
        declaration["description"]
            .as_str()
            .unwrap()
            .contains("Add File")
    );
    assert!(
        declaration["parameters"]["properties"]["input"]["description"]
            .as_str()
            .unwrap()
            .contains("prefix every new file content line")
    );
}

#[test]
fn gemini_response_translation_maps_apply_patch_to_custom_tool_call() {
    let patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
    let response = serde_json::json!({
        "responseId": "resp_patch_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_patch_1",
                        "name": "apply_patch",
                        "args": {"input": patch}
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 47);

    assert_eq!(translated["output"][0]["type"], "custom_tool_call");
    assert_eq!(translated["output"][0]["name"], "apply_patch");
    assert_eq!(translated["output"][0]["call_id"], "call_patch_1");
    assert_eq!(translated["output"][0]["input"], patch);
}

#[test]
fn gemini_response_translation_normalizes_unified_diff_apply_patch() {
    let unified_diff = "\
--- a/crates/prodex-cli/src/presidio.rs
+++ b/crates/prodex-cli/src/presidio.rs
@@ -1,4 +1,4 @@
-use clap::{Args, Subcommand, ValueEnum};
+use clap::{Args, Subcommand, ValueEnum, parser::ValueSource};
 use std::path::PathBuf;";
    let response = serde_json::json!({
        "responseId": "resp_patch_2",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_patch_2",
                        "name": "apply_patch",
                        "args": {"input": unified_diff}
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 48);

    assert_eq!(translated["output"][0]["type"], "custom_tool_call");
    assert_eq!(translated["output"][0]["name"], "apply_patch");
    assert_eq!(
        translated["output"][0]["input"],
        "\
*** Begin Patch
*** Update File: crates/prodex-cli/src/presidio.rs
@@
-use clap::{Args, Subcommand, ValueEnum};
+use clap::{Args, Subcommand, ValueEnum, parser::ValueSource};
 use std::path::PathBuf;
*** End Patch"
    );
}

#[test]
fn gemini_apply_patch_input_extracts_fenced_apply_patch_block() {
    let arguments = serde_json::json!({
        "input": "Here is the patch:\n```patch\n*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch\n```\n"
    });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch"
    );
}

#[test]
fn gemini_apply_patch_input_repairs_add_file_lines_without_plus_prefix() {
    let arguments = serde_json::json!({
        "input": "*** Begin Patch\n*** Add File: gemini-patch-smoke.txt\nPRODEX_GEMINI_LIVE_OK\n*** End Patch"
    });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "*** Begin Patch\n*** Add File: gemini-patch-smoke.txt\n+PRODEX_GEMINI_LIVE_OK\n*** End Patch"
    );
}

#[test]
fn gemini_apply_patch_input_normalizes_multifile_git_diff() {
    let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,2 @@ pub fn run()
 keep
-old
+new
diff --git a/new file.txt b/new file.txt
new file mode 100644
--- /dev/null
+++ \"b/new file.txt\"
@@ -0,0 +1,2 @@
+alpha
+beta
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-gone";
    let arguments = serde_json::json!({ "input": diff });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "\
*** Begin Patch
*** Update File: src/lib.rs
@@ pub fn run()
 keep
-old
+new
*** Add File: new file.txt
+alpha
+beta
*** Delete File: gone.txt
*** End Patch"
    );
}

#[test]
fn gemini_apply_patch_input_normalizes_rename_with_changes() {
    let diff = "\
diff --git a/old/name.txt b/new/name.txt
similarity index 60%
rename from old/name.txt
rename to new/name.txt
--- a/old/name.txt
+++ b/new/name.txt
@@ -1 +1 @@
-old
+new";
    let arguments = serde_json::json!({ "input": diff });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "\
*** Begin Patch
*** Update File: old/name.txt
*** Move to: new/name.txt
@@
-old
+new
*** End Patch"
    );
}

#[test]
fn gemini_apply_patch_input_normalizes_gemini_replace_args() {
    let arguments = serde_json::json!({
        "file_path": "src/lib.rs",
        "old_string": "let value = 1;\n",
        "new_string": "let value = 2;\n"
    });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "\
*** Begin Patch
*** Update File: src/lib.rs
@@
-let value = 1;
+let value = 2;
*** End Patch"
    );
}

#[test]
fn gemini_response_translation_maps_tool_search_function_calls() {
    let response = serde_json::json!({
        "responseId": "resp_search_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": "call_search_1",
                        "name": "tool_search",
                        "args": {"query": "sqz read file", "limit": 2}
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 45);

    assert_eq!(translated["output"][0]["type"], "tool_search_call");
    assert_eq!(translated["output"][0]["execution"], "client");
    assert_eq!(translated["output"][0]["call_id"], "call_search_1");
    assert_eq!(
        translated["output"][0]["arguments"]["query"],
        "sqz read file"
    );
}

#[test]
fn gemini_response_translation_preserves_grounding_metadata_as_web_search_call() {
    let response = serde_json::json!({
        "responseId": "resp_grounded_1",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {
                "parts": [{"text": "Gemini found one source."}]
            },
            "groundingMetadata": {
                "webSearchQueries": ["prodex gemini bridge"],
                "groundingChunks": [{
                    "web": {
                        "uri": "https://example.com/prodex-gemini",
                        "title": "Prodex Gemini"
                    }
                }]
            }
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 46);

    assert_eq!(translated["output"][0]["type"], "message");
    assert_eq!(translated["output"][1]["type"], "web_search_call");
    assert_eq!(translated["output"][1]["id"], "ws_resp_grounded_1");
    assert_eq!(translated["output"][1]["status"], "completed");
    assert_eq!(
        translated["output"][1]["action"]["queries"][0],
        "prodex gemini bridge"
    );
    assert_eq!(
        translated["output"][1]["action"]["sources"][0]["url"],
        "https://example.com/prodex-gemini"
    );
}

#[test]
fn gemini_request_translation_replays_custom_tool_output_as_function_response() {
    let conversations = conversation_store();
    let patch = "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch";
    runtime_deepseek_store_conversation(
        &conversations,
        "resp_patch_1",
        vec![serde_json::json!({"role": "user", "content": "edit"})],
        vec![serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_patch_1",
                "type": "function",
                "function": {
                    "name": "apply_patch",
                    "arguments": serde_json::to_string(&serde_json::json!({"input": patch})).unwrap()
                }
            }]
        })],
    );
    let followup = serde_json::json!({
        "model": "gemini-2.5-pro",
        "previous_response_id": "resp_patch_1",
        "input": [{
            "type": "custom_tool_call_output",
            "call_id": "call_patch_1",
            "output": "Success. Updated the following files:\nA note.txt\n"
        }]
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&followup).unwrap(),
        &conversations,
        false,
        None,
        None,
    )
    .expect("request should translate");
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        value["contents"][1]["parts"][0]["functionCall"]["name"],
        "apply_patch"
    );
    assert_eq!(
        value["contents"][2]["parts"][0]["functionResponse"]["name"],
        "apply_patch"
    );
    assert_eq!(
        value["contents"][2]["parts"][0]["functionResponse"]["response"]["output"],
        "Success. Updated the following files:\nA note.txt\n"
    );
}
