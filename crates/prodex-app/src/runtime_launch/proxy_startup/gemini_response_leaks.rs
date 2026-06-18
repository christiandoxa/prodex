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
    "Never commit AST summary artifacts",
    "Never rehydrate cached optimizer state",
    "Do not substitute synthetic `MEMORY.md` summaries",
    "If the user changes their instructions",
    "If the `sqz-mcp`",
    "Do not download `.whl` packages",
    "When the `prodex` runtime proxy is diagnosing itself",
    "For diagnostics, the runtime provides `prodex super check-optimizers`",
    "The Prodex bridge handles auto-compression",
    "When reviewing diffs of files managed by Prodex Super",
    "Prodex manages its own documentation",
    "For queries related to `presidio`",
    "Only use `prodex-sqz` or `token-savior` tools",
    "The `rtk proxy` fallback allows you",
    "Use these capabilities only as directed",
    "If reads or basic config debugging",
    "If `SUPER_OPTIMIZERS_WANTS_CAVEMAN`",
    "If `SUPER_OPTIMIZERS_WANTS_NO_RTK_AUTO`",
    "Use this knowledge silently",
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
    "## Interaction",
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
    "This file is generated by `prodex super`",
    "Previously unseen internal prompt wording",
    "SUPER_OPTIMIZERS_WANTS_CAVEMAN",
    "SUPER_OPTIMIZERS_WANTS_NO_RTK_AUTO",
    "Prodex Super token optimizers initialized",
    "ExactUse web search",
    "RTK auto-wrapping is suppressed",
    "does not change Codex or Claude Code native telemetry",
    "irreversible reductions",
];

pub(in super::super::super) fn runtime_gemini_internal_instruction_leak_text(text: &str) -> bool {
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
        || lower.contains("if a compression tool")
        || lower.contains("if a token")
        || lower.contains("token-saving")
        || lower.contains("lossless deduplication")
        || lower.contains("syntactic abbreviation")
        || lower.contains("bypass it for that turn")
        || lower.contains("strip rtk proxy banners")
        || lower.contains("prodex manages optimizer scopes")
        || lower.contains("normal shell commands or file reads")
        || lower.contains("breaks task execution")
        || lower.contains("immediately drop the optimizer")
        || lower.contains("drop the optimizer tool")
        || lower.contains("fall back to plain shell")
        || lower.contains("fallback to plain shell")
        || lower.contains("fall back to normal file")
        || lower.contains("use normal file reads")
        || lower.contains("use normal file reads/commands")
        || lower.contains("use normal file reads/commands to complete")
        || lower.contains("unless the user explicitly asks for optimizer diagnostics")
        || lower.contains("use optimizers for their intended job")
        || lower.contains("do not overcomplicate targeted reads")
        || lower.contains("do not overcomplicate basic config debugging");
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

pub(in super::super::super) fn runtime_gemini_sanitize_internal_instruction_leak_text(
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

pub(in super::super::super) fn runtime_gemini_visible_text_from_part(
    part: &serde_json::Value,
) -> Option<String> {
    if part
        .get("thought")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    part.get("text")
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .and_then(runtime_gemini_sanitize_internal_instruction_leak_text)
}

pub(in super::super::super) fn runtime_gemini_internal_instruction_corpus(
    messages: &[serde_json::Value],
) -> String {
    let mut text = String::new();
    for message in messages {
        if message.get("role").and_then(serde_json::Value::as_str) != Some("system") {
            continue;
        }
        runtime_gemini_collect_text_for_echo_detection(message.get("content"), &mut text);
        text.push('\n');
    }
    runtime_gemini_normalized_words(&text).join(" ")
}

pub(in super::super::super) fn runtime_gemini_text_echoes_internal_instruction(
    text: &str,
    internal_instruction_corpus: &str,
) -> bool {
    if internal_instruction_corpus.is_empty() {
        return false;
    }
    let words = runtime_gemini_normalized_words(text);
    const ECHO_WORDS: usize = 8;
    if words.len() < ECHO_WORDS {
        return false;
    }
    words.windows(ECHO_WORDS).take(128).any(|window| {
        let needle = window.join(" ");
        internal_instruction_corpus.contains(&needle)
    })
}

fn runtime_gemini_collect_text_for_echo_detection(
    value: Option<&serde_json::Value>,
    output: &mut String,
) {
    match value {
        Some(serde_json::Value::String(text)) => {
            output.push_str(text);
            output.push('\n');
        }
        Some(serde_json::Value::Array(values)) => {
            for value in values {
                runtime_gemini_collect_text_for_echo_detection(Some(value), output);
            }
        }
        Some(serde_json::Value::Object(object)) => {
            for key in ["text", "content", "input", "output"] {
                runtime_gemini_collect_text_for_echo_detection(object.get(key), output);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_normalized_words(text: &str) -> Vec<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .map(str::trim)
        .filter(|word| word.len() >= 2)
        .map(str::to_ascii_lowercase)
        .collect()
}
