#!/usr/bin/env node
import assert from "node:assert/strict";
import { once } from "node:events";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

function parseArgs(argv) {
  const args = { binary: null, version: null, help: false };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--binary" || value === "--version") {
      index += 1;
      if (!argv[index]) throw new Error(`${value} requires a value`);
      args[value === "--binary" ? "binary" : "version"] =
        value === "--binary" ? path.resolve(argv[index]) : argv[index];
    } else if (value === "--help" || value === "-h") {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${value}`);
    }
  }
  if (!args.help && (!args.binary || !args.version)) {
    throw new Error("--binary and --version are required");
  }
  return args;
}

function usage() {
  return [
    "Usage: node scripts/ci/release-artifact-smoke.mjs --binary PATH --version VERSION",
    "",
    "Runs credential-free smoke checks against one downloaded release artifact.",
  ].join("\n") + "\n";
}

function run(command, args, { cwd, env, timeoutMs = 15_000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, timeoutMs);
    child.on("error", reject);
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal, stderr, stdout, timedOut });
    });
    child.stdin.end();
  });
}

async function stop(child) {
  if (!child || child.exitCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([once(child, "close"), sleep(3_000)]);
  if (child.exitCode === null) child.kill("SIGKILL");
  if (child.exitCode === null) await Promise.race([once(child, "close"), sleep(1_000)]);
}

async function startUpstream() {
  const state = { responses: 0 };
  const server = http.createServer((request, response) => {
    request.resume();
    request.on("end", async () => {
      if (!request.url?.endsWith("/backend-api/codex/responses")) {
        response.writeHead(404, { "content-type": "application/json" });
        response.end("{}");
        return;
      }
      state.responses += 1;
      response.writeHead(200, {
        "cache-control": "no-cache",
        "content-type": "text/event-stream",
        connection: "keep-alive",
      });
      const emit = (type, value) => {
        response.write(`event: ${type}\r\ndata: ${JSON.stringify(value)}\r\n\r\n`);
      };
      emit("response.created", { type: "response.created", response: { id: "smoke-response" } });
      emit("response.in_progress", {
        type: "response.in_progress",
        response: { id: "smoke-response" },
      });
      emit("response.output_item.added", {
        type: "response.output_item.added",
        item: { type: "message", id: "smoke-message" },
      });
      emit("response.content_part.added", {
        type: "response.content_part.added",
        response: { id: "smoke-response" },
      });
      emit("response.output_text.delta", {
        type: "response.output_text.delta",
        response: { id: "smoke-response" },
        delta: "artifact",
      });
      await sleep(300);
      emit("response.output_text.delta", {
        type: "response.output_text.delta",
        response: { id: "smoke-response" },
        delta: " smoke",
      });
      await sleep(300);
      emit("response.completed", {
        type: "response.completed",
        response: {
          id: "smoke-response",
          usage: { input_tokens: 10, output_tokens: 20 },
        },
      });
      response.end();
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const { port } = server.address();
  return { baseUrl: `http://127.0.0.1:${port}/backend-api`, server, state };
}

function request(url, body) {
  return new Promise((resolve, reject) => {
    const target = new URL(url);
    const payload = JSON.stringify(body);
    const client = http.request(
      target,
      {
        headers: {
          accept: "text/event-stream",
          authorization: "Bearer artifact-smoke-client",
          "content-length": Buffer.byteLength(payload),
          "content-type": "application/json",
        },
        method: "POST",
      },
      (response) => {
        const chunks = [];
        response.setEncoding("utf8");
        response.on("data", (chunk) => chunks.push(chunk));
        response.on("end", () =>
          resolve({ body: chunks.join(""), status: response.statusCode ?? 0 }),
        );
      },
    );
    client.setTimeout(15_000, () => client.destroy(new Error("artifact smoke request timed out")));
    client.on("error", reject);
    client.end(payload);
  });
}

