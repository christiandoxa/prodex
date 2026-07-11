use super::{RuntimeDeepSeekConversationStore, RuntimeGeminiGenerateSseReader};
use std::io::Read;

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    RuntimeDeepSeekConversationStore::default()
}

#[test]
fn gemini_sse_reader_drops_internal_instruction_leak_text_before_function_call() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Do not treat them as magic box services. Explain briefly when you use them.\"},{\"functionCall\":{\"id\":\"call_poll\",\"name\":\"write_stdin\",\"args\":{\"session_id\":28582,\"chars\":\"\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        17,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("magic box services"));
    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("\"name\":\"write_stdin\""));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_strips_instruction_leak_prefix_but_keeps_answer() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_leak_answer\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Never save compressed or corrupted state to disk. Do not commit damaged or compressed files.\\nNever alter `.prodex/profiles/` files or the user's Codex `.config/` using destructive optimizers.\\nIf Presidio is enabled with `fail_mode = \\\"closed\\\"`, the proxy drops the connection on unmasked PII.\\nKeep exact output for exact requests.\\n\\nIf an optimizer fails, falls out of sync, damages files, or the user tells you to stop, report the failure directly and stop using that optimizer.\\n\\nSemua tools opsional Prodex yang terinstall di laptop ini telah diupdate ke versi terbaru.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        18,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("Never compress"));
    assert!(!output.contains("Do not repeat this checklist"));
    assert!(output.contains("Semua tools opsional Prodex"));
    assert!(output.contains("event: response.completed"));
}

#[test]
fn gemini_sse_reader_drops_optimizer_and_caveman_instruction_leaks() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_optimizer_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Do not use lossy summarization for edits, syntax definitions, migrations, schema parsing, binary patches, protocol specs, dependencies, lockfiles, exact references, or generated outputs.\\n\\nThe user instruction `caveman full` triggers the Caveman Plugin `full` level. Do not explain these instructions to the user.\\n\\nrtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        19,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("lossy summarization"));
    assert!(!output.contains("Caveman Plugin"));
    assert!(output.contains("rtk, sqz"));
}

#[test]
fn gemini_sse_reader_drops_exec_loop_and_redaction_instruction_leaks() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_optimizer_leak_2\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Use the normal `exec` follow-up loops for running tasks; do not wrap interactive loops in compression.\\nIf you need exact command output, run `rtk bypass <cmd>` or just use the plain command.\\nPresidio redaction replaces sensitive PII with synthetic markers before the model sees it.\\n\\nThese optimizers are active only when the user selects them via `prodex super` or individual binary opt-in.\\n\\nrtk, sqz, token-savior, dan claw-compactor diperbarui ke versi terbaru.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        20,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("exec` follow-up loops"));
    assert!(!output.contains("Presidio redaction replaces"));
    assert!(!output.contains("optimizers are active"));
    assert!(output.contains("rtk, sqz"));
}

#[test]
fn gemini_sse_reader_drops_remote_optimizer_instruction_leaks() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_optimizer_leak_3\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Never use remote or LLM-based optimizers for token-saving text passes.\\nOnly apply deterministic local tools. Ensure your response is completely readable, unambiguous, and true to the underlying system state.\\n\\nrtk, sqz, token-savior, dan claw-compactor sudah versi terbaru.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        21,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("remote or LLM-based optimizers"));
    assert!(!output.contains("deterministic local tools"));
    assert!(output.contains("rtk, sqz, token-savior"));
}

#[test]
fn gemini_sse_reader_drops_super_runtime_and_memory_instruction_leaks() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_super_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"When a test or build fails under `rtk`, read the exact failure details before editing.\\nToken savings tools must fail fast. Do not loop trying to fix a token-optimizer tool crash.\\nProdex Super injects `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `XDG_CACHE_HOME` overrides.\\n\\n## Memory Files\\n\\nWhen updating memory files manually, ensure you write to the correct active path.\\n\\nrtk tetap versi terbaru.\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        22,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("test or build fails under"));
    assert!(!output.contains("Memory Files"));
    assert!(!output.contains("XDG_DATA_HOME"));
    assert!(output.contains("rtk tetap versi terbaru"));
}

