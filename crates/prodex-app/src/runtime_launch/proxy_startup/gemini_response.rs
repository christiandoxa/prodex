use super::super::deepseek_rewrite::{
    runtime_deepseek_created_at, runtime_deepseek_rtk_wrapped_tool_arguments,
};
use super::super::gemini_thought_signatures::runtime_gemini_thought_signature;
use super::super::provider_tools::runtime_provider_split_flat_namespace_tool_name;
use super::gemini_apply_patch::runtime_gemini_custom_apply_patch_input;
use super::gemini_request::{
    runtime_gemini_blocked_tool_call_message, runtime_gemini_data_url_parts,
    runtime_gemini_mime_type_for_uri,
};
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use std::borrow::Cow;

const GEMINI_CUSTOM_APPLY_PATCH_TOOL: &str = "apply_patch";
const GEMINI_INTERNAL_INSTRUCTION_LEAK_PREFIXES: &[&str] = &[
    "When editing code, always emit exact valid language syntax",
    "Do not use lossy summarization",
    "The user instruction `caveman",
    "Do not run tests or builds unless",
    "If a command produces no output",
    "If a tool command succeeds",
    "When tests pass",
    "When tests fail",
    "When the user writes in Indonesian",
    "No \"Saya sudah menyelesaikan",
    "If a compression tool or optimizer wrapper crashes",
    "If `rtk`, `sqz-mcp`, `token-savior`, or `claw-compactor` fails",
    "If `rtk`, `sqz`, `token-savior`, `claw-compactor`, or `presidio` fails",
    "If `rtk`, `sqz`, or `token-savior` is completely unusable",
    "If `rtk`, `sqz-mcp`, or `claw-compactor` fails",
    "If a token optimization tool fails",
    "If a token optimizer fails",
    "If an optimizer MCP server crashes",
    "If the user disables Caveman mode",
    "If an optimizer command fails",
    "If an optimizer tool hangs",
    "If an optimizer MCP tool or wrapper hangs",
    "If an optimizer blocks progress",
    "If an optimizer breaks or stalls",
    "If an optimizer breaks output layout",
    "If a `rtk` command drops",
    "If a `token-savior` edit corrupts",
    "For critical signals",
    "Do not apply these tools to configuration",
    "If you are unsure if compression removes relevant detail",
    "If the user asks you to stop using optimizers",
    "If the model loses context",
    "If `prodex-token-savior` or `prodex-sqz` commands are not found",
    "If `claw-compactor` fails",
    "If a token-optimizer proxy breaks",
    "Missing MCP servers, disabled tools",
    "Do not install optimizers during a run unless",
    "Use `rtk --no-proxy <cmd>` only as an escape hatch",
    "When a test or build fails under `rtk`",
    "Token savings tools must fail fast",
    "Fall back to exact commands or text",
    "Do not skip or shorten test failures",
    "Do not skip imports or type signatures",
    "Never skip or shorten test failures",
    "Do not pipe edits",
    "Never use lossy tools on exact command output",
    "Never use lossy tools to summarize user-facing documentation",
    "Keep code examples exactly as written",
    "Keep task-related code exact",
    "Keep imports and type signatures exact",
    "Keep failing test cases exact",
    "When modifying code based on a token-optimized representation",
    "Prodex Super disables verbose upstream OpenAI library",
    "Prodex Super automatically uses `codex --mem`",
    "Prodex Super strips `cat`, `grep`, and `ls` aliases",
    "Prodex Super injects `XDG_DATA_HOME`",
    "Prodex super-isolated shells strip `cat`, `grep`, and `ls` aliases",
    "Prodex Super passes the exact CLI arguments",
    "When diagnosing issues in `prodex-app`",
    "If `prodex super` seems slow to start",
    "If memory usage surges",
    "Use standard diagnostic helpers such as `prodex doctor --runtime`",
    "When updating memory files manually",
    "Always use caveman style",
    "For exact-output prompts",
    "To read memory:",
    "To read the inbox:",
    "To force a memory consolidation:",
    "If the environment variables are not set",
    "When the user specifies `--project-memory`",
    "Use the normal `exec` follow-up loops",
    "If you need exact command output",
    "If you need a tool result byte-for-byte exact",
    "Presidio redaction replaces sensitive PII",
    "Redacted text must be manually re-assembled",
    "Do not use `token-savior` `apply_patch`",
    "If you find `PRODEX_GEMINI_LIVE_EXTENDED`",
    "These optimizers are active only when",
    "Caveman communication style stays active",
    "Always check that the user explicitly asked for a tool that is an optimizer",
    "Always use raw text for security checks",
    "Do not apply text-based optimizers",
    "Do not compress the active Prodex codebase",
    "Never use remote or LLM-based optimizers",
    "Only apply deterministic local tools",
    "Never use AST compression",
    "Never use lossy or AST-mutating optimizers",
    "Never rewrite Prodex proxy source files",
    "Exact source must remain intact",
    "Exact source must remain intact for code generation",
    "If Presidio redaction is active",
    "If the user asks for exact text or code",
    "If the user asks for exact command output",
    "When returning control",
    "Use `rtk gain`",
    "Do not narrate token-saving steps",
    "Savior diagnostic command",
    "Let the user know the optimization tools saved tokens",
    "If the user's explicit requested answer or command output",
    "If the prompt dictates \"Answer with only the command output\"",
    "If the user reports missing IPs",
    "If you suspect an optimizer has obscured a critical signal",
    "Do not hallucinate optimizer tools or CLI wrapper paths",
    "Do not emit custom marker prefixes",
    "Do not use `rtk` on shell commands if you need exactly matched raw byte output",
    "Do not invoke MCP servers or extra optimization processes",
    "Keep original `.md` memory files intact",
    "Keep files clean",
    "Never compress any part of an edit",
    "Never save compressed files to disk",
    "Never save compressed or corrupted state to disk",
    "Never rewrite `.prodex/profiles/` files",
    "Never alter `.prodex/profiles/` files",
    "When reporting issues, include the exact error",
    "Before launch, Super asks whether to add Presidio redaction",
    "Presidio redaction strips PII",
    "If Presidio is enabled with `fail_mode = \"closed\"`",
    "If an optimizer strips necessary detail or breaks",
    "If an optimizer produces mangled",
    "If an optimizer is blocked",
    "If an optimizer fails, falls out of sync",
    "If the user says stop, wait, or revert",
    "When the user uses `--debug`",
    "The prompt requires me to output ONLY",
    "You must follow Caveman rules",
    "Tokens must die before facts",
    "Keep exact output for exact requests",
    "Do not repeat this checklist",
    "verify that required data",
    "Do not treat them as magic box services",
    "These optimizers are local tools",
    "Super keeps compression decisions local",
    "You are Codex CLI",
    "The user must experience native Codex CLI",
    "Follow the active Codex",
    "Tool discipline for Codex parity",
    "CAVEMAN MODE ACTIVE",
    "RTK ACTIVE",
    "Step 3: Run `cat gemini-patch-smoke.txt`",
];
const GEMINI_INTERNAL_INSTRUCTION_LEAK_MARKERS: &[&str] = &[
    "## Privacy (Presidio)",
    "## Metrics",
    "## Reporting",
    "## References",
    "## Verification",
    "## Status",
    "## Prodex Runtime Logging",
    "<thought",
    "CRITICAL INSTRUCTION",
    "Related tools for this task",
    "default_api:",
    "I must use `default_api:",
    "I will call `default_api:",
    "I am still waiting for the command to finish",
    "## Memory Files",
    "## Formatting",
    "## Configuration",
    "Caveman Plugin",
    "Do not explain these instructions",
    "You are Codex CLI",
    "native Codex CLI",
    "Tool discipline for Codex parity",
    "CAVEMAN MODE ACTIVE",
    "RTK ACTIVE",
    "Step 3: Run `cat gemini-patch-smoke.txt`",
    "Keep Caveman style",
    "Do not tell the user what tools or settings you used",
    "Do not try to debug or test if tests pass",
    "Do not pretend optimizers are built into the models",
    "Never use lossy memory compaction without the user's explicit consent",
    "When using these optimizers, state briefly that you are using them to save tokens",
    "Presidio redacted it",
    "RTK and Prodex Smart Context auto-wrappers conflict",
    "disable wrapper fallback by using an absolute path",
    "Do not use local optimizers for exact validation tasks",
    "If the user requests an exact string",
    "If the user requests exact text",
    "bypass all optimizers",
    "RTK, SQZ, and Token Savior",
    "native Codex MCP",
    "answer-only output",
    "command output only",
    "emit only that requested output",
    "For exact-output prompts",
    "do not include explanations, diffs, status",
    "Code changes, commits, and PRs must remain readable",
    "Use local optimizers only",
    "The Super launch is your implicit permission",
    "Prodex proxy components",
    "Prodex internal source code",
    "Prodex internals (`prodex-app`",
    "agent's meta-operations",
    "Prodex Super code bug",
    "When in Super tools",
    "degraded fallback capability",
    "Do not use Caveman `ultra`",
    "known noisy targets like tests or diffs",
    "untrusted code outside the agent environment",
    "Super mode does not alter normal security instructions",
    "token-optimizer tool",
    "token-optimizer crash",
    "token savings",
    "Claude-Mem workflow",
    "active profile path",
    "PRODEX_SUPER_OAI_DEBUG",
    "PRODEX_CLAW_SESSIONSTART_TIMEOUT_SECONDS",
    "XDG_CACHE_HOME/prodex-sqz",
    "XDG_CACHE_HOME/prodex-token-savior",
    "optimizer <name> unavailable",
    "optimizer <name> failed",
    "prodex super --metrics",
    "token_budget",
    "SUPER_OPTIMIZERS.md",
    "RTK.md",
    "optimizer overrides",
    "host environment to be healthy",
    "This concludes the injected system instructions",
    ".prodex/super.toml",
    "PRODEX_SUPER_AUTO_COMPRESS",
    "irreversible reductions",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum RuntimeGeminiResponseStatus {
    Failed { code: String, message: String },
    Incomplete { reason: String, message: String },
}

pub(in super::super) fn runtime_gemini_normalized_response_value(
    value: &serde_json::Value,
) -> Cow<'_, serde_json::Value> {
    let Some(response) = value.get("response") else {
        return Cow::Borrowed(value);
    };
    let Some(trace_id) = value.get("traceId").and_then(serde_json::Value::as_str) else {
        return Cow::Borrowed(response);
    };
    let mut response = response.clone();
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "responseId".to_string(),
            serde_json::Value::String(trace_id.to_string()),
        );
    }
    Cow::Owned(response)
}

