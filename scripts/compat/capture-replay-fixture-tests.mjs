#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const FIXTURE_DIR = path.join(repoRoot, "scripts/compat/replay-fixtures");

const FIXTURES = [
  {
    name: "codex SSE response stream",
    input: "codex_sse_capture.jsonl",
    expected: "codex_sse_capture_replay.json",
    assert(fixture, text) {
      assert.equal(fixture.source, "codex_sse_capture.jsonl");
      assert.deepEqual(
        fixture.records.map((record) => record.type),
        ["request", "sse_stream"],
      );

      const [request, stream] = fixture.records;
      assert.equal(request.method, "POST");
      assert.equal(request.path_and_query, "/backend-api/codex/responses?api-version=2026-04-01");
      assertHeader(request, "Authorization", "Bearer <redacted>");
      assertHeader(request, "ChatGPT-Account-Id", "<account_id>");
      assertHeader(request, "session_id", "<session_id>");
      assertHeader(request, "x-codex-turn-state", "<turn_state>");
      assertHeader(request, "x-codex-turn-metadata", "<turn_metadata>");
      assertHeader(request, "x-openai-subagent", "planner");
      assert.equal(request.body.previous_response_id, "<previous_response_id>");
      assert.equal(request.body.client_metadata.session_id, "<session_id>");
      assert.equal(request.body.input[1].approval_request_id, "<approval_request_id>");
      assert.equal(request.body.input[2].call_id, "<call_id>");

      assert.match(stream.stream, /^: keep-alive\nid: <id>\nevent: response.created/m);
      assert.match(stream.stream, /"x-codex-turn-state":"<turn_state>"/);
      assert.match(stream.stream, /"authorization_token":"<redacted>"/);
      assert.match(stream.stream, /data: \[DONE\]$/);
      assertNoNeedles(text, ["resp_sse_", "sess_sse_", "turn_sse_", "req_sse_", "call_sse_"]);
    },
  },
  {
    name: "codex WebSocket handshake and messages",
    input: "codex_websocket_capture.jsonl",
    expected: "codex_websocket_capture_replay.json",
    assert(fixture, text) {
      assert.equal(fixture.source, "codex_websocket_capture.jsonl");
      assert.deepEqual(
        fixture.records.map((record) => record.type),
        ["request", "websocket_message", "websocket_message"],
      );

      const [handshake, clientMessage, serverMessage] = fixture.records;
      assert.equal(handshake.method, "GET");
      assert.equal(handshake.path_and_query, "/backend-api/codex/responses?transport=websocket");
      assertHeader(handshake, "Authorization", "Bearer <redacted>");
      assertHeader(handshake, "ChatGPT-Account-Id", "<account_id>");
      assertHeader(handshake, "session_id", "<session_id>");
      assertHeader(handshake, "x-codex-turn-state", "<turn_state>");
      assertHeader(handshake, "Sec-WebSocket-Protocol", "codex.realtime");
      assertHeader(handshake, "Sec-WebSocket-Key", "<websocket_key>");
      assert.equal(clientMessage.direction, "client_to_upstream");
      assert.equal(clientMessage.message.previous_response_id, "<previous_response_id>");
      assert.equal(clientMessage.message.session_id, "<session_id>");
      assert.equal(clientMessage.message.input[1].call_id, "<call_id>");
      assert.deepEqual(clientMessage.message.input[1].arguments, {
        cmd: "pwd",
        token: "<redacted>",
      });
      assert.equal(serverMessage.direction, "upstream_to_client");
      assert.equal(serverMessage.message.response.id, "<id>");
      assert.equal(serverMessage.message.response.headers["x-codex-turn-state"], "<turn_state>");
      assertNoNeedles(text, ["resp_ws_", "sess_ws_", "turn_ws_", "req_ws_", "call_ws_"]);
    },
  },
  {
    name: "codex remote compact unary quota response",
    input: "codex_compact_capture.jsonl",
    expected: "codex_compact_capture_replay.json",
    assert(fixture, text) {
      assert.equal(fixture.source, "codex_compact_capture.jsonl");
      assert.deepEqual(
        fixture.records.map((record) => record.type),
        ["request", "response"],
      );

      const [request, response] = fixture.records;
      assert.equal(request.method, "POST");
      assert.equal(request.path_and_query, "/backend-api/codex/responses/compact?reason=remote");
      assertHeader(request, "Authorization", "Bearer <redacted>");
      assertHeader(request, "ChatGPT-Account-Id", "<account_id>");
      assertHeader(request, "session_id", "<session_id>");
      assert.equal(request.body.previous_response_id, "<previous_response_id>");
      assert.equal(request.body.session_id, "<session_id>");
      assert.equal(response.status, 429);
      assertHeader(response, "x-request-id", "<request_id>");
      assertHeader(response, "set-cookie", "<redacted>");
      assert.equal(response.body.error.code, "insufficient_quota");
      assert.equal(response.body.error.request_id, "<request_id>");
      assertNoNeedles(text, ["resp_compact_", "sess_compact_", "turn_compact_", "req_compact_"]);
    },
  },
];