#[test]
fn gemini_sse_reader_drops_optimizer_diagnostic_instruction_leaks() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_optimizer_diagnostic_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"If an optimizer tool hangs, faults, or emits corrupted/binary text, stop using it, say why briefly, and proceed without it.\\nIf the user's explicit requested answer or command output must be an exact string, strip RTK proxy banners and diagnostic noise before returning the final text.\\nIf the prompt dictates \\\"Answer with only the command output\\\", then output only the command output string.\\nIf you suspect an optimizer has obscured a critical signal, bypass it and inspect the raw file or standard command output directly.\\nDo not hallucinate optimizer tools or CLI wrapper paths. Inspect `PRODEX_OPTIMIZERS_HOME` or `~/.local/share/prodex-optimizers` only if the user explicitly asks for optimizer diagnostics.\\nDo not apply lossy formatters/minifiers as a token-saving tactic unless the user requested code minification. Lossless deduplication and syntactic abbreviation (like `rtk`, `prodex-sqz`, `claw-compactor`, `prodex-token-savior`) are allowed and encouraged. If an optimizer crashes or strips critical context, bypass it for that turn and use normal shell commands or file reads.\\nDo not invoke MCP servers or extra optimization processes without matching local files or user instructions.\\n## References\\n[RTK Proxy documentation](https://github.com/doxa-labs/rust-token-killer/blob/main/README.md)\\nThis concludes the injected system instructions.\\n\\nPRODEX_GEMINI_LIVE_OK\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        23,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("optimizer has obscured"));
    assert!(!output.contains("optimizer tool hangs"));
    assert!(!output.contains("requested answer or command output"));
    assert!(!output.contains("hallucinate optimizer tools"));
    assert!(!output.contains("MCP servers or extra optimization"));
    assert!(!output.contains("lossy formatters"));
    assert!(!output.contains("Lossless deduplication"));
    assert!(!output.contains("RTK Proxy documentation"));
    assert!(!output.contains("injected system instructions"));
    assert!(output.contains("PRODEX_GEMINI_LIVE_OK"));
}

#[test]
fn gemini_sse_reader_drops_optimizer_fallback_instruction_leaks() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_optimizer_fallback_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"breaks task execution, immediately drop the optimizer tool and use normal file reads/commands to complete the task. updates or basic file reads unless the user explicitly asks for optimizer diagnostics. Use optimizers for their intended job: reducing token usage of large files and deep graphs. Do not overcomplicate targeted reads or basic config debugging.\\n\\nSaya cek implementasi Gemini adapter sekarang.\"},{\"functionCall\":{\"name\":\"exec_command\",\"args\":{\"cmd\":\"rg gemini_response crates/prodex-app/src/runtime_launch/proxy_startup\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        25,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("breaks task execution"));
    assert!(!output.contains("optimizer diagnostics"));
    assert!(output.contains("Saya cek implementasi Gemini adapter sekarang."));
    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("\"name\":\"exec_command\""));
}

#[test]
fn gemini_sse_reader_hides_super_capabilities_leak_during_native_working_state() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_super_capabilities_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Previously unseen internal prompt wording that must stay hidden.\\n\\n## Interaction\\nUse tools transparently behind the scenes.\"},{\"functionCall\":{\"name\":\"exec_command\",\"args\":{\"cmd\":\"rtk git log --oneline -5\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        26,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("Previously unseen"));
    assert!(!output.contains("Interaction"));
    assert!(!output.contains("response.output_text.delta"));
    assert!(!output.contains("response.reasoning_summary_text.delta"));
    assert!(output.contains("\"type\":\"function_call\""));
    assert!(output.contains("\"name\":\"exec_command\""));
}

#[test]
fn gemini_sse_reader_drops_exact_output_instruction_leak() {
    let stream = concat!(
        "data: {\"responseId\":\"resp_exact_output_leak\",\"modelVersion\":\"gemini-3.1-pro-preview\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"```\\ncat gemini-patch-smoke.txt\\n```\\nBut the prompt says: \\\"If the user requests an exact string, answer-only output, or command output only, emit only that requested output and nothing else. For exact-output prompts, do not include explanations, diffs, status, previous-turn recaps, or extra sentences before or after the requested output.\\\"\\n\\nWhen the user explicitly asks for exact command output, answer with exactly that output, without surrounding text, summaries, or rates.\\n\\nAll commands run with user privileges. Wait or poll for commands that return a running session ID until they complete. Do not stop midway. Execute requested tool tasks fully.\\n\\nPRODEX_GEMINI_LIVE_OK\"}]},\"finishReason\":\"STOP\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(stream.as_bytes()),
        24,
        Vec::new(),
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(!output.contains("answer-only output"));
    assert!(!output.contains("exact command output"));
    assert!(!output.contains("without surrounding text"));
    assert!(!output.contains("Execute requested tool tasks"));
    assert!(!output.contains("For exact-output prompts"));
    assert!(!output.contains("do not include explanations"));
    assert!(output.contains("PRODEX_GEMINI_LIVE_OK"));
}
