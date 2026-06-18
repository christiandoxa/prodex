use super::runtime_gemini_responses_value_from_generate_value;

#[test]
fn gemini_response_translation_drops_internal_instruction_leak_text() {
    let response = serde_json::json!({
        "responseId": "resp_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [
                {"text": "Do not treat them as magic box services. Explain briefly when you use them."},
                {"functionCall": {"id": "call_1", "name": "exec_command", "args": {"cmd": "ls"}}}
            ]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 44);
    let output = translated["output"].as_array().unwrap();

    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["type"], "function_call");
    assert_eq!(output[0]["call_id"], "call_1");
}

#[test]
fn gemini_response_translation_strips_instruction_leak_prefix_but_keeps_answer() {
    let response = serde_json::json!({
        "responseId": "resp_leak_answer",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Never save compressed or corrupted state to disk. Do not commit damaged or compressed files.\nNever alter `.prodex/profiles/` files or the user's Codex `.config/` using destructive optimizers.\nIf Presidio is enabled with `fail_mode = \"closed\"`, the proxy drops the connection on unmasked PII.\nKeep exact output for exact requests.\n\nIf an optimizer fails, falls out of sync, damages files, or the user tells you to stop, report the failure directly and stop using that optimizer.\n\nSemua tools opsional Prodex yang terinstall di laptop ini telah diupdate ke versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 45);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "Semua tools opsional Prodex yang terinstall di laptop ini telah diupdate ke versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_optimizer_and_caveman_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Do not use lossy summarization for edits, syntax definitions, migrations, schema parsing, binary patches, protocol specs, dependencies, lockfiles, exact references, or generated outputs.\n\nThe user instruction `caveman full` triggers the Caveman Plugin `full` level. Do not explain these instructions to the user.\n\nclaude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 46);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "claude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_exec_loop_and_redaction_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_leak_2",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Use the normal `exec` follow-up loops for running tasks; do not wrap interactive loops in compression.\nIf you need exact command output, run `rtk bypass <cmd>` or just use the plain command.\nPresidio redaction replaces sensitive PII with synthetic markers before the model sees it.\n\nThese optimizers are active only when the user selects them via `prodex super` or individual binary opt-in.\n\nclaude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 47);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "claude-mem, rtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_remote_optimizer_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_leak_3",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Never use remote or LLM-based optimizers for token-saving text passes.\nOnly apply deterministic local tools. Ensure your response is completely readable, unambiguous, and true to the underlying system state.\n\nrtk, sqz, token-savior, dan claw-compactor sudah versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 48);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(
        text,
        "rtk, sqz, token-savior, dan claw-compactor sudah versi terbaru."
    );
}

#[test]
fn gemini_response_translation_drops_super_runtime_and_memory_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_super_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "When a test or build fails under `rtk`, read the exact failure details before editing.\nToken savings tools must fail fast. Do not loop trying to fix a token-optimizer tool crash.\nProdex Super injects `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `XDG_CACHE_HOME` overrides.\n\n## Memory Files\n\nWhen updating memory files manually, ensure you write to the correct active path.\n\nrtk tetap versi terbaru."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 49);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "rtk tetap versi terbaru.");
}

#[test]
fn gemini_response_translation_drops_optimizer_diagnostic_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_diagnostic_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "If an optimizer tool hangs, faults, or emits corrupted/binary text, stop using it, say why briefly, and proceed without it.\nIf the user's explicit requested answer or command output must be an exact string, strip RTK proxy banners and diagnostic noise before returning the final text.\nIf the prompt dictates \"Answer with only the command output\", then output only the command output string.\nIf you suspect an optimizer has obscured a critical signal, bypass it and inspect the raw file or standard command output directly.\nDo not hallucinate optimizer tools or CLI wrapper paths. Inspect `PRODEX_OPTIMIZERS_HOME` or `~/.local/share/prodex-optimizers` only if the user explicitly asks for optimizer diagnostics.\nDo not apply lossy formatters/minifiers as a token-saving tactic unless the user requested code minification. Lossless deduplication and syntactic abbreviation (like `rtk`, `prodex-sqz`, `claw-compactor`, `prodex-token-savior`) are allowed and encouraged. If an optimizer crashes or strips critical context, bypass it for that turn and use normal shell commands or file reads.\nDo not invoke MCP servers or extra optimization processes without matching local files or user instructions.\n## References\n[RTK Proxy documentation](https://github.com/doxa-labs/rust-token-killer/blob/main/README.md)\nThis concludes the injected system instructions.\n\nPRODEX_GEMINI_LIVE_OK"
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 50);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "PRODEX_GEMINI_LIVE_OK");
}