pub(in super::super) fn runtime_gemini_responses_usage(
    usage: &serde_json::Value,
) -> Option<serde_json::Value> {
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    let cached_tokens = usage
        .get("cachedContentTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let reasoning_tokens = usage
        .get("thoughtsTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let tool_tokens = usage
        .get("toolUsePromptTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    Some(serde_json::json!({
        "input_tokens": input_tokens,
        "input_tokens_details": {
            "cached_tokens": cached_tokens,
            "tool_tokens": tool_tokens,
        },
        "output_tokens": output_tokens,
        "output_tokens_details": {
            "reasoning_tokens": reasoning_tokens,
        },
        "total_tokens": total_tokens,
    }))
}

pub(in super::super) fn runtime_gemini_chat_assistant_messages_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> Vec<serde_json::Value> {
    let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut text = String::new();
    let mut reasoning_content = String::new();
    let mut gemini_content = Vec::new();
    let mut native_parts = Vec::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str) {
            if part
                .get("thought")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                reasoning_content.push_str(part_text);
            } else {
                let Some(part_text) =
                    runtime_gemini_sanitize_internal_instruction_leak_text(part_text)
                else {
                    continue;
                };
                text.push_str(&part_text);
            }
        }
        if let Some(part_text) = runtime_gemini_text_from_special_part(part) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&part_text);
        }
        if let Some(content_item) = runtime_gemini_media_content_item_from_part(part) {
            gemini_content.push(content_item);
            native_parts.push(part.clone());
        }
        if part.get("videoMetadata").is_some() && !native_parts.contains(part) {
            native_parts.push(part.clone());
        }
        if let Some(function_call) = part.get("functionCall") {
            let name = function_call
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool_call");
            let call_id = runtime_gemini_function_call_id(function_call, request_id, index);
            let args = function_call
                .get("args")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            if let Some(blocked) = runtime_gemini_blocked_tool_call_message(name, &args) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&blocked);
                continue;
            }
            let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
            let mut tool_call = serde_json::json!({
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": runtime_deepseek_rtk_wrapped_tool_arguments(name, &args),
                },
            });
            if let Some(signature) = runtime_gemini_thought_signature(part)
                .or_else(|| runtime_gemini_thought_signature(function_call))
            {
                tool_call["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            tool_calls.push(tool_call);
        }
    }
    if text.is_empty()
        && reasoning_content.is_empty()
        && gemini_content.is_empty()
        && tool_calls.is_empty()
    {
        return Vec::new();
    }
    let mut assistant = serde_json::json!({
        "role": "assistant",
        "content": if text.is_empty() {
            if tool_calls.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(String::new())
            }
        } else {
            serde_json::Value::String(text)
        },
    });
    if !reasoning_content.is_empty() {
        assistant["reasoning_content"] = serde_json::Value::String(reasoning_content);
    }
    if !gemini_content.is_empty() {
        assistant["gemini_media_content"] = serde_json::Value::Array(gemini_content);
    }
    if !native_parts.is_empty() {
        assistant["gemini_native_parts"] = serde_json::Value::Array(native_parts);
    }
    if !tool_calls.is_empty() {
        assistant["tool_calls"] = serde_json::Value::Array(tool_calls);
    }
    if let Some(metadata) = runtime_gemini_response_metadata(value) {
        assistant["gemini_metadata"] = metadata;
    }
    vec![assistant]
}

