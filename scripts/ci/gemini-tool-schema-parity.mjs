#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");
const geminiBridgeDir = path.join(repoRoot, "crates/prodex-provider-core/src/gemini_bridge");
const geminiTranslatorDir = path.join(
  repoRoot,
  "crates/prodex-provider-core/src/translators/gemini",
);
const proxyStartupDir = path.join(
  repoRoot,
  "crates/prodex-app/src/runtime_launch/proxy_startup",
);

const PARITY = Object.freeze([
  {
    tool: "read_file",
    description: [
      "targeted, surgical ranges",
      "start_line and end_line",
    ],
    parameters: {
      start_line: "1-based first line",
      end_line: "1-based last line",
    },
  },
  {
    tool: "read_many_files",
    description: [
      "Read multiple files",
      "honor git, Gemini, custom ignore rules",
      "default heavy-directory excludes",
    ],
  },
  {
    tool: "grep",
    description: [
      "ripgrep-style behavior",
      "Use this before broad reads",
    ],
    parameters: {
      pattern: "Literal or regex pattern",
    },
  },
  {
    tool: "exec_command",
    description: [
      "Run a shell command",
      "non-interactive commands",
      "continue background sessions",
    ],
  },
  {
    tool: "write_file",
    description: [
      "Write complete file content",
      "Do not use placeholders",
      "preserve unrelated user changes",
    ],
  },
  {
    tool: "apply_patch",
    description: [
      "Apply targeted file edits",
      "Keep edits narrow",
      "instead of describing changes",
    ],
  },
]);

const REQUEST_CONTRACTS = Object.freeze([
  "systemInstruction",
  "contents",
  "tools",
  "toolConfig",
  "generationConfig",
  "functionDeclarations",
  "functionCall",
  "functionResponse",
  "inlineData",
  "fileData",
]);

const RESPONSE_CONTRACTS = Object.freeze([
  "usageMetadata",
  "cachedContentTokenCount",
  "thoughtsTokenCount",
  "toolUsePromptTokenCount",
  "finishReason",
  "promptFeedback",
  "citationMetadata",
  "safetyRatings",
]);

const SSE_CONTRACTS = Object.freeze([
  "response.created",
  "response.output_item.added",
  "response.output_text.delta",
  "response.function_call_arguments.delta",
  "response.completed",
  "response.failed",
  "provider_stream_error",
]);

const LIVE_CONTRACTS = Object.freeze([
  "session.update",
  "input_audio_buffer.append",
  "response.cancel",
  "conversation.item.create",
  "setup",
  "setupComplete",
  "realtime_input",
  "tool_response",
  "serverContent",
  "toolCall",
  "functionCalls",
  "interrupted",
  "turnComplete",
  "response.cancelled",
  "response.done",
]);

const COMPACT_CONTRACTS = Object.freeze([
  "GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS",
  "prodex_gemini_compaction",
  "gemini_provider_core_semantic_compact_request_body",
  "runtime_gemini_semantic_compact_response_parts",
  "runtime_gemini_local_compact_response_parts",
]);

const EXACT_OUTPUT_CONTRACTS = Object.freeze([
  "runtime_gemini_forced_command_output",
  "gemini_provider_core_requests_command_output_only",
  "gemini_provider_core_command_output_from_tool_message",
  "gemini_provider_core_collect_payload_text",
  "skip(user_index + 1)",
]);

function parseArgs(argv) {
  const args = { json: false };
  for (const value of argv.slice(2)) {
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/gemini-tool-schema-parity.mjs [--json]",
      "",
      "Validates Gemini request, response, SSE, compact, exact-output, tool-schema, and Live compatibility snippets.",
      "--json prints the generated parity manifest used by the validator.",
    ].join("\n") + "\n",
  );
}

function validateSnippetSet(source, label, snippets) {
  return snippets
    .filter((snippet) => !source.includes(snippet))
    .map((snippet) => `${label}: missing schema snippet ${JSON.stringify(snippet)}`);
}

function validate(requestSource, responseSource, sseSource, liveSource, compactSource) {
  const failures = [];
  for (const item of PARITY) {
    for (const snippet of item.description) {
      if (!requestSource.includes(snippet)) {
        failures.push(`${item.tool}: missing description snippet ${JSON.stringify(snippet)}`);
      }
    }
    for (const [parameter, snippet] of Object.entries(item.parameters ?? {})) {
      if (!requestSource.includes(`"${parameter}"`) || !requestSource.includes(snippet)) {
        failures.push(
          `${item.tool}.${parameter}: missing parameter description snippet ${JSON.stringify(snippet)}`,
        );
      }
    }
  }
  failures.push(...validateSnippetSet(requestSource, "request", REQUEST_CONTRACTS));
  failures.push(...validateSnippetSet(responseSource, "response", RESPONSE_CONTRACTS));
  failures.push(...validateSnippetSet(sseSource, "sse", SSE_CONTRACTS));
  failures.push(...validateSnippetSet(liveSource, "live", LIVE_CONTRACTS));
  failures.push(...validateSnippetSet(compactSource, "compact", COMPACT_CONTRACTS));
  failures.push(...validateSnippetSet(sseSource, "exact-output", EXACT_OUTPUT_CONTRACTS));
  return failures;
}

