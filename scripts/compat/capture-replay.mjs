#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_OUT_DIR = path.join(repoRoot, "tests/fixtures/compat_replay");
const DEFAULT_FORMAT_VERSION = 1;

const SENSITIVE_KEY_PATTERNS = [
  /authorization/i,
  /api[_-]?key/i,
  /cookie/i,
  /credential/i,
  /password/i,
  /private[_-]?key/i,
  /refresh[_-]?token/i,
  /access[_-]?token/i,
  /secret/i,
  /token/i,
];

const VOLATILE_KEY_PLACEHOLDERS = new Map([
  ["id", "<id>"],
  ["request_id", "<request_id>"],
  ["x-request-id", "<request_id>"],
  ["x-client-request-id", "<request_id>"],
  ["session_id", "<session_id>"],
  ["x-session-id", "<session_id>"],
  ["x-claude-code-session-id", "<session_id>"],
  ["response_id", "<response_id>"],
  ["previous_response_id", "<previous_response_id>"],
  ["response_ids", "<response_id>"],
  ["trace_id", "<trace_id>"],
  ["thread_id", "<thread_id>"],
  ["conversation_id", "<conversation_id>"],
  ["account_id", "<account_id>"],
  ["chatgpt-account-id", "<account_id>"],
  ["organization_id", "<organization_id>"],
  ["user_id", "<user_id>"],
  ["call_id", "<call_id>"],
  ["approval_request_id", "<approval_request_id>"],
  ["created", "<created>"],
  ["created_at", "<created_at>"],
  ["updated_at", "<updated_at>"],
  ["started_at", "<started_at>"],
  ["completed_at", "<completed_at>"],
  ["timestamp", "<timestamp>"],
  ["ts", "<timestamp>"],
  ["pid", "<pid>"],
]);

function usage() {
  return `Usage:
  node scripts/compat/capture-replay.mjs --input capture.jsonl [--name codex_live]

Options:
  --input, -i <path>       JSON, JSONL, or text capture input
  --out-dir <path>         Output directory (default: tests/fixtures/compat_replay)
  --output <path>          Exact output file path
  --name <slug>            Fixture basename (default: input filename)
  --stdout                 Print scrubbed fixture instead of writing
  --check                  Compare rendered fixture with existing output
  --self-test              Run built-in scrub regression check
  --help, -h               Show this help
`;
}