pub(in super::super) fn runtime_gemini_responses_value_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    let response_id = runtime_gemini_response_id(value, request_id);
    let model = runtime_gemini_model(value);
    let mut output = Vec::new();
    if let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    {
        let mut text = String::new();
        let mut content_items = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            if let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str)
                && !part
                    .get("thought")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
                && let Some(part_text) =
                    runtime_gemini_sanitize_internal_instruction_leak_text(part_text)
            {
                text.push_str(&part_text);
            }
            if let Some(part_text) = runtime_gemini_text_from_special_part(part) {
                content_items.push(serde_json::json!({
                    "type": "output_text",
                    "text": part_text,
                }));
            }
            if let Some(content_item) = runtime_gemini_media_content_item_from_part(part) {
                content_items.push(content_item);
            }
            if let Some(image_generation) =
                runtime_gemini_image_generation_call_item_from_part(&response_id, index, part)
            {
                output.push(image_generation);
            }
            if let Some(function_call) = part.get("functionCall") {
                output.push(runtime_gemini_responses_tool_call_item(
                    part,
                    function_call,
                    request_id,
                    index,
                ));
            }
        }
        if !text.is_empty() {
            let mut content = vec![serde_json::json!({
                "type": "output_text",
                "text": text,
            })];
            content.extend(content_items);
            output.insert(
                0,
                serde_json::json!({
                    "type": "message",
                    "role": "assistant",
                    "content": content,
                }),
            );
        } else if !content_items.is_empty() {
            output.insert(
                0,
                serde_json::json!({
                    "type": "message",
                    "role": "assistant",
                    "content": content_items,
                }),
            );
        }
    }
    if let Some(grounding_call) = runtime_gemini_web_search_call_from_grounding(value, &response_id)
    {
        output.push(grounding_call);
    }
    if let Some(citations) = runtime_gemini_citation_text(value) {
        output.push(serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": citations,
            }],
        }));
    }
    let status = runtime_gemini_response_status(value, !output.is_empty());
    let mut response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": runtime_deepseek_created_at(),
        "model": model,
        "output": output,
    });
    if let Some(status) = status {
        match status {
            RuntimeGeminiResponseStatus::Failed { code, message } => {
                response["status"] = serde_json::Value::String("failed".to_string());
                response["error"] = serde_json::json!({
                    "code": code,
                    "message": message,
                });
            }
            RuntimeGeminiResponseStatus::Incomplete { reason, message } => {
                response["status"] = serde_json::Value::String("incomplete".to_string());
                response["incomplete_details"] = serde_json::json!({
                    "reason": reason,
                    "message": message,
                });
            }
        }
    }
    if let Some(usage) = value
        .get("usageMetadata")
        .and_then(runtime_gemini_responses_usage)
    {
        response["usage"] = usage;
    }
    if let Some(metadata) = runtime_gemini_response_metadata(value) {
        response["metadata"] = metadata;
    }
    response
}