async function waitForRegistry(pathname, broker) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    if (broker.child.exitCode !== null) {
      throw new Error(`runtime broker exited before publishing its registry: ${broker.stderr}`);
    }
    try {
      const registry = JSON.parse(await fs.readFile(pathname, "utf8"));
      if (registry.instance_id === "artifact-smoke-instance" && registry.listen_addr) {
        return registry;
      }
    } catch {
      // The broker writes the registry atomically; retry until it is ready.
    }
    await sleep(100);
  }
  throw new Error(`timed out waiting for runtime broker registry: ${broker.stderr}`);
}

async function runTui(binary, env, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "script",
      ["-qefc", 'stty cols 100 rows 32; exec "$PRODEX_SMOKE_BINARY" log stream', "/dev/null"],
      { cwd, env: { ...env, PRODEX_SMOKE_BINARY: binary }, stdio: ["pipe", "pipe", "pipe"] },
    );
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
    const quitTimer = setTimeout(() => child.stdin.write("q"), 1_200);
    const killTimer = setTimeout(() => child.kill("SIGKILL"), 8_000);
    child.on("error", reject);
    child.on("close", (code, signal) => {
      clearTimeout(quitTimer);
      clearTimeout(killTimer);
      resolve({ code, signal, stderr, stdout });
    });
  });
}

async function readUsageFromLogLast(binary, env, cwd) {
  const deadline = Date.now() + 10_000;
  let last;
  while (Date.now() < deadline) {
    const result = await run(binary, ["log", "last", "--json"], { cwd, env });
    last = result;
    if (result.code === 0) {
      for (const line of result.stdout.trim().split(/\r?\n/u).filter(Boolean)) {
        try {
          const event = JSON.parse(line);
          if (event.output_tokens === 20) return event;
        } catch {
          // Retry while the runtime logger finishes its bounded write.
        }
      }
    }
    await sleep(100);
  }
  throw new Error(`timed out waiting for parsed token usage: ${last?.stderr || last?.stdout || "no output"}`);
}