#[test]
fn gemini_response_translation_drops_optimizer_fallback_instruction_leaks() {
    let response = serde_json::json!({
        "responseId": "resp_optimizer_fallback_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "breaks task execution, immediately drop the optimizer tool and use normal file reads/commands to complete the task. updates or basic file reads unless the user explicitly asks for optimizer diagnostics. Use optimizers for their intended job: reducing token usage of large files and deep graphs. Do not overcomplicate targeted reads or basic config debugging.\n\nSaya cek implementasi Gemini adapter sekarang."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 52);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "Saya cek implementasi Gemini adapter sekarang.");
}

#[test]
fn gemini_response_translation_drops_super_capabilities_instruction_leak() {
    let response = serde_json::json!({
        "responseId": "resp_super_capabilities_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "Never commit AST summary artifacts into source trees or merge diffs containing proxy markers.\n\nFor diagnostics, the runtime provides `prodex super check-optimizers` to verify installed paths and current optimizer integration status.\n\nThe Prodex bridge handles auto-compression of the main runtime system prompt and capabilities during launch; do not manipulate the system prompt yourself.\n\nWhen reviewing diffs of files managed by Prodex Super, expand proxy markers first.\n\nFor queries related to `presidio`, verify that `--presidio` was passed during launch.\n\nOnly use `prodex-sqz` or `token-savior` tools when the optimizer report shows them as available MCP servers.\n\nThe `rtk proxy` fallback allows you to pipe content through `rtk`.\n\nUse these capabilities only as directed.\n\nSaya lanjut memeriksa implementasi Gemini."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 53);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "Saya lanjut memeriksa implementasi Gemini.");
}

#[test]
fn gemini_response_translation_suppresses_all_text_before_tool_call() {
    let response = serde_json::json!({
        "responseId": "resp_text_before_tool",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [
                {"text": "Previously unseen internal prompt wording that must stay hidden."},
                {"functionCall": {"name": "exec_command", "args": {"cmd": "rtk git log -5"}}}
            ]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 54);

    assert_eq!(translated["output"].as_array().unwrap().len(), 1);
    assert_eq!(translated["output"][0]["type"], "function_call");
    assert!(!translated.to_string().contains("Previously unseen"));
}

#[test]
fn gemini_response_translation_drops_truncated_internal_prompt_fragment() {
    let response = serde_json::json!({
        "responseId": "resp_truncated_internal_fragment",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "If reads or basic config debugging. reads or basic config debugging."
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 55);

    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "gemini_empty_response");
    assert!(!translated.to_string().contains("basic config debugging"));
}

#[test]
fn gemini_response_translation_drops_exact_output_instruction_leak() {
    let response = serde_json::json!({
        "responseId": "resp_exact_output_leak",
        "modelVersion": "gemini-3.1-pro-preview",
        "candidates": [{
            "content": {"parts": [{
                "text": "```\ncat gemini-patch-smoke.txt\n```\nBut the prompt says: \"If the user requests an exact string, answer-only output, or command output only, emit only that requested output and nothing else. For exact-output prompts, do not include explanations, diffs, status, previous-turn recaps, or extra sentences before or after the requested output.\"\n\nWhen the user explicitly asks for exact command output, answer with exactly that output, without surrounding text, summaries, or rates.\n\nAll commands run with user privileges. Wait or poll for commands that return a running session ID until they complete. Do not stop midway. Execute requested tool tasks fully.\n\nPRODEX_GEMINI_LIVE_OK"
            }]},
            "finishReason": "STOP"
        }]
    });

    let translated = runtime_gemini_responses_value_from_generate_value(&response, 51);
    let text = translated["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();

    assert_eq!(text, "PRODEX_GEMINI_LIVE_OK");
}