function runCaptureReplay(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["scripts/compat/capture-replay.mjs", ...args], {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      resolve({ code: signal ? null : code, signal, stdout, stderr });
    });
  });
}

function assertCleanExit(label, result) {
  assert.equal(
    result.signal,
    null,
    `${label}: command signalled ${result.signal}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  assert.equal(
    result.code,
    0,
    `${label}: expected exit 0, got ${result.code}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
}

function headerMap(record) {
  return new Map(record.headers.map((header) => [header.name.toLowerCase(), header.value]));
}

function assertHeader(record, name, expectedValue) {
  assert.equal(headerMap(record).get(name.toLowerCase()), expectedValue, `${name} header`);
}

function assertNoNeedles(text, needles) {
  for (const needle of needles) {
    assert.equal(text.includes(needle), false, `golden replay should not contain raw value ${needle}`);
  }
  assert.doesNotMatch(text, /\bBearer\s+(?!<redacted>)[A-Za-z0-9._~+/-]+=*/i);
  assert.doesNotMatch(text, /\bsk-(?!proj-<redacted>\b|ant-<redacted>\b|<redacted>\b)[A-Za-z0-9_-]{8,}\b/);
}

async function checkBuiltInScrubSelfTest() {
  const result = await runCaptureReplay(["--self-test"]);
  assertCleanExit("capture replay self-test", result);
  assert.match(result.stdout, /compat capture scrub self-test passed/);
}

async function checkOfflineReplayGateWiring() {
  const packageJson = JSON.parse(await fs.readFile(path.join(repoRoot, "package.json"), "utf8"));
  assert.equal(
    packageJson.scripts?.["compat:offline-gate"],
    "npm run compat:check && npm run compat:replay-fixtures",
  );

  const workflow = await fs.readFile(path.join(repoRoot, ".github/workflows/ci.yml"), "utf8");
  assert.match(workflow, /^\s{2}compat-replay-gate:\n/m);
  assert.match(workflow, /Run offline upstream compat replay gate/);
  assert.match(workflow, /\bnpm run compat:offline-gate\b/);
}

async function checkFixture(spec) {
  const inputPath = path.join(FIXTURE_DIR, spec.input);
  const expectedPath = path.join(FIXTURE_DIR, spec.expected);
  const result = await runCaptureReplay(["--input", inputPath, "--output", expectedPath, "--check"]);
  assertCleanExit(spec.name, result);
  assert.match(result.stdout, new RegExp(`checked .*${spec.expected.replaceAll(".", "\\.")}`));

  const text = await fs.readFile(expectedPath, "utf8");
  const fixture = JSON.parse(text);
  assert.equal(fixture.format_version, 1);
  assert.equal(fixture.kind, "prodex_compat_replay_capture");
  assert.ok(Array.isArray(fixture.records));
  assertNoNeedles(text, [
    "fixture-redaction-bearer",
    "fixture-redaction-value",
    "acct_live_",
  ]);
  spec.assert(fixture, text);
}

let failures = 0;
try {
  await checkBuiltInScrubSelfTest();
  process.stdout.write("ok - capture replay scrub self-test\n");
} catch (error) {
  failures += 1;
  process.stderr.write(`not ok - capture replay scrub self-test\n  ${error.stack ?? error.message}\n`);
}

try {
  await checkOfflineReplayGateWiring();
  process.stdout.write("ok - offline replay gate wiring\n");
} catch (error) {
  failures += 1;
  process.stderr.write(`not ok - offline replay gate wiring\n  ${error.stack ?? error.message}\n`);
}

for (const fixture of FIXTURES) {
  try {
    await checkFixture(fixture);
    process.stdout.write(`ok - ${fixture.name}\n`);
  } catch (error) {
    failures += 1;
    process.stderr.write(`not ok - ${fixture.name}\n  ${error.stack ?? error.message}\n`);
  }
}

if (failures > 0) {
  process.stderr.write(`capture replay fixtures: ${failures} failed\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(`capture replay fixtures: ${FIXTURES.length + 2} passed\n`);
}