pub(in super::super) fn runtime_gemini_internal_instruction_leak_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    GEMINI_INTERNAL_INSTRUCTION_LEAK_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || GEMINI_INTERNAL_INSTRUCTION_LEAK_MARKERS
            .iter()
            .any(|marker| trimmed.contains(marker))
        || runtime_gemini_optimizer_instruction_leak_text(&lower)
}

fn runtime_gemini_optimizer_instruction_leak_text(lower: &str) -> bool {
    let mentions_optimizer_surface = lower.contains("optimizer")
        || lower.contains("rtk")
        || lower.contains("prodex-sqz")
        || lower.contains("token-savior")
        || lower.contains("claw-compactor")
        || lower.contains("mcp server");
    let imperative_or_internal = lower.contains("do not ")
        || lower.contains("never ")
        || lower.contains("if an optimizer")
        || lower.contains("if a token")
        || lower.contains("token-saving")
        || lower.contains("lossless deduplication")
        || lower.contains("syntactic abbreviation")
        || lower.contains("bypass it for that turn")
        || lower.contains("strip rtk proxy banners")
        || lower.contains("prodex manages optimizer scopes")
        || lower.contains("normal shell commands or file reads");
    mentions_optimizer_surface && imperative_or_internal
        || runtime_gemini_exact_output_instruction_leak_text(lower)
}

