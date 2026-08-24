#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import http from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";

function parseArgs(argv) {
  const args = {
    prodex: "./target/debug/prodex",
    codex0147: process.env.PRODEX_CODEX_0147_BIN,
    codex0148: process.env.PRODEX_CODEX_0148_BIN,
    codex0149: process.env.PRODEX_CODEX_0149_BIN,
  };
  const names = new Map([
    ["--prodex-bin", "prodex"],
    ["--codex-0147", "codex0147"],
    ["--codex-0148", "codex0148"],
    ["--codex-0149", "codex0149"],
  ]);
  for (let index = 2; index < argv.length; index += 2) {
    const key = names.get(argv[index]);
    if (!key || !argv[index + 1]) throw new Error(`unknown or incomplete argument: ${argv[index]}`);
    args[key] = argv[index + 1];
  }
  if (!args.codex0147 || !args.codex0148 || !args.codex0149) {
    throw new Error(
      "pass --codex-0147, --codex-0148, and --codex-0149 (or set the matching PRODEX_CODEX_*_BIN variables)",
    );
  }
  if (path.isAbsolute(args.prodex) || args.prodex.includes("/") || args.prodex.includes("\\")) {
    args.prodex = path.resolve(args.prodex);
  }
  return args;
}

function commandOutput(command, commandArgs, env = process.env) {
  const result = spawnSync(command, commandArgs, { encoding: "utf8", env });
  if (result.status !== 0) throw new Error(result.stderr || result.error || `${command} failed`);
  return result.stdout.trim();
}

function websocketFrame(value, opcode = 1) {
  const payload = Buffer.from(typeof value === "string" ? value : JSON.stringify(value));
  const header = payload.length < 126 ? Buffer.from([0x80 | opcode, payload.length]) : Buffer.alloc(4);
  if (payload.length >= 126) {
    header[0] = 0x80 | opcode;
    header[1] = 126;
    header.writeUInt16BE(payload.length, 2);
  }
  return Buffer.concat([header, payload]);
}

async function startUpstream() {
  const requests = [];
  const sockets = new Set();
  let connections = 0;
  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host}`);
    res.writeHead(url.pathname.endsWith("/responses") ? 426 : 404, { "content-type": "application/json" });
    res.end('{"error":"websocket_required"}\n');
  });
  server.on("upgrade", (req, socket, head) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host}`);
    const key = req.headers["sec-websocket-key"];
    if (!url.pathname.endsWith("/responses") || typeof key !== "string") {
      socket.destroy();
      return;
    }
    const connection = ++connections;
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    const accept = createHash("sha1").update(`${key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest("base64");
    socket.write(
      `HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`,
    );
    let buffered = head;
    socket.on("data", (chunk) => {
      buffered = Buffer.concat([buffered, chunk]);
      for (;;) {
        if (buffered.length < 2) return;
        const opcode = buffered[0] & 0x0f;
        let length = buffered[1] & 0x7f;
        let offset = 2;
        if (length === 126) {
          if (buffered.length < 4) return;
          length = buffered.readUInt16BE(2);
          offset = 4;
        } else if (length === 127) {
          if (buffered.length < 10) return;
          length = Number(buffered.readBigUInt64BE(2));
          offset = 10;
        }
        const masked = (buffered[1] & 0x80) !== 0;
        const maskBytes = masked ? 4 : 0;
        if (buffered.length < offset + maskBytes + length) return;
        const mask = masked ? buffered.subarray(offset, offset + 4) : null;
        offset += maskBytes;
        const payload = Buffer.from(buffered.subarray(offset, offset + length));
        buffered = buffered.subarray(offset + length);
        if (mask) for (let index = 0; index < payload.length; index += 1) payload[index] ^= mask[index % 4];
        if (opcode === 8) {
          socket.end(websocketFrame(payload, 8));
          return;
        }
        if (opcode === 9) {
          socket.write(websocketFrame(payload, 10));
          continue;
        }
        if (opcode !== 1) throw new Error(`unsupported websocket opcode ${opcode}`);
        const body = JSON.parse(payload.toString("utf8"));
        requests.push({ body, headers: req.headers, connection });
        const turnNumber = requests.filter((request) => request.body.input?.length > 0).length;
        const responseId = `R${turnNumber}`;
        socket.write(websocketFrame({ type: "response.created", response: { id: responseId } }));
        socket.write(
          websocketFrame({
            type: "response.output_item.done",
            item: {
              type: "message",
              role: "assistant",
              id: `msg_${requests.length}`,
              content: [{ type: "output_text", text: `TURN_${turnNumber}_OK` }],
            },
          }),
        );
        socket.write(
          websocketFrame({
            type: "response.completed",
            response: {
              id: responseId,
              usage: {
                input_tokens: 1,
                input_tokens_details: null,
                output_tokens: 1,
                output_tokens_details: null,
                total_tokens: 2,
              },
            },
          }),
        );
      }
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  assert(address && typeof address === "object");
  return {
    baseUrl: `http://127.0.0.1:${address.port}/backend-api/codex`,
    requests,
    reset: () => {
      requests.length = 0;
      connections = 0;
    },
    close: () => {
      for (const socket of sockets) socket.destroy();
      return new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
    },
  };
}