function parseArgs(argv) {
  const args = {
    input: null,
    outDir: DEFAULT_OUT_DIR,
    output: null,
    name: null,
    stdout: false,
    check: false,
    selfTest: false,
    help: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--input" || value === "-i") {
      index += 1;
      args.input = argv[index];
      if (!args.input) {
        throw new Error(`${value} requires a value`);
      }
      continue;
    }
    if (value === "--out-dir") {
      index += 1;
      args.outDir = argv[index];
      if (!args.outDir) {
        throw new Error("--out-dir requires a value");
      }
      continue;
    }
    if (value === "--output") {
      index += 1;
      args.output = argv[index];
      if (!args.output) {
        throw new Error("--output requires a value");
      }
      continue;
    }
    if (value === "--name") {
      index += 1;
      args.name = argv[index];
      if (!args.name) {
        throw new Error("--name requires a value");
      }
      continue;
    }
    if (value === "--stdout") {
      args.stdout = true;
      continue;
    }
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--self-test") {
      args.selfTest = true;
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

function lowerKey(value) {
  return String(value ?? "").toLowerCase();
}

function isSensitiveKey(key) {
  const normalized = lowerKey(key);
  return SENSITIVE_KEY_PATTERNS.some((pattern) => pattern.test(normalized));
}

function placeholderForVolatileKey(key) {
  const normalized = lowerKey(key);
  if (VOLATILE_KEY_PLACEHOLDERS.has(normalized)) {
    return VOLATILE_KEY_PLACEHOLDERS.get(normalized);
  }
  if (normalized.endsWith("_at") || normalized.endsWith("-at")) {
    return `<${normalized.replace(/-/g, "_")}>`;
  }
  return null;
}

function slugify(value) {
  const slug = String(value ?? "")
    .trim()
    .replace(/\.[^.]+$/, "")
    .replace(/[^A-Za-z0-9._-]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return slug || "capture";
}

function parseJsonMaybe(value) {
  if (typeof value !== "string") {
    return { parsed: false, value };
  }
  const trimmed = value.trim();
  if (!trimmed || !/^[{["0-9tfn-]/.test(trimmed)) {
    return { parsed: false, value };
  }
  try {
    return { parsed: true, value: JSON.parse(trimmed) };
  } catch {
    return { parsed: false, value };
  }
}

function scrubText(value) {
  return String(value)
    .replace(/\bBearer\s+[A-Za-z0-9._~+/-]+=*/gi, "Bearer <redacted>")
    .replace(/\bsk-proj-[A-Za-z0-9_-]{8,}\b/g, "sk-proj-<redacted>")
    .replace(/\bsk-ant-[A-Za-z0-9_-]{8,}\b/g, "sk-ant-<redacted>")
    .replace(/\bsk-[A-Za-z0-9_-]{8,}\b/g, "sk-<redacted>")
    .replace(
      /\b(api[_-]?key|access[_-]?token|refresh[_-]?token|auth(?:orization)?(?:[_-]?token)?|client[_-]?secret|password|cookie|secret)(\s*[:=]\s*)(["']?)[^"',\s}]+/gi,
      "$1$2$3<redacted>",
    )
    .replace(
      /\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/gi,
      "<uuid>",
    )
    .replace(/\bresp_[A-Za-z0-9_-]+\b/g, "<response_id>")
    .replace(/\bsess(?:ion)?_[A-Za-z0-9_-]+\b/g, "<session_id>")
    .replace(/\bturn_[A-Za-z0-9_-]+\b/g, "<turn_state>")
    .replace(/\breq_[A-Za-z0-9_-]+\b/g, "<request_id>")
    .replace(/\bcall_[A-Za-z0-9_-]+\b/g, "<call_id>")
    .replace(/\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\b/g, "<timestamp>");
}

function scrubValue(value, key = "") {
  if (value === null || value === undefined) {
    return value ?? null;
  }

  if (isSensitiveKey(key)) {
    return "<redacted>";
  }

  const placeholder = placeholderForVolatileKey(key);
  if (placeholder) {
    if (Array.isArray(value)) {
      return value.map((item) => (item === null || item === undefined ? null : placeholder));
    }
    return value === null ? null : placeholder;
  }

  if (Array.isArray(value)) {
    return value.map((item) => scrubValue(item, key));
  }

  if (typeof value === "object") {
    const scrubbed = Object.fromEntries(
      Object.entries(value).map(([entryKey, entryValue]) => [
        entryKey,
        scrubValue(entryValue, entryKey),
      ]),
    );
    if (typeof value.name === "string" && Object.hasOwn(value, "value")) {
      scrubbed.value = normalizeHeaderValue(value.name, value.value);
    }
    return scrubbed;
  }

  if (typeof value === "string") {
    const parsed = parseJsonMaybe(value);
    if (parsed.parsed) {
      return scrubValue(parsed.value, key);
    }
    return scrubText(value);
  }

  return value;
}

function normalizeHeaderValue(name, value) {
  const normalized = lowerKey(name);
  if (isSensitiveKey(normalized)) {
    if (/^authorization$/i.test(normalized) && /^Bearer\s+/i.test(String(value))) {
      return "Bearer <redacted>";
    }
    return "<redacted>";
  }
  const placeholder = placeholderForVolatileKey(normalized);
  if (placeholder) {
    return placeholder;
  }
  if (normalized === "x-codex-turn-state") {
    return "<turn_state>";
  }
  if (normalized === "x-codex-turn-metadata") {
    return "<turn_metadata>";
  }
  return scrubText(value);
}

function normalizeHeaderEntry(name, value) {
  return {
    name: String(name),
    value: normalizeHeaderValue(name, value),
  };
}

function normalizeHeaders(headers) {
  if (!headers) {
    return [];
  }

  if (Array.isArray(headers)) {
    return headers.flatMap((entry) => {
      if (Array.isArray(entry) && entry.length >= 2) {
        return [normalizeHeaderEntry(entry[0], entry[1])];
      }
      if (typeof entry === "string") {
        const separator = entry.indexOf(":");
        if (separator <= 0) {
          return [];
        }
        return [normalizeHeaderEntry(entry.slice(0, separator).trim(), entry.slice(separator + 1).trim())];
      }
      if (entry && typeof entry === "object") {
        const name = entry.name ?? entry.key ?? entry.header;
        const value = entry.value ?? entry.values ?? "";
        if (!name) {
          return [];
        }
        if (Array.isArray(value)) {
          return value.map((item) => normalizeHeaderEntry(name, item));
        }
        return [normalizeHeaderEntry(name, value)];
      }
      return [];
    });
  }

  if (typeof headers === "object") {
    return Object.entries(headers).flatMap(([name, value]) => {
      if (Array.isArray(value)) {
        return value.map((item) => normalizeHeaderEntry(name, item));
      }
      return [normalizeHeaderEntry(name, value)];
    });
  }

  return [];
}

function normalizeBody(value) {
  if (value === undefined) {
    return null;
  }
  const parsed = parseJsonMaybe(value);
  return scrubValue(parsed.value);
}

function pathAndQueryFromRecord(record) {
  const raw = record.path_and_query ?? record.path ?? record.url ?? "/";
  const value = String(raw);
  try {
    const parsed = new URL(value);
    return `${parsed.pathname}${parsed.search}`;
  } catch {
    return scrubText(value);
  }
}

function normalizeRequest(record) {
  return {
    type: "request",
    method: String(record.method ?? "GET").toUpperCase(),
    path_and_query: pathAndQueryFromRecord(record),
    headers: normalizeHeaders(record.headers),
    body: normalizeBody(record.body ?? record.payload ?? record.data),
  };
}

function normalizeResponseOrEvent(record, type) {
  const body = record.body ?? record.payload ?? record.data ?? record.event ?? null;
  const normalized = {
    type,
    headers: normalizeHeaders(record.headers),
    body: normalizeBody(body),
  };
  if (record.status !== undefined) {
    normalized.status = record.status;
  }
  if (typeof record.event_name === "string" || typeof record.event === "string") {
    normalized.event_name = scrubText(record.event_name ?? record.event);
  }
  return normalized;
}

function normalizeWebsocketMessage(record) {
  const message = record.message ?? record.payload ?? record.data ?? "";
  return {
    type: "websocket_message",
    direction: scrubText(record.direction ?? "unknown"),
    message: normalizeBody(message),
  };
}

function normalizeSseStreamText(text) {
  const output = [];
  for (const rawLine of String(text).split(/\r?\n/)) {
    const line = rawLine.trimEnd();
    if (line.startsWith("data:")) {
      const prefix = line.startsWith("data: ") ? "data: " : "data:";
      const payload = line.slice(prefix.length);
      if (payload.trim() === "[DONE]") {
        output.push(`${prefix}[DONE]`);
        continue;
      }
      const parsed = parseJsonMaybe(payload);
      if (parsed.parsed) {
        output.push(`${prefix}${JSON.stringify(scrubValue(parsed.value))}`);
      } else {
        output.push(`${prefix}${scrubText(payload)}`);
      }
      continue;
    }
    if (line.startsWith("id:")) {
      output.push("id: <id>");
      continue;
    }
    output.push(scrubText(line));
  }
  return output.join("\n");
}

function normalizeSseStream(record) {
  const stream = record.stream ?? record.text ?? record.body ?? record.data ?? "";
  if (Array.isArray(stream)) {
    return {
      type: "sse_stream",
      events: scrubValue(stream),
    };
  }
  return {
    type: "sse_stream",
    stream: normalizeSseStreamText(stream),
  };
}

function normalizeRecordType(record) {
  return lowerKey(record.type ?? record.kind ?? record.record_type ?? "request").replace(/-/g, "_");
}

function normalizeRecord(record, index) {
  const type = normalizeRecordType(record);
  let normalized;
  if (type === "request") {
    normalized = normalizeRequest(record);
  } else if (type === "response") {
    normalized = normalizeResponseOrEvent(record, "response");
  } else if (type === "event" || type === "response_event") {
    normalized = normalizeResponseOrEvent(record, "event");
  } else if (type === "websocket_message" || type === "websocket") {
    normalized = normalizeWebsocketMessage(record);
  } else if (type === "sse_stream" || type === "sse") {
    normalized = normalizeSseStream(record);
  } else {
    throw new Error(`record ${index + 1} has unsupported type: ${type}`);
  }

  if (record.name || record.label) {
    normalized.name = slugify(record.name ?? record.label);
  }
  return normalized;
}

function buildFixture(records, sourceName) {
  return {
    format_version: DEFAULT_FORMAT_VERSION,
    kind: "prodex_compat_replay_capture",
    source: path.basename(sourceName),
    records: records.map((record, index) => normalizeRecord(record, index)),
  };
}

async function loadRecords(inputPath) {
  const text = await fs.readFile(inputPath, "utf8");
  const extension = path.extname(inputPath).toLowerCase();

  if (extension === ".jsonl" || extension === ".ndjson") {
    return text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith("#"))
      .map((line, index) => {
        try {
          return JSON.parse(line);
        } catch (error) {
          throw new Error(`invalid JSONL at line ${index + 1}: ${error.message}`);
        }
      });
  }

  if (extension === ".json") {
    const value = JSON.parse(text);
    if (Array.isArray(value)) {
      return value;
    }
    if (Array.isArray(value.records)) {
      return value.records;
    }
    return [value];
  }

  return [
    {
      type: "sse_stream",
      name: slugify(path.basename(inputPath)),
      stream: text,
    },
  ];
}

function outputPathForArgs(args) {
  if (args.output) {
    return path.resolve(args.output);
  }
  const inputBase = args.input ? path.basename(args.input) : "capture";
  const name = slugify(args.name ?? inputBase);
  return path.resolve(args.outDir, `${name}_capture_replay.json`);
}

function sampleRecords() {
  return [
    {
      type: "request",
      name: "codex_live_request",
      method: "POST",
      url: "https://chatgpt.com/backend-api/codex/responses",
      headers: {
        Authorization: "Bearer live_authorization_secret_12345",
        "ChatGPT-Account-Id": "acct_live_12345",
        ["x-" + "api-key"]: "sk-live-" + "secret-123456789",
        "x-codex-turn-state": "turn_live_20260428",
        session_id: "sess_live_20260428",
        "User-Agent": "codex-cli/fixture",
      },
      body: {
        model: "gpt-5.3-codex",
        stream: true,
        previous_response_id: "resp_parent_live_20260428",
        client_metadata: {
          session_id: "sess_body_live_20260428",
          ["api_" + "key"]: "body_api_" + "key_secret_12345",
        },
      },
    },
    {
      type: "sse_stream",
      name: "codex_live_sse",
      stream:
        'event: response.created\n' +
        'data: {"created_at":"2026-04-28T01:02:03Z","response_id":"resp_child_live_20260428","response":{"id":"resp_child_live_20260428"},"authorization_token":"sse_secret_token_12345"}\n\n',
    },
    {
      type: "websocket_message",
      name: "codex_live_ws",
      direction: "client_to_upstream",
      message: {
        previous_response_id: "resp_ws_live_20260428",
        session_id: "sess_ws_live_20260428",
        input: [{ type: "function_call", call_id: "call_ws_live_20260428" }],
      },
    },
  ];
}

function assertNoSampleSecrets(rendered) {
  for (const needle of [
    "live_authorization_secret",
    "sk-live-secret",
    "body_api_key_secret",
    "sse_secret_token",
    "resp_parent_live",
    "sess_body_live",
    "turn_live_20260428",
    "call_ws_live",
    "2026-04-28T01:02:03Z",
  ]) {
    if (rendered.includes(needle)) {
      throw new Error(`scrubbed replay still contains secret or volatile value: ${needle}`);
    }
  }
  for (const expected of ["<redacted>", "<response_id>", "<session_id>", "<turn_state>"]) {
    if (!rendered.includes(expected)) {
      throw new Error(`scrubbed replay is missing expected placeholder: ${expected}`);
    }
  }
}

function assertNoObviousSecrets(rendered) {
  const violations = [
    /\bBearer\s+(?!<redacted>)[A-Za-z0-9._~+/-]+=*/i,
    /\bsk-(?!proj-<redacted>\b|ant-<redacted>\b|<redacted>\b)[A-Za-z0-9_-]{8,}\b/,
  ];
  for (const violation of violations) {
    if (violation.test(rendered)) {
      throw new Error(`scrubbed replay still matches obvious secret pattern: ${violation}`);
    }
  }
}

async function runSelfTest() {
  const fixture = buildFixture(sampleRecords(), "self-test.jsonl");
  const rendered = `${JSON.stringify(fixture, null, 2)}\n`;
  assertNoSampleSecrets(rendered);
  assertNoObviousSecrets(rendered);
  process.stdout.write("compat capture scrub self-test passed\n");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(usage());
    return;
  }
  if (args.selfTest) {
    await runSelfTest();
    return;
  }
  if (!args.input) {
    throw new Error("--input is required unless --self-test is used");
  }

  const inputPath = path.resolve(args.input);
  const records = await loadRecords(inputPath);
  const fixture = buildFixture(records, inputPath);
  const rendered = `${JSON.stringify(fixture, null, 2)}\n`;
  assertNoObviousSecrets(rendered);

  if (args.stdout) {
    process.stdout.write(rendered);
    return;
  }

  const outputPath = outputPathForArgs(args);
  if (args.check) {
    const expected = await fs.readFile(outputPath, "utf8");
    if (expected !== rendered) {
      throw new Error(`scrubbed replay fixture is stale: ${path.relative(repoRoot, outputPath)}`);
    }
    process.stdout.write(`checked ${path.relative(repoRoot, outputPath)}\n`);
    return;
  }

  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, rendered);
  process.stdout.write(`wrote ${path.relative(repoRoot, outputPath)}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
});