fn runtime_gemini_exact_output_instruction_leak_text(lower: &str) -> bool {
    let mentions_exact_output = lower.contains("exact command output")
        || lower.contains("answer with exactly that output")
        || lower.contains("without surrounding text")
        || lower.contains("answer-only output")
        || lower.contains("command output only")
        || lower.contains("emit only that requested output")
        || lower.contains("for exact-output prompts")
        || lower.contains("answer with only the command output");
    let mentions_internal_action = lower.contains("when the user")
        || lower.contains("if the user")
        || lower.contains("do not ")
        || lower.contains("wait or poll")
        || lower.contains("all commands run with user privileges")
        || lower.contains("running session id")
        || lower.contains("do not stop midway")
        || lower.contains("execute requested tool tasks");
    mentions_exact_output && mentions_internal_action
        || (lower.contains("all commands run with user privileges")
            && lower.contains("execute requested tool tasks"))
}

pub(in super::super) fn runtime_gemini_sanitize_internal_instruction_leak_text(
    text: &str,
) -> Option<String> {
    if !runtime_gemini_internal_instruction_leak_text(text) {
        return Some(text.to_string());
    }
    let retained = text
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .filter(|paragraph| !runtime_gemini_internal_instruction_leak_text(paragraph))
        .collect::<Vec<_>>();
    if retained.is_empty() {
        None
    } else {
        Some(retained.join("\n\n"))
    }
}

pub(in super::super) fn runtime_gemini_response_status(
    value: &serde_json::Value,
    has_visible_output: bool,
) -> Option<RuntimeGeminiResponseStatus> {
    if let Some(failure) = runtime_gemini_prompt_feedback_failure(value) {
        let (code, message) = failure;
        return Some(RuntimeGeminiResponseStatus::Failed { code, message });
    }
    if let Some(reason) = runtime_gemini_finish_reason(value) {
        if let Some((reason, message)) = runtime_gemini_finish_reason_incomplete(&reason) {
            return Some(RuntimeGeminiResponseStatus::Incomplete { reason, message });
        }
        if let Some(failure) = runtime_gemini_finish_reason_failure(&reason) {
            let (code, message) = failure;
            return Some(RuntimeGeminiResponseStatus::Failed { code, message });
        }
    }
    if !has_visible_output {
        let suffix = runtime_gemini_finish_reason(value)
            .map(|reason| format!(" finishReason={reason}"))
            .unwrap_or_default();
        return Some(RuntimeGeminiResponseStatus::Failed {
            code: "gemini_empty_response".to_string(),
            message: format!("Gemini returned no visible response content.{suffix}"),
        });
    }
    None
}

pub(in super::super) fn runtime_gemini_prompt_feedback_failure(
    value: &serde_json::Value,
) -> Option<(String, String)> {
    let feedback = value.get("promptFeedback")?;
    let reason = feedback
        .get("blockReason")
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())?;
    Some((
        "gemini_prompt_blocked".to_string(),
        format!("Gemini blocked the prompt: {reason}"),
    ))
}

pub(in super::super) fn runtime_gemini_finish_reason(value: &serde_json::Value) -> Option<String> {
    value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("finishReason"))
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .map(str::to_string)
}