function stripAnsi(value) {
  return value
    .replace(/\u001b\][^\u0007]*(?:\u0007|\u001b\\)/gu, "")
    .replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, "")
    .replaceAll("\r", "");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(usage());
    return;
  }

  const binaryStat = await fs.stat(args.binary);
  assert(binaryStat.isFile(), `artifact is not a file: ${args.binary}`);
  const smokeRoot = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-release-artifact-smoke-"));
  let upstream;
  let broker;
  try {
    const prodexHome = path.join(smokeRoot, "prodex-home");
    const profileHome = path.join(prodexHome, "profiles", "main");
    const runtimeLogDir = path.join(smokeRoot, "runtime-logs");
    const fakeBin = path.join(smokeRoot, "fake-bin");
    const fakeCodex = path.join(fakeBin, "codex");
    const fakeCodexArgs = path.join(smokeRoot, "codex-args.log");
    await fs.mkdir(profileHome, { recursive: true });
    await fs.mkdir(runtimeLogDir, { recursive: true });
    await fs.mkdir(path.join(smokeRoot, "home"), { recursive: true });
    await fs.mkdir(fakeBin, { recursive: true });
    await fs.chmod(prodexHome, 0o700);
    await fs.chmod(path.join(prodexHome, "profiles"), 0o700);
    await fs.chmod(profileHome, 0o700);
    await fs.writeFile(
      path.join(prodexHome, "state.json"),
      JSON.stringify({
        active_profile: "main",
        profiles: {
          main: { codex_home: profileHome, managed: true, provider_kind: "openai" },
        },
        schema_version: 1,
      }),
    );
    await fs.writeFile(
      path.join(profileHome, "auth.json"),
      JSON.stringify({ tokens: { access_token: "artifact-smoke-token", account_id: "artifact-smoke-account" } }),
      { mode: 0o600 },
    );
    await fs.chmod(path.join(profileHome, "auth.json"), 0o600);
    await fs.writeFile(
      fakeCodex,
      `#!/bin/sh
set -eu
printf '%s\\n' "$@" > "$SMOKE_CODEX_ARGS"
[ "$1" = exec ]
case " $* " in
  *" --json "*) ;;
  *) exit 91 ;;
esac
printf '%s\\n' '{"type":"thread.started","thread_id":"artifact-smoke-thread"}'
printf '%s\\n' '{"type":"turn.started"}'
printf '%s\\n' '{"type":"item.completed","item":{"type":"agent_message","text":"artifact smoke response"}}'
printf '%s\\n' '{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":2}}'
`,
    );
    await fs.chmod(fakeCodex, 0o755);

    const env = {
      HOME: path.join(smokeRoot, "home"),
      PATH: "/usr/bin:/bin",
      PRODEX_CODEX_BIN: fakeCodex,
      PRODEX_HOME: prodexHome,
      PRODEX_RUNTIME_LOG_RECORD: "1",
      PRODEX_RUNTIME_LOG_DIR: runtimeLogDir,
      SMOKE_CODEX_ARGS: fakeCodexArgs,
      TERM: "xterm-256color",
    };

    const version = await run(args.binary, ["--version"], { cwd: smokeRoot, env });
    assert.equal(version.code, 0, version.stderr || version.stdout);
    assert.ok(`${version.stdout}${version.stderr}`.includes(args.version), args.version);

    const ping = await run(args.binary, ["ping", "openai", "--profile", "main", "--json"], {
      cwd: smokeRoot,
      env,
    });
    assert.equal(ping.code, 0, ping.stderr || ping.stdout);
    const pingReport = JSON.parse(ping.stdout.trim());
    assert.equal(pingReport.status, "ok");
    assert.deepEqual(pingReport.profiles, [
      {
        detail: "valid model response received",
        latency_ms: pingReport.profiles[0].latency_ms,
        model: null,
        profile: "main",
        status: "ok",
      },
    ]);
    const codexArgs = await fs.readFile(fakeCodexArgs, "utf8");
    assert.match(codexArgs, /^exec$/m);
    assert.match(codexArgs, /^--json$/m);

    upstream = await startUpstream();
    broker = (() => {
      const child = spawn(args.binary, ["__runtime-broker"], {
        cwd: smokeRoot,
        env,
        stdio: ["pipe", "ignore", "pipe"],
      });
      let stderr = "";
      child.stderr.setEncoding("utf8");
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
      });
      child.stdin.end(
        JSON.stringify({
          admin_token: "artifact-smoke-admin",
          broker_key: "artifact-smoke",
          current_profile: "main",
          include_code_review: false,
          instance_id: "artifact-smoke-instance",
          listen_addr: "127.0.0.1:0",
          model_context_window_tokens: null,
          smart_context_enabled: false,
          upstream_base_url: upstream.baseUrl,
          upstream_no_proxy: true,
          version: 1,
        }),
      );
      return { child, get stderr() { return stderr; } };
    })();
    const registry = await waitForRegistry(
      path.join(prodexHome, "runtime-broker-artifact-smoke.json"),
      broker,
    );
    const response = await request(
      `http://${registry.listen_addr}/backend-api/prodex/responses`,
      {
        input: [{ content: [{ text: "artifact smoke" }], role: "user" }],
        model: "gpt-5.6-luna",
        stream: true,
      },
    );
    assert.equal(response.status, 200, response.body);
    assert.match(response.body, /response\.completed/);
    assert.equal(upstream.state.responses, 1);

    const usage = await readUsageFromLogLast(args.binary, env, smokeRoot);
    assert.equal(usage.profile, "main");
    assert.equal(usage.source, "responses_sse");
    assert.ok(usage.generation_ms > 0, JSON.stringify(usage));
    assert.ok(usage.output_tokens_per_second > 0, JSON.stringify(usage));

    const tui = await runTui(args.binary, env, smokeRoot);
    const tuiText = stripAnsi(`${tui.stdout}${tui.stderr}`);
    assert.equal(tui.code, 0, tuiText);
    assert.match(tuiText, /Prodex Log/);
    assert.match(tuiText, /output\s+20/u);
    assert.match(tuiText, /\d+(?:\.\d+)? t\/s/u);
    process.stdout.write("release artifact smoke passed\n");
  } finally {
    await stop(broker?.child);
    if (upstream) await new Promise((resolve) => upstream.server.close(resolve));
    await fs.rm(smokeRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  process.stderr.write(`release artifact smoke failed: ${error.message}\n`);
  process.exitCode = 1;
});
