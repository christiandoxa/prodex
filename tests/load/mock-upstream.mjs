#!/usr/bin/env node
import http from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCENARIO_FILE = path.join(__dirname, "scenarios.json");

function parseArgs(argv) {
  const args = {
    host: "127.0.0.1",
    port: 9900,
    scenario: "baseline",
    printReady: false,
    quiet: false,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    const key = value.replace(/^--/, "");
    const booleanKeys = new Set(["print-ready", "quiet"]);
    if (booleanKeys.has(key)) {
      args[key.replace(/-([a-z])/g, (_, char) => char.toUpperCase())] = true;
      continue;
    }
    index += 1;
    if (!argv[index]) {
      throw new Error(`${value} requires a value`);
    }
    const normalized = key.replace(/-([a-z])/g, (_, char) => char.toUpperCase());
    args[normalized] = argv[index];
  }
  args.port = Number(args.port);
  for (const key of [
    "firstByteMs",
    "chunkDelayMs",
    "jitterMs",
    "errorRate",
    "quotaRate",
    "generic429Rate",
    "slowEvery",
    "slowDelayMs",
  ]) {
    if (args[key] !== undefined) {
      args[key] = Number(args[key]);
    }
  }
  return args;
}

async function loadScenario(name) {
  const raw = await readFile(SCENARIO_FILE, "utf8");
  const parsed = JSON.parse(raw);
  const scenario = parsed.scenarios?.[name];
  if (!scenario) {
    throw new Error(`unknown scenario: ${name}`);
  }
  return scenario;
}

function mergeMockConfig(scenario, args) {
  const config = {
    firstByteMs: 50,
    chunkDelayMs: 15,
    jitterMs: 0,
    errorRate: 0,
    quotaRate: 0,
    generic429Rate: 0,
    slowEvery: 0,
    slowDelayMs: 0,
    ...(scenario.mock ?? {}),
  };
  for (const key of Object.keys(config)) {
    if (args[key] !== undefined) {
      config[key] = args[key];
    }
  }
  return config;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, Math.max(0, ms)));
}

function futureEpoch(offsetSeconds) {
  return Math.floor(Date.now() / 1000) + offsetSeconds;
}

function usageBody(accountId) {
  return {
    email: `${accountId || "load"}@example.test`,
    plan_type: "plus",
    rate_limit: {
      primary_window: {
        used_percent: 5,
        reset_at: futureEpoch(18_000),
        limit_window_seconds: 18_000,
      },
      secondary_window: {
        used_percent: 5,
        reset_at: futureEpoch(604_800),
        limit_window_seconds: 604_800,
      },
    },
  };
}

function writeJson(res, status, body, headers = {}) {
  res.writeHead(status, {
    "content-type": "application/json",
    ...headers,
  });
  res.end(`${JSON.stringify(body)}\n`);
}

function writeText(res, status, body, headers = {}) {
  res.writeHead(status, {
    "content-type": "text/plain",
    ...headers,
  });
  res.end(body);
}

function sse(event, payload) {
  return `event: ${event}\r\ndata: ${JSON.stringify(payload)}\r\n\r\n`;
}

function requestBody(req, maxBytes = 1_048_576) {
  return new Promise((resolve, reject) => {
    let body = "";
    req.setEncoding("utf8");
    req.on("data", (chunk) => {
      body += chunk;
      if (body.length > maxBytes) {
        reject(new Error("request body too large"));
        req.destroy();
      }
    });
    req.on("end", () => resolve(body));
    req.on("error", reject);
  });
}

function randomDelay(config, requestNumber) {
  let delay = config.firstByteMs + Math.floor(Math.random() * (config.jitterMs + 1));
  if (config.slowEvery > 0 && requestNumber % config.slowEvery === 0) {
    delay += config.slowDelayMs;
  }
  return delay;
}

function chooseInjectedFailure(config) {
  const roll = Math.random();
  if (roll < config.generic429Rate) {
    return "generic429";
  }
  if (roll < config.generic429Rate + config.quotaRate) {
    return "quota";
  }
  if (roll < config.generic429Rate + config.quotaRate + config.errorRate) {
    return "error";
  }
  return null;
}

function createMetrics() {
  return {
    startedAt: new Date().toISOString(),
    requests: 0,
    active: 0,
    maxActive: 0,
    responses: 0,
    compact: 0,
    usage: 0,
    injectedErrors: 0,
    injectedQuota: 0,
    injectedGeneric429: 0,
    byAccount: {},
  };
}

function noteRequest(metrics, accountId) {
  metrics.requests += 1;
  metrics.active += 1;
  metrics.maxActive = Math.max(metrics.maxActive, metrics.active);
  const account = accountId || "-";
  metrics.byAccount[account] = (metrics.byAccount[account] ?? 0) + 1;
  return () => {
    metrics.active -= 1;
  };
}