pub(in super::super) fn runtime_gemini_finish_reason_failure(
    reason: &str,
) -> Option<(String, String)> {
    let code = match reason {
        "MALFORMED_FUNCTION_CALL" => "gemini_malformed_function_call",
        "UNEXPECTED_TOOL_CALL" => "gemini_unexpected_tool_call",
        "OTHER" => "gemini_finish_other",
        "NO_IMAGE" => "gemini_no_image",
        "SAFETY"
        | "RECITATION"
        | "LANGUAGE"
        | "BLOCKLIST"
        | "PROHIBITED_CONTENT"
        | "SPII"
        | "IMAGE_SAFETY"
        | "IMAGE_PROHIBITED_CONTENT" => "invalid_prompt",
        _ => return None,
    };
    Some((
        code.to_string(),
        format!("Gemini ended the stream with finishReason={reason}"),
    ))
}

pub(in super::super) fn runtime_gemini_finish_reason_incomplete(
    reason: &str,
) -> Option<(String, String)> {
    match reason {
        "MAX_TOKENS" => Some((
            "max_output_tokens".to_string(),
            "Gemini stopped because it reached the maximum output token limit.".to_string(),
        )),
        _ => None,
    }
}

pub(in super::super) fn runtime_gemini_finish_reason_retryable_invalid(reason: &str) -> bool {
    matches!(
        reason,
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "OTHER"
    )
}

pub(in super::super) fn runtime_gemini_media_content_item_from_part(
    part: &serde_json::Value,
) -> Option<serde_json::Value> {
    if let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("application/octet-stream");
        let data = inline_data
            .get("data")
            .and_then(serde_json::Value::as_str)?;
        if mime_type.starts_with("image/") {
            return Some(serde_json::json!({
                "type": "input_image",
                "image_url": format!("data:{mime_type};base64,{data}"),
            }));
        }
        return Some(serde_json::json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }));
    }
    if let Some(file_data) = part.get("fileData").or_else(|| part.get("file_data")) {
        let file_uri = file_data
            .get("fileUri")
            .or_else(|| file_data.get("file_uri"))
            .and_then(serde_json::Value::as_str)?;
        let mime_type = file_data
            .get("mimeType")
            .or_else(|| file_data.get("mime_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| runtime_gemini_mime_type_for_uri(file_uri));
        if mime_type.starts_with("image/") {
            return Some(serde_json::json!({
                "type": "input_image",
                "image_url": file_uri,
            }));
        }
        return Some(serde_json::json!({
            "type": "output_text",
            "text": format!("Gemini returned {mime_type} media: {file_uri}"),
        }));
    }
    let text = part.get("text").and_then(serde_json::Value::as_str)?;
    let (mime_type, data) = runtime_gemini_data_url_parts(text)?;
    if mime_type.starts_with("image/") {
        Some(serde_json::json!({
            "type": "input_image",
            "image_url": format!("data:{mime_type};base64,{data}"),
        }))
    } else {
        Some(serde_json::json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }))
    }
}

pub(in super::super) fn runtime_gemini_text_from_special_part(
    part: &serde_json::Value,
) -> Option<String> {
    if let Some(executable_code) = part.get("executableCode") {
        let language = executable_code
            .get("language")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("text");
        let code = executable_code
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if code.trim().is_empty() {
            return None;
        }
        return Some(format!(
            "Gemini executable code ({language}):\n```{language}\n{code}\n```"
        ));
    }
    if let Some(result) = part.get("codeExecutionResult") {
        let outcome = result
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("OUTCOME_UNSPECIFIED");
        let output = result
            .get("output")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        return Some(format!(
            "Gemini code execution result ({outcome}):\n```text\n{output}\n```"
        ));
    }
    if let Some(video_metadata) = part.get("videoMetadata") {
        let metadata = serde_json::to_string(video_metadata).unwrap_or_else(|_| "{}".to_string());
        return Some(format!("Gemini video metadata: {metadata}"));
    }
    None
}