function readRustFiles(dir, predicate = () => true) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "tests") {
          return [];
        }
        return readRustFiles(entryPath, predicate);
      }
      if (
        !entry.isFile() ||
        !entry.name.endsWith(".rs") ||
        entry.name === "tests.rs" ||
        entry.name.endsWith("_tests.rs") ||
        !predicate(entryPath)
      ) {
        return [];
      }
      return [fs.readFileSync(entryPath, "utf8")];
    })
    .join("\n");
}

function readRustModule(file, seen = new Set()) {
  const resolved = path.resolve(file);
  const basename = path.basename(resolved);
  if (
    seen.has(resolved) ||
    basename === "tests.rs" ||
    basename.endsWith("_tests.rs") ||
    resolved.split(path.sep).includes("tests")
  ) {
    return "";
  }
  seen.add(resolved);
  const source = fs.readFileSync(resolved, "utf8");
  const sources = [source];
  const modulePattern = /(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;/g;
  for (const match of source.matchAll(modulePattern)) {
    const [, explicitPath, moduleName] = match;
    const dir = path.dirname(resolved);
    const stem = path.basename(resolved, path.extname(resolved));
    const candidates = explicitPath
      ? [path.resolve(dir, explicitPath)]
      : [
          path.join(dir, `${moduleName}.rs`),
          path.join(dir, moduleName, "mod.rs"),
          path.join(dir, stem, `${moduleName}.rs`),
          path.join(dir, stem, moduleName, "mod.rs"),
        ];
    const child = candidates.find((candidate) => fs.existsSync(candidate));
    if (child) {
      sources.push(readRustModule(child, seen));
    }
  }
  return sources.join("\n");
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  if (args.json) {
    process.stdout.write(
      JSON.stringify(
        {
          version: 3,
          parity: PARITY,
          requestContracts: REQUEST_CONTRACTS,
          responseContracts: RESPONSE_CONTRACTS,
          sseContracts: SSE_CONTRACTS,
          liveContracts: LIVE_CONTRACTS,
          compactContracts: COMPACT_CONTRACTS,
          exactOutputContracts: EXACT_OUTPUT_CONTRACTS,
        },
        null,
        2,
      ) + "\n",
    );
    return;
  }
  const requestSource = [
    readRustFiles(geminiBridgeDir),
    readRustFiles(geminiTranslatorDir),
    readRustModule(path.join(proxyStartupDir, "gemini_request.rs")),
  ].join("\n");
  const responseSource = [
    readRustFiles(geminiBridgeDir),
    readRustFiles(geminiTranslatorDir),
    readRustModule(path.join(proxyStartupDir, "gemini_sse.rs")),
  ].join("\n");
  const sseSource = [
    readRustFiles(geminiBridgeDir),
    readRustModule(path.join(proxyStartupDir, "gemini_sse.rs")),
    fs.readFileSync(path.join(proxyStartupDir, "provider_sse_events.rs"), "utf8"),
  ].join("\n");
  const liveSource = [
    readRustModule(path.join(geminiBridgeDir, "live.rs")),
    readRustModule(path.join(proxyStartupDir, "local_rewrite_gemini_live.rs")),
  ].join("\n");
  const compactSource = [
    readRustModule(path.join(geminiBridgeDir, "compact.rs")),
    readRustModule(path.join(proxyStartupDir, "local_rewrite_gemini_compact.rs")),
  ].join("\n");
  const failures = validate(requestSource, responseSource, sseSource, liveSource, compactSource);
  if (failures.length > 0) {
    for (const failure of failures) {
      process.stderr.write(`gemini schema parity: ${failure}\n`);
    }
    process.exitCode = 1;
    return;
  }
  process.stdout.write(
    `gemini schema parity: ${PARITY.length} tool contracts, ${REQUEST_CONTRACTS.length} request snippets, ${RESPONSE_CONTRACTS.length} response snippets, ${SSE_CONTRACTS.length} SSE snippets, ${LIVE_CONTRACTS.length} Live snippets, ${COMPACT_CONTRACTS.length} compact snippets, ${EXACT_OUTPUT_CONTRACTS.length} exact-output snippets validated\n`,
  );
}

main();