function appServer(child) {
  let nextId = 1;
  let output = "";
  let stderr = "";
  const pending = new Map();
  const notifications = [];
  const waiters = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => (stderr += chunk));
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    output += chunk;
    const lines = output.split(/\r?\n/);
    output = lines.pop();
    for (const line of lines) {
      if (!line.trim()) continue;
      const message = JSON.parse(line);
      if (message.id !== undefined && pending.has(message.id)) {
        const { resolve, reject } = pending.get(message.id);
        pending.delete(message.id);
        if (message.error) reject(new Error(JSON.stringify(message.error)));
        else resolve(message.result);
      } else if (message.method) {
        notifications.push(message);
        for (const wake of waiters.splice(0)) wake();
      }
    }
  });
  child.once("exit", (code, signal) => {
    const error = new Error(`app-server exited before replying (code=${code}, signal=${signal})`);
    for (const { reject } of pending.values()) reject(error);
    pending.clear();
    for (const wake of waiters.splice(0)) wake();
  });

  const request = (method, params) => {
    const id = nextId++;
    child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
    return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
  };
  const notify = (method, params) => child.stdin.write(`${JSON.stringify({ method, params })}\n`);
  const waitFor = async (method, predicate = () => true) => {
    const deadline = Date.now() + 20_000;
    for (;;) {
      const index = notifications.findIndex((message) => message.method === method && predicate(message.params));
      if (index >= 0) return notifications.splice(index, 1)[0].params;
      if (Date.now() >= deadline) throw new Error(`timed out waiting for ${method}: ${stderr}`);
      await new Promise((resolve) => {
        const timer = setTimeout(resolve, Math.min(250, deadline - Date.now()));
        waiters.push(() => {
          clearTimeout(timer);
          resolve();
        });
      });
    }
  };
  return { request, notify, waitFor };
}

async function reproduce(args, upstream, codex, expectedVersion) {
  assert.match(commandOutput(codex, ["--version"]), new RegExp(`\\b${expectedVersion.replaceAll(".", "\\.")}\\b`));
  upstream.reset();
  const root = mkdtempSync(path.join(tmpdir(), `prodex-previous-response-${expectedVersion}-`));
  const prodexHome = path.join(root, "prodex");
  const workspace = path.join(root, "workspace");
  mkdirSync(workspace, { recursive: true });
  const env = {
    ...process.env,
    HOME: path.join(root, "home"),
    PRODEX_HOME: prodexHome,
    PRODEX_SHARED_CODEX_HOME: path.join(root, "shared"),
    PRODEX_CODEX_BIN: codex,
    PRODEX_REPRO_API_KEY: "reproducer-not-a-secret",
  };
  mkdirSync(env.HOME, { recursive: true });
  commandOutput(args.prodex, ["profile", "add", "repro", "--activate", "--insecure"], env);
  writeFileSync(
    path.join(prodexHome, "profiles", "repro", "config.toml"),
    [
      'model = "mock-model"',
      'model_provider = "reproducer"',
      "",
      "[model_providers.reproducer]",
      'name = "Deterministic previous-response reproducer"',
      `base_url = ${JSON.stringify(upstream.baseUrl)}`,
      'env_key = "PRODEX_REPRO_API_KEY"',
      'wire_api = "responses"',
      "requires_openai_auth = false",
      "supports_websockets = true",
      "",
    ].join("\n"),
  );

  const child = spawn(
    args.prodex,
    ["run", "--profile", "repro", "--no-auto-rotate", "--skip-quota-check", "app-server", "--stdio"],
    { cwd: workspace, env, stdio: ["pipe", "pipe", "pipe"] },
  );
  const client = appServer(child);
  try {
    await client.request("initialize", {
      clientInfo: { name: "prodex-previous-response-reproducer", title: "Prodex reproducer", version: "1" },
      capabilities: null,
    });
    client.notify("initialized", {});
    const started = await client.request("thread/start", { cwd: workspace, model: "mock-model" });
    const threadId = started.thread.id;
    upstream.requests.length = 0;
    for (const text of ["Turn one", "Turn two"]) {
      const response = await client.request("turn/start", {
        threadId,
        input: [{ type: "text", text }],
      });
      await client.waitFor("turn/completed", (params) => params.threadId === threadId && params.turn.id === response.turn.id);
    }
    const turns = upstream.requests.filter((request) => request.body.input?.length > 0);
    assert.equal(turns.length, 2);
    assert.equal(turns[0].body.previous_response_id, undefined);
    assert.equal(turns[1].body.previous_response_id, "R1");
    assert.equal(turns[0].connection, turns[1].connection);
    assert.equal(turns[0].body.model, turns[1].body.model);
    assert.equal(turns[0].headers.authorization, turns[1].headers.authorization);
    return {
      codex: expectedVersion,
      response_id: "R1",
      previous_response_id: "R1",
      transport: "websocket",
      connection_generation: turns[0].connection,
      requests: 2,
    };
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = new Promise((resolve) => child.once("exit", resolve));
      child.kill("SIGTERM");
      await exited;
    }
    rmSync(root, { recursive: true, force: true });
  }
}

const args = parseArgs(process.argv);
assert.match(commandOutput(args.prodex, ["--version"]), /^prodex /);
const upstream = await startUpstream();
try {
  const results = [];
  results.push(await reproduce(args, upstream, args.codex0147, "0.147.0"));
  results.push(await reproduce(args, upstream, args.codex0148, "0.148.0"));
  results.push(await reproduce(args, upstream, args.codex0149, "0.149.1"));
  process.stdout.write(`${JSON.stringify({ status: "passed", provider: "reproducer", profile: "repro", model: "mock-model", rotation: false, results }, null, 2)}\n`);
} finally {
  await upstream.close();
}