pub(in super::super) fn runtime_gemini_image_generation_call_item_from_part(
    response_id: &str,
    index: usize,
    part: &serde_json::Value,
) -> Option<serde_json::Value> {
    let inline_data = part.get("inlineData").or_else(|| part.get("inline_data"))?;
    let mime_type = inline_data
        .get("mimeType")
        .or_else(|| inline_data.get("mime_type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("application/octet-stream");
    if !mime_type.starts_with("image/") {
        return None;
    }
    let data = inline_data
        .get("data")
        .and_then(serde_json::Value::as_str)?;
    Some(serde_json::json!({
        "type": "image_generation_call",
        "id": format!("ig_{response_id}_{index}"),
        "status": "completed",
        "result": data,
    }))
}

pub(in super::super) fn runtime_gemini_response_metadata(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let mut gemini = serde_json::Map::new();
    for key in ["promptFeedback", "usageMetadata"] {
        if let Some(field) = value.get(key).filter(|field| !field.is_null()) {
            gemini.insert(key.to_string(), field.clone());
        }
    }
    if let Some(candidate) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
    {
        for key in [
            "finishReason",
            "finishMessage",
            "safetyRatings",
            "citationMetadata",
            "groundingMetadata",
            "urlContextMetadata",
            "avgLogprobs",
            "logprobsResult",
        ] {
            if let Some(field) = candidate.get(key).filter(|field| !field.is_null()) {
                gemini.insert(key.to_string(), field.clone());
            }
        }
    }
    if gemini.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "gemini": serde_json::Value::Object(gemini),
    }))
}

pub(in super::super) fn runtime_gemini_citation_text(value: &serde_json::Value) -> Option<String> {
    runtime_gemini_finish_reason(value)?;
    let citations = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("citationMetadata"))
        .and_then(|metadata| metadata.get("citations"))
        .and_then(serde_json::Value::as_array)?;
    let mut lines = citations
        .iter()
        .filter_map(|citation| {
            let uri = citation
                .get("uri")
                .and_then(serde_json::Value::as_str)
                .filter(|uri| !uri.trim().is_empty())?;
            let title = citation
                .get("title")
                .and_then(serde_json::Value::as_str)
                .filter(|title| !title.trim().is_empty());
            Some(match title {
                Some(title) => format!("({title}) {uri}"),
                None => uri.to_string(),
            })
        })
        .collect::<Vec<_>>();
    lines.sort();
    lines.dedup();
    (!lines.is_empty()).then(|| format!("Citations:\n{}", lines.join("\n")))
}