async function handleResponses(req, res, config, metrics, requestNumber, accountId) {
  await requestBody(req).catch(() => "");
  const done = noteRequest(metrics, accountId);
  metrics.responses += 1;
  try {
    const failure = chooseInjectedFailure(config);
    if (failure === "generic429") {
      metrics.injectedGeneric429 += 1;
      writeText(res, 429, "Too Many Requests", { "x-prodex-load-mock-error": "generic429" });
      return;
    }
    if (failure === "error") {
      metrics.injectedErrors += 1;
      writeJson(
        res,
        503,
        { error: { message: "mock upstream injected failure" }, status: 503 },
        { "x-prodex-load-mock-error": "upstream_error" },
      );
      return;
    }
    await sleep(randomDelay(config, requestNumber));
    const responseId = `resp_load_${requestNumber}`;
    const turnState = `turn_load_${requestNumber}`;
    res.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      "x-codex-turn-state": turnState,
    });
    if (failure === "quota") {
      metrics.injectedQuota += 1;
      res.write(
        sse("response.failed", {
          type: "response.failed",
          response_id: responseId,
          response: {
            id: responseId,
            error: {
              code: "insufficient_quota",
              message: "mock quota exhausted",
            },
          },
        }),
      );
      res.end();
      return;
    }
    const chunks = [
      sse("response.created", {
        type: "response.created",
        response_id: responseId,
        response: { id: responseId },
      }),
      sse("response.in_progress", {
        type: "response.in_progress",
        response_id: responseId,
        response: { id: responseId },
      }),
      sse("response.output_text.delta", {
        type: "response.output_text.delta",
        response_id: responseId,
        delta: "ok",
      }),
      sse("response.completed", {
        type: "response.completed",
        response_id: responseId,
        turn_state: turnState,
        response: {
          id: responseId,
          usage: {
            input_tokens: 10,
            cached_input_tokens: 0,
            output_tokens: 2,
            output_tokens_details: { reasoning_tokens: 0 },
          },
        },
      }),
    ];
    for (const chunk of chunks) {
      res.write(chunk);
      await sleep(config.chunkDelayMs);
    }
    res.end();
  } finally {
    done();
  }
}

async function handleCompact(req, res, config, metrics, requestNumber, accountId) {
  await requestBody(req).catch(() => "");
  const done = noteRequest(metrics, accountId);
  metrics.compact += 1;
  try {
    await sleep(randomDelay(config, requestNumber));
    writeJson(
      res,
      200,
      { output: [] },
      {
        "x-codex-turn-state": `compact_turn_load_${requestNumber}`,
      },
    );
  } finally {
    done();
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node tests/load/mock-upstream.mjs [--scenario baseline|stress|spike|soak] [--host 127.0.0.1] [--port 9900]",
        "",
        "Options override scenario mock config: --first-byte-ms, --chunk-delay-ms, --jitter-ms, --error-rate, --quota-rate, --generic-429-rate, --slow-every, --slow-delay-ms.",
      ].join("\n") + "\n",
    );
    return;
  }
  const scenario = await loadScenario(args.scenario);
  const config = mergeMockConfig(scenario, args);
  const metrics = createMetrics();
  let sequence = 0;

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "127.0.0.1"}`);
    const accountId = String(req.headers["chatgpt-account-id"] ?? "");
    const requestNumber = ++sequence;
    try {
      if (req.method === "GET" && url.pathname === "/__load/metrics") {
        writeJson(res, 200, metrics);
        return;
      }
      if (url.pathname.endsWith("/backend-api/wham/usage")) {
        metrics.usage += 1;
        writeJson(res, 200, usageBody(accountId));
        return;
      }
      if (url.pathname.endsWith("/backend-api/codex/responses/compact")) {
        await handleCompact(req, res, config, metrics, requestNumber, accountId);
        return;
      }
      if (url.pathname.endsWith("/backend-api/codex/responses")) {
        await handleResponses(req, res, config, metrics, requestNumber, accountId);
        return;
      }
      if (url.pathname.endsWith("/backend-api/status") || url.pathname === "/health") {
        writeJson(res, 200, { status: "ok" });
        return;
      }
      writeJson(res, 404, { error: "not_found", path: url.pathname });
    } catch (error) {
      writeJson(res, 500, { error: error instanceof Error ? error.message : String(error) });
    }
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(args.port, args.host, resolve);
  });

  const address = server.address();
  const port = typeof address === "object" && address ? address.port : args.port;
  const baseUrl = `http://${args.host}:${port}/backend-api`;
  const ready = { baseUrl, metricsUrl: `http://${args.host}:${port}/__load/metrics`, scenario: args.scenario };
  if (args.printReady) {
    process.stdout.write(`mock-upstream-ready ${JSON.stringify(ready)}\n`);
  }
  if (!args.quiet) {
    process.stdout.write(`mock upstream listening ${baseUrl}\n`);
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`mock-upstream: ${message}\n`);
  process.exitCode = 1;
}