pub(in super::super) fn runtime_gemini_web_search_call_from_grounding(
    value: &serde_json::Value,
    response_id: &str,
) -> Option<serde_json::Value> {
    let candidate = value.get("candidates")?.as_array()?.first()?;
    let mut sources = Vec::new();
    let grounding_metadata = candidate.get("groundingMetadata");
    if let Some(chunks) = grounding_metadata
        .and_then(|metadata| metadata.get("groundingChunks"))
        .and_then(serde_json::Value::as_array)
    {
        for chunk in chunks {
            for source_kind in ["web", "retrievedContext"] {
                if let Some(source) = chunk
                    .get(source_kind)
                    .and_then(runtime_gemini_url_source_from_metadata)
                {
                    runtime_gemini_push_unique_url_source(&mut sources, source);
                }
            }
        }
    }
    if let Some(citations) = candidate
        .get("citationMetadata")
        .and_then(|metadata| {
            metadata
                .get("citations")
                .or_else(|| metadata.get("citationSources"))
        })
        .and_then(serde_json::Value::as_array)
    {
        for citation in citations {
            if let Some(source) = runtime_gemini_url_source_from_metadata(citation) {
                runtime_gemini_push_unique_url_source(&mut sources, source);
            }
        }
    }
    if let Some(url_metadata) = candidate
        .get("urlContextMetadata")
        .and_then(|metadata| {
            metadata
                .get("urlMetadata")
                .or_else(|| metadata.get("url_metadata"))
        })
        .and_then(serde_json::Value::as_array)
    {
        for entry in url_metadata {
            if let Some(source) = runtime_gemini_url_source_from_metadata(entry) {
                runtime_gemini_push_unique_url_source(&mut sources, source);
            }
        }
    }

    let queries = grounding_metadata
        .and_then(|metadata| metadata.get("webSearchQueries"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    if sources.is_empty() && queries.is_empty() {
        return None;
    }

    let action = if queries.is_empty() {
        let first_url = sources
            .first()
            .and_then(|source| source.get("url"))
            .and_then(serde_json::Value::as_str)?;
        serde_json::json!({
            "type": "open_page",
            "url": first_url,
            "sources": sources,
        })
    } else {
        serde_json::json!({
            "type": "search",
            "queries": queries,
            "sources": sources,
        })
    };
    Some(serde_json::json!({
        "type": "web_search_call",
        "id": format!("ws_{response_id}"),
        "status": "completed",
        "action": action,
    }))
}

fn runtime_gemini_url_source_from_metadata(value: &serde_json::Value) -> Option<serde_json::Value> {
    let uri = ["uri", "url", "retrievedUrl", "retrieved_url"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(serde_json::Value::as_str))
        .filter(|uri| !uri.trim().is_empty())?;
    let mut source = serde_json::json!({
        "type": "url",
        "url": uri,
    });
    if let Some(title) = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .filter(|title| !title.trim().is_empty())
    {
        source["title"] = serde_json::Value::String(title.to_string());
    }
    if let Some(status) = value
        .get("urlRetrievalStatus")
        .or_else(|| value.get("url_retrieval_status"))
        .or_else(|| value.get("status"))
        .filter(|status| !status.is_null())
    {
        source["status"] = status.clone();
    }
    Some(source)
}

fn runtime_gemini_push_unique_url_source(
    sources: &mut Vec<serde_json::Value>,
    source: serde_json::Value,
) {
    let source_url = source.get("url").and_then(serde_json::Value::as_str);
    if sources
        .iter()
        .any(|existing| existing.get("url").and_then(serde_json::Value::as_str) == source_url)
    {
        return;
    }
    sources.push(source);
}

fn runtime_gemini_response_id(value: &serde_json::Value, request_id: u64) -> String {
    value
        .get("responseId")
        .or_else(|| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("resp_gemini_{request_id}"))
}

fn runtime_gemini_model(value: &serde_json::Value) -> String {
    value
        .get("modelVersion")
        .or_else(|| value.get("model"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(GEMINI_DEFAULT_MODEL)
        .to_string()
}

fn runtime_gemini_responses_tool_call_item(
    part: &serde_json::Value,
    function_call: &serde_json::Value,
    request_id: u64,
    index: usize,
) -> serde_json::Value {
    let call_id = runtime_gemini_function_call_id(function_call, request_id, index);
    let flat_name = function_call
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    let args_value = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(blocked) = runtime_gemini_blocked_tool_call_message(flat_name, &args_value) {
        return runtime_gemini_blocked_tool_call_item(&blocked);
    }
    if flat_name == "tool_search" {
        return serde_json::json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": args_value,
        });
    }
    if let Some(item) = runtime_gemini_custom_tool_call_item(&call_id, flat_name, &args_value) {
        return item;
    }
    let args = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
    let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(flat_name);
    let mut item = serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": runtime_deepseek_rtk_wrapped_tool_arguments(flat_name, &args),
    });
    if let Some(namespace) = namespace {
        item["namespace"] = serde_json::Value::String(namespace);
    }
    if let Some(signature) = runtime_gemini_thought_signature(part)
        .or_else(|| runtime_gemini_thought_signature(function_call))
    {
        item["gemini_thought_signature"] = serde_json::Value::String(signature);
    }
    item
}

fn runtime_gemini_blocked_tool_call_item(message: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": message,
        }],
    })
}

pub(in super::super) fn runtime_gemini_custom_tool_call_item(
    call_id: &str,
    flat_name: &str,
    args_value: &serde_json::Value,
) -> Option<serde_json::Value> {
    if flat_name != GEMINI_CUSTOM_APPLY_PATCH_TOOL {
        return None;
    }
    Some(serde_json::json!({
        "type": "custom_tool_call",
        "call_id": call_id,
        "name": flat_name,
        "input": runtime_gemini_custom_tool_input_from_args_value(args_value),
    }))
}

pub(in super::super) fn runtime_gemini_custom_tool_input_from_arguments(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .map(|value| runtime_gemini_custom_tool_input_from_args_value(&value))
        .unwrap_or_else(|_| arguments.to_string())
}

fn runtime_gemini_custom_tool_input_from_args_value(args_value: &serde_json::Value) -> String {
    runtime_gemini_custom_apply_patch_input(args_value)
}

fn runtime_gemini_function_call_id(
    function_call: &serde_json::Value,
    request_id: u64,
    index: usize,
) -> String {
    function_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("call_gemini_{request_id}_{index}"))
}
