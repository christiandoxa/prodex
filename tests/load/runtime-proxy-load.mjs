#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { chmod, mkdtemp, mkdir, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCENARIO_FILE = path.join(__dirname, "scenarios.json");
const MOCK_UPSTREAM = path.join(__dirname, "mock-upstream.mjs");
const MAX_BODY_CAPTURE_BYTES = 1024 * 1024;
const MAX_CLIENT_READ_DELAY_MS = 1_000;
const MAX_REQUEST_TIMEOUT_MS = 120_000;
const MAX_SCENARIO_CONCURRENCY = 256;
const MAX_SCENARIO_REQUESTS = 100_000;
const MAX_SCENARIO_DURATION_SECONDS = 3_600;
const BROKER_METRICS_PATH = "/__prodex/runtime/metrics";
const BROKER_ADMIN_TOKEN_HEADER = "X-Prodex-Admin-Token";
const BROKER_ALLOCATION_FIELDS = [
  ["allocCalls", "alloc_calls"],
  ["reallocCalls", "realloc_calls"],
  ["deallocCalls", "dealloc_calls"],
  ["allocatedBytes", "allocated_bytes"],
  ["reallocatedBytes", "reallocated_bytes"],
  ["deallocatedBytes", "deallocated_bytes"],
  ["liveBytes", "live_bytes"],
  ["peakLiveBytes", "peak_live_bytes"],
];
const BROKER_ALLOCATION_CUMULATIVE_FIELDS = [
  "allocCalls",
  "reallocCalls",
  "deallocCalls",
  "allocatedBytes",
  "reallocatedBytes",
  "deallocatedBytes",
  "peakLiveBytes",
];
const PRESSURE_MARKERS = [
  "runtime_proxy_queue_overloaded",
  "runtime_proxy_active_limit_reached",
  "runtime_proxy_lane_limit_reached",
  "runtime_proxy_overload_backoff",
  "profile_inflight_saturated",
  "precommit_budget_exhausted",
];

function parseArgs(argv) {
  const args = {
    scenario: "baseline",
    target: null,
    route: null,
    compactEvery: null,
    startMock: false,
    startProxy: false,
    prodex: "target/debug/prodex",
    profiles: 3,
    listenAddr: null,
    prodexHome: null,
    runtimeLogDir: null,
    runtimeLog: null,
    json: false,
    upstreamNoProxy: true,
    authToken: "load-client-token",
  };
  const booleans = new Set([
    "start-mock",
    "start-proxy",
    "json",
    "upstream-proxy",
    "self-test",
    "dry-run",
  ]);
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    const key = value.replace(/^--/, "");
    if (booleans.has(key)) {
      if (key === "upstream-proxy") {
        args.upstreamNoProxy = false;
      } else {
        args[key.replace(/-([a-z])/g, (_, char) => char.toUpperCase())] = true;
      }
      continue;
    }
    index += 1;
    if (!argv[index]) {
      throw new Error(`${value} requires a value`);
    }
    const normalized = key.replace(/-([a-z])/g, (_, char) => char.toUpperCase());
    args[normalized] = argv[index];
  }
  for (const key of [
    "requests",
    "concurrency",
    "durationSec",
    "profiles",
    "compactEvery",
    "maxErrorRate",
    "maxTtftP95Ms",
    "maxAdmissionPressureRate",
    "clientReadDelayMs",
    "requestTimeoutMs",
  ]) {
    if (args[key] !== undefined && args[key] !== null) {
      args[key] = Number(args[key]);
    }
  }
  return args;
}

function validateLaunchMode(args) {
  if (args.gatewayBinary && args.startProxy) {
    throw new Error("--gateway-binary and --start-proxy are mutually exclusive");
  }
}

async function loadScenarios() {
  const raw = await readFile(SCENARIO_FILE, "utf8");
  const scenarios = JSON.parse(raw).scenarios;
  for (const [name, scenario] of Object.entries(scenarios)) validateScenario(name, scenario);
  return scenarios;
}

function boundedInteger(value, minimum, maximum, name) {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    throw new Error(`${name} must be between ${minimum} and ${maximum}`);
  }
  return value;
}

function validateScenario(name, scenario) {
  if (!scenario || typeof scenario !== "object" || !Array.isArray(scenario.stages)) {
    throw new Error(`scenario ${name} must define stages`);
  }
  for (const stage of scenario.stages) validateStage(name, stage);
  clientConfig(scenario, {});
}

function validateStage(name, stage) {
  boundedInteger(
    Number(stage.concurrency ?? 1),
    1,
    MAX_SCENARIO_CONCURRENCY,
    `${name} concurrency`,
  );
  if (stage.requests === undefined && stage.durationSec === undefined) {
    throw new Error(`${name} must bound each stage by requests or durationSec`);
  }
  if (stage.requests !== undefined) {
    boundedInteger(Number(stage.requests), 1, MAX_SCENARIO_REQUESTS, `${name} requests`);
  }
  if (stage.durationSec !== undefined) {
    boundedInteger(
      Number(stage.durationSec),
      1,
      MAX_SCENARIO_DURATION_SECONDS,
      `${name} durationSec`,
    );
  }
}

function clientConfig(scenario, args) {
  const readDelayMs = Number(args.clientReadDelayMs ?? scenario.client?.readDelayMs ?? 0);
  const requestTimeoutMs = Number(
    args.requestTimeoutMs ?? scenario.client?.requestTimeoutMs ?? 30_000,
  );
  boundedInteger(readDelayMs, 0, MAX_CLIENT_READ_DELAY_MS, "client read delay");
  boundedInteger(requestTimeoutMs, 1_000, MAX_REQUEST_TIMEOUT_MS, "request timeout");
  return { readDelayMs, requestTimeoutMs };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function freePort(host = "127.0.0.1") {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, host, () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : null;
      server.close(() => {
        if (port) {
          resolve(port);
        } else {
          reject(new Error("failed to allocate free port"));
        }
      });
    });
  });
}

function percentile(values, percentileValue) {
  if (values.length === 0) {
    return 0;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.ceil((percentileValue / 100) * sorted.length) - 1);
  return sorted[index];
}

function round(value, digits = 2) {
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

function normalizeBaseUrl(target) {
  return target.replace(/\/+$/, "");
}

function publicUpstreamUrl(value) {
  const url = new URL(value);
  if (!["http:", "https:"].includes(url.protocol)) {
    throw new Error("gateway upstream URL must use http or https");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("gateway upstream URL must not contain credentials, query, or fragment");
  }
  return normalizeBaseUrl(url.toString());
}

function gatewayPolicy(upstreamBaseUrl) {
  return `version = 1

[secrets]
production = false

[gateway]
base_url = ${JSON.stringify(publicUpstreamUrl(upstreamBaseUrl))}
require_auth = false
`;
}

function routePath(route, gatewayMode = false) {
  if (route === "compact") {
    return gatewayMode ? "/responses/compact" : "/codex/responses/compact";
  }
  return gatewayMode ? "/responses" : "/codex/responses";
}

function isPressureResponse(status, body) {
  const lower = body.toLowerCase();
  return (
    status === 503 &&
    (lower.includes("service_unavailable") ||
      lower.includes("admission") ||
      lower.includes("active_limit") ||
      lower.includes("lane_limit") ||
      lower.includes("proxy capacity") ||
      lower.includes("overloaded_error"))
  );
}

function requestPayload(route, id) {
  if (route === "compact") {
    return {
      model: "gpt-5.4",
      input: [],
      previous_response_id: `resp_load_seed_${id}`,
    };
  }
  return {
    model: "gpt-5.4",
    stream: true,
    input: [
      {
        role: "user",
        content: [
          {
            type: "input_text",
            text: `prodex load request ${id}`,
          },
        ],
      },
    ],
  };
}

async function sendRequest(baseUrl, route, id, authToken, client, gatewayMode) {
  const startedAt = performance.now();
  let status = 0;
  let firstByteAt = null;
  let body = "";
  try {
    const response = await fetch(`${baseUrl}${routePath(route, gatewayMode)}`, {
      method: "POST",
      headers: {
        ...(authToken ? { authorization: `Bearer ${authToken}` } : {}),
        "content-type": "application/json",
        "chatgpt-account-id": "load-client-account",
        "session_id": `load-session-${id % 32}`,
        "x-codex-turn-state": `load-client-turn-${id % 64}`,
        "user-agent": "prodex-load-harness/1",
      },
      body: JSON.stringify(requestPayload(route, id)),
      signal: AbortSignal.timeout(client.requestTimeoutMs),
    });
    status = response.status;
    const reader = response.body?.getReader();
    if (!reader) {
      body = await response.text();
      firstByteAt = performance.now();
    } else {
      const decoder = new TextDecoder();
      let reads = 0;
      while (true) {
        if (reads > 0 && client.readDelayMs > 0) await sleep(client.readDelayMs);
        const { done, value } = await reader.read();
        if (done) {
          if (body.length < MAX_BODY_CAPTURE_BYTES) body += decoder.decode();
          break;
        }
        reads += 1;
        if (firstByteAt === null) {
          firstByteAt = performance.now();
        }
        if (body.length < MAX_BODY_CAPTURE_BYTES) {
          body += decoder
            .decode(value, { stream: true })
            .slice(0, MAX_BODY_CAPTURE_BYTES - body.length);
        }
      }
    }
    const finishedAt = performance.now();
    return {
      route,
      status,
      ok: status >= 200 && status < 300,
      ttftMs: (firstByteAt ?? finishedAt) - startedAt,
      latencyMs: finishedAt - startedAt,
      pressure: isPressureResponse(status, body),
      error: null,
    };
  } catch (error) {
    const finishedAt = performance.now();
    return {
      route,
      status,
      ok: false,
      ttftMs: finishedAt - startedAt,
      latencyMs: finishedAt - startedAt,
      pressure: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function stagePlan(scenario, args) {
  let stages;
  if (
    args.requests !== undefined ||
    args.durationSec !== undefined ||
    args.concurrency !== undefined
  ) {
    stages = [
      {
        name: "override",
        requests: args.requests,
        durationSec: args.durationSec,
        concurrency: args.concurrency ?? 1,
      },
    ];
  } else {
    stages = scenario.stages ?? [{ name: args.scenario, requests: 100, concurrency: 8 }];
  }
  for (const stage of stages) validateStage(args.scenario, stage);
  return stages;
}

function selfTestScenarios(scenarios) {
  assert.equal(
    parseArgs(["node", "load", "--gateway-binary", "./prodex-gateway"]).gatewayBinary,
    "./prodex-gateway",
  );
  assert.throws(
    () => validateLaunchMode({ gatewayBinary: "./prodex-gateway", startProxy: true }),
    /mutually exclusive/,
  );
  assert.equal(routePath("responses", true), "/responses");
  assert.equal(routePath("compact", true), "/responses/compact");
  assert.match(gatewayPolicy("http://127.0.0.1:1234/backend-api"), /require_auth = false/);
  assert.doesNotMatch(
    gatewayPolicy("http://127.0.0.1:1234/backend-api"),
    /(?:token|api[_-]?key|password)\s*=/i,
  );
  assert.throws(() => publicUpstreamUrl("http://user:password@127.0.0.1/backend-api"));
  assert.ok(clientConfig(scenarios["slow-client"], {}).readDelayMs > 0);
  assert.ok(scenarios["slow-upstream"].mock.firstByteMs >= 500);
  assert.ok(scenarios["long-stream"].mock.outputChunks >= 32);
  assert.throws(() =>
    validateStage("unbounded", {
      concurrency: 1,
    }),
  );
  assert.throws(() => clientConfig({ client: { readDelayMs: MAX_CLIENT_READ_DELAY_MS + 1 } }, {}));
  assert.deepEqual(
    waitDurationEvidence(
      { waitTotalNs: 100, waitCount: 2, waitMaxNs: 80 },
      { waitTotalNs: 175, waitCount: 5, waitMaxNs: 90 },
      true,
    ),
    {
      status: "captured",
      start: { waitTotalNs: 100, waitCount: 2, waitMaxNs: 80 },
      end: { waitTotalNs: 175, waitCount: 5, waitMaxNs: 90 },
      delta: { waitTotalNs: 75, waitCount: 3, waitMaxNs: 90, waitMeanNs: 25 },
    },
  );
  const allocationStart = brokerAllocationSnapshot({
    allocation: {
      alloc_calls: 100,
      realloc_calls: 20,
      dealloc_calls: 90,
      allocated_bytes: 1_000,
      reallocated_bytes: 400,
      deallocated_bytes: 800,
      live_bytes: 600,
      peak_live_bytes: 700,
    },
  });
  const allocationEnd = brokerAllocationSnapshot({
    allocation: {
      alloc_calls: 160,
      realloc_calls: 30,
      dealloc_calls: 140,
      allocated_bytes: 2_200,
      reallocated_bytes: 800,
      deallocated_bytes: 1_600,
      live_bytes: 1_400,
      peak_live_bytes: 1_500,
    },
  });
  assert.equal(brokerAllocationSnapshot({}).status, "unsupported");
  assert.equal(
    brokerAllocationSnapshot({
      allocation: {
        alloc_calls: Number.MAX_SAFE_INTEGER + 1,
        realloc_calls: 0,
        dealloc_calls: 0,
        allocated_bytes: 0,
        reallocated_bytes: 0,
        deallocated_bytes: 0,
        live_bytes: 0,
        peak_live_bytes: 0,
      },
    }).status,
    "invalid",
  );
  const zeroAllocation = Object.fromEntries(
    BROKER_ALLOCATION_FIELDS.map(([field]) => [field, 0]),
  );
  assert.equal(
    allocationPerRequestEvidence(
      { status: "captured", snapshot: zeroAllocation },
      {
        status: "captured",
        snapshot: { ...zeroAllocation, allocCalls: Number.MAX_SAFE_INTEGER, reallocCalls: 1 },
      },
      true,
      1,
    ).status,
    "not_captured",
  );
  const performance = performanceEvidence(
    true,
    {
      allocation: allocationStart,
      admissionWait: { waitTotalNs: 10, waitCount: 1, waitMaxNs: 10 },
      longLivedQueueWait: { waitTotalNs: 20, waitCount: 2, waitMaxNs: 12 },
      runtimeStateLockWait: { waitTotalNs: 30, waitCount: 3, waitMaxNs: 15 },
    },
    {
      allocation: allocationEnd,
      admissionWait: { waitTotalNs: 50, waitCount: 3, waitMaxNs: 25 },
      longLivedQueueWait: { waitTotalNs: 70, waitCount: 4, waitMaxNs: 30 },
      runtimeStateLockWait: { waitTotalNs: 90, waitCount: 6, waitMaxNs: 35 },
    },
    20,
  );
  assert.deepEqual(performance.allocationPerRequest.delta, {
    allocCalls: 60,
    reallocCalls: 10,
    deallocCalls: 50,
    allocatedBytes: 1_200,
    reallocatedBytes: 400,
    deallocatedBytes: 800,
    liveBytes: 800,
    peakLiveBytes: 800,
  });
  assert.equal(performance.allocationPerRequest.allocationOperationsPerRequest, 3.5);
  assert.equal(performance.allocationPerRequest.requestedBytesPerRequest, 80);
  assert.equal(
    performanceEvidence(false, null, null, 20).allocationPerRequest.status,
    "unsupported",
  );
  assert.equal(
    performanceEvidence(
      true,
      { allocation: { status: "unsupported" } },
      { allocation: { status: "unsupported" } },
      20,
    ).allocationPerRequest.status,
    "unsupported",
  );
  assert.deepEqual(performance.admissionWait.delta, {
    waitTotalNs: 40,
    waitCount: 2,
    waitMaxNs: 25,
    waitMeanNs: 20,
  });
  assert.deepEqual(performance.longLivedQueueWait.delta, {
    waitTotalNs: 50,
    waitCount: 2,
    waitMaxNs: 30,
    waitMeanNs: 25,
  });
}

function routeForRequest(scenario, args, id) {
  const route = args.route ?? scenario.route ?? "responses";
  if (route !== "mixed") {
    return route;
  }
  const compactEvery = args.compactEvery ?? scenario.compactEvery ?? 5;
  return compactEvery > 0 && id % compactEvery === 0 ? "compact" : "responses";
}

async function runStage(stage, scenario, args, baseUrl, state) {
  const concurrency = Math.max(1, Number(stage.concurrency ?? 1));
  const results = [];
  const startedAt = Date.now();
  const endsAt = stage.durationSec ? startedAt + Number(stage.durationSec) * 1000 : null;
  let issued = 0;
  const maxRequests = stage.requests ? Number(stage.requests) : Number.POSITIVE_INFINITY;
  const client = clientConfig(scenario, args);

  async function worker() {
    while (issued < maxRequests) {
      if (endsAt && Date.now() >= endsAt) {
        break;
      }
      issued += 1;
      const id = ++state.sequence;
      const route = routeForRequest(scenario, args, id);
      results.push(
        await sendRequest(
          baseUrl,
          route,
          id,
          args.gatewayBinary ? null : args.authToken,
          client,
          Boolean(args.gatewayBinary),
        ),
      );
    }
  }

  await Promise.all(Array.from({ length: concurrency }, () => worker()));
  return { name: stage.name ?? "stage", results };
}

function summarize(results, pressureMarkers) {
  const total = results.length;
  const failures = results.filter((result) => !result.ok).length;
  const pressureResponses = results.filter((result) => result.pressure).length;
  const ttft = results.map((result) => result.ttftMs);
  const latency = results.map((result) => result.latencyMs);
  const byStatus = {};
  const byRoute = {};
  for (const result of results) {
    byStatus[result.status] = (byStatus[result.status] ?? 0) + 1;
    byRoute[result.route] = (byRoute[result.route] ?? 0) + 1;
  }
  return {
    total,
    ok: total - failures,
    failures,
    errorRate: total ? failures / total : 0,
    ttftMs: {
      p50: round(percentile(ttft, 50)),
      p95: round(percentile(ttft, 95)),
      p99: round(percentile(ttft, 99)),
      max: round(Math.max(0, ...ttft)),
    },
    latencyMs: {
      p50: round(percentile(latency, 50)),
      p95: round(percentile(latency, 95)),
      p99: round(percentile(latency, 99)),
      max: round(Math.max(0, ...latency)),
    },
    admissionPressure: {
      responses: pressureResponses,
      markers: pressureMarkers.count,
      rate: total ? (pressureResponses + pressureMarkers.count) / total : 0,
      markerBreakdown: pressureMarkers.byMarker,
    },
    byStatus,
    byRoute,
  };
}

function thresholdsFor(scenario, args) {
  return {
    ...(scenario.thresholds ?? {}),
    ...(args.maxErrorRate === undefined ? {} : { maxErrorRate: args.maxErrorRate }),
    ...(args.maxTtftP95Ms === undefined ? {} : { maxTtftP95Ms: args.maxTtftP95Ms }),
    ...(args.maxAdmissionPressureRate === undefined
      ? {}
      : { maxAdmissionPressureRate: args.maxAdmissionPressureRate }),
  };
}

function thresholdFailures(summary, thresholds) {
  const failures = [];
  if (
    thresholds.maxErrorRate !== undefined &&
    summary.errorRate > Number(thresholds.maxErrorRate)
  ) {
    failures.push(`error_rate ${round(summary.errorRate, 4)} > ${thresholds.maxErrorRate}`);
  }
  if (
    thresholds.maxTtftP95Ms !== undefined &&
    summary.ttftMs.p95 > Number(thresholds.maxTtftP95Ms)
  ) {
    failures.push(`ttft_p95_ms ${summary.ttftMs.p95} > ${thresholds.maxTtftP95Ms}`);
  }
  if (
    thresholds.maxAdmissionPressureRate !== undefined &&
    summary.admissionPressure.rate > Number(thresholds.maxAdmissionPressureRate)
  ) {
    failures.push(
      `admission_pressure_rate ${round(summary.admissionPressure.rate, 4)} > ${thresholds.maxAdmissionPressureRate}`,
    );
  }
  return failures;
}

async function scanPressureMarkers(args) {
  const files = [];
  if (args.runtimeLog) {
    files.push(args.runtimeLog);
  }
  if (args.runtimeLogDir) {
    const latestPointer = path.join(args.runtimeLogDir, "prodex-runtime-latest.path");
    try {
      const latest = (await readFile(latestPointer, "utf8")).trim();
      if (latest) {
        files.push(latest);
      }
    } catch {
      const entries = await readdir(args.runtimeLogDir).catch(() => []);
      const candidates = [];
      for (const entry of entries) {
        if (!/^prodex-runtime-.*\.log$/.test(entry)) {
          continue;
        }
        const fullPath = path.join(args.runtimeLogDir, entry);
        const fileStat = await stat(fullPath).catch(() => null);
        if (fileStat) {
          candidates.push({ fullPath, mtimeMs: fileStat.mtimeMs });
        }
      }
      candidates.sort((left, right) => right.mtimeMs - left.mtimeMs);
      if (candidates[0]) {
        files.push(candidates[0].fullPath);
      }
    }
  }
  const byMarker = {};
  for (const file of [...new Set(files)]) {
    let text = "";
    try {
      text = await readFile(file, "utf8");
    } catch {
      continue;
    }
    for (const marker of PRESSURE_MARKERS) {
      const count = text.split(marker).length - 1;
      if (count > 0) {
        byMarker[marker] = (byMarker[marker] ?? 0) + count;
      }
    }
  }
  return {
    count: Object.values(byMarker).reduce((sum, count) => sum + count, 0),
    byMarker,
  };
}

function waitDurationEvidence(start, end, proxyStarted) {
  if (!proxyStarted) {
    return { status: "not_captured", reason: "runtime broker was not launched by the harness" };
  }
  if (!start || !end) {
    return { status: "not_captured", reason: "read-only broker metrics were unavailable" };
  }
  if (end.waitTotalNs < start.waitTotalNs || end.waitCount < start.waitCount) {
    return {
      status: "not_captured",
      reason: "broker wait-duration counters reset during the run",
    };
  }
  const waitTotalNs = end.waitTotalNs - start.waitTotalNs;
  const waitCount = end.waitCount - start.waitCount;
  return {
    status: "captured",
    start,
    end,
    delta: {
      waitTotalNs,
      waitCount,
      waitMaxNs: end.waitMaxNs,
      waitMeanNs: waitCount > 0 ? round(waitTotalNs / waitCount) : 0,
    },
  };
}

function brokerWaitDuration(metrics, field) {
  const wait = metrics[field];
  if (
    !wait ||
    ![wait.wait_total_ns, wait.wait_count, wait.wait_max_ns].every(
      (value) => Number.isSafeInteger(value) && value >= 0,
    )
  ) {
    return null;
  }
  return {
    waitTotalNs: wait.wait_total_ns,
    waitCount: wait.wait_count,
    waitMaxNs: wait.wait_max_ns,
  };
}

function brokerAllocationSnapshot(metrics) {
  const allocation = metrics.allocation;
  if (allocation == null) return { status: "unsupported" };

  const snapshot = {};
  for (const [outputField, brokerField] of BROKER_ALLOCATION_FIELDS) {
    const value = allocation[brokerField];
    if (!Number.isSafeInteger(value) || value < 0) {
      return { status: "invalid" };
    }
    snapshot[outputField] = value;
  }
  return { status: "captured", snapshot };
}

function allocationPerRequestEvidence(start, end, proxyStarted, attemptedRequests) {
  if (!proxyStarted) {
    return {
      status: "unsupported",
      reason: "allocation counters require a broker launched by the harness",
    };
  }
  if (!start || !end) {
    return { status: "not_captured", reason: "read-only broker metrics were unavailable" };
  }
  if (start.status === "unsupported" || end.status === "unsupported") {
    return {
      status: "unsupported",
      reason: "the prodex binary was not built with allocation-bench-support",
    };
  }
  if (start.status !== "captured" || end.status !== "captured") {
    return {
      status: "not_captured",
      reason: "broker allocation counters were not non-negative safe integers",
    };
  }
  if (!Number.isSafeInteger(attemptedRequests) || attemptedRequests <= 0) {
    return { status: "not_captured", reason: "no safe positive attempted-request count" };
  }

  const startSnapshot = start.snapshot;
  const endSnapshot = end.snapshot;
  if (
    BROKER_ALLOCATION_CUMULATIVE_FIELDS.some(
      (field) => endSnapshot[field] < startSnapshot[field],
    )
  ) {
    return { status: "not_captured", reason: "broker allocation counters reset during the run" };
  }

  const delta = Object.fromEntries(
    BROKER_ALLOCATION_FIELDS.map(([field]) => [field, endSnapshot[field] - startSnapshot[field]]),
  );
  if (!Object.values(delta).every(Number.isSafeInteger)) {
    return {
      status: "not_captured",
      reason: "broker allocation counter deltas exceeded safe integer precision",
    };
  }
  const allocationOperations = delta.allocCalls + delta.reallocCalls;
  const requestedBytes = delta.allocatedBytes + delta.reallocatedBytes;
  if (![allocationOperations, requestedBytes].every(Number.isSafeInteger)) {
    return {
      status: "not_captured",
      reason: "broker allocation totals exceeded safe integer precision",
    };
  }

  return {
    status: "captured",
    definitions: {
      attemptedRequests: "all requests attempted by the harness, including failed requests",
      allocationOperationsPerRequest:
        "(allocCalls delta + reallocCalls delta) / attemptedRequests",
      requestedBytesPerRequest:
        "(allocatedBytes delta + reallocatedBytes delta) / attemptedRequests",
    },
    attemptedRequests,
    start: startSnapshot,
    end: endSnapshot,
    delta,
    allocationOperationsPerRequest: round(allocationOperations / attemptedRequests, 4),
    requestedBytesPerRequest: round(requestedBytes / attemptedRequests, 4),
  };
}

async function readBrokerPerformanceMetrics(proxy) {
  if (!proxy) return null;
  try {
    const response = await fetch(`${proxy.root}${BROKER_METRICS_PATH}`, {
      headers: { [BROKER_ADMIN_TOKEN_HEADER]: proxy.adminToken },
    });
    if (!response.ok) return null;
    const metrics = await response.json();
    return {
      allocation: brokerAllocationSnapshot(metrics),
      admissionWait: brokerWaitDuration(metrics, "admission_wait"),
      longLivedQueueWait: brokerWaitDuration(metrics, "long_lived_queue_wait"),
      runtimeStateLockWait: brokerWaitDuration(metrics, "runtime_state_lock_wait"),
    };
  } catch {
    return null;
  }
}

function performanceEvidence(proxyStarted, metricsStart, metricsEnd, attemptedRequests) {
  return {
    allocationPerRequest: allocationPerRequestEvidence(
      metricsStart?.allocation,
      metricsEnd?.allocation,
      proxyStarted,
      attemptedRequests,
    ),
    admissionWait: waitDurationEvidence(
      metricsStart?.admissionWait,
      metricsEnd?.admissionWait,
      proxyStarted,
    ),
    longLivedQueueWait: waitDurationEvidence(
      metricsStart?.longLivedQueueWait,
      metricsEnd?.longLivedQueueWait,
      proxyStarted,
    ),
    runtimeStateLockWait: waitDurationEvidence(
      metricsStart?.runtimeStateLockWait,
      metricsEnd?.runtimeStateLockWait,
      proxyStarted,
    ),
  };
}

async function startMock(args) {
  const child = spawn(process.execPath, [
    MOCK_UPSTREAM,
    "--scenario",
    args.scenario,
    "--port",
    "0",
    "--print-ready",
    "--quiet",
  ]);
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => process.stderr.write(chunk));
  const lines = readline.createInterface({ input: child.stdout });
  return new Promise((resolve, reject) => {
    let settled = false;
    const fail = (error) => {
      if (!settled) {
        settled = true;
        clearTimeout(timeout);
        reject(error);
      }
    };
    const timeout = setTimeout(() => {
      fail(new Error("mock upstream did not become ready"));
    }, 10_000);
    child.once("error", fail);
    child.once("exit", (code, signal) => {
      fail(new Error(`mock upstream exited early code=${code} signal=${signal}`));
    });
    lines.on("line", (line) => {
      if (!line.startsWith("mock-upstream-ready ")) {
        return;
      }
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      const ready = JSON.parse(line.slice("mock-upstream-ready ".length));
      resolve({ child, ...ready });
    });
  });
}

async function prepareProdexHome(root, profiles) {
  const profilesRoot = path.join(root, "profiles");
  await mkdir(profilesRoot, { recursive: true, mode: 0o700 });
  await chmod(profilesRoot, 0o700);
  const stateProfiles = {};
  for (let index = 1; index <= profiles; index += 1) {
    const name = `load-${index}`;
    const accountId = `${name}-account`;
    const profileHome = path.join(root, "profiles", name);
    await mkdir(profileHome, { recursive: true, mode: 0o700 });
    await chmod(profileHome, 0o700);
    await writeFile(
      path.join(profileHome, "auth.json"),
      JSON.stringify({ tokens: { access_token: `token-${name}`, account_id: accountId } }),
      { mode: 0o600 },
    );
    stateProfiles[name] = {
      codex_home: profileHome,
      managed: true,
      email: `${name}@example.test`,
      provider: { provider_kind: "openai" },
    };
  }
  await writeFile(
    path.join(root, "state.json"),
    JSON.stringify(
      {
        active_profile: "load-1",
        profiles: stateProfiles,
        last_run_selected_at: {},
        response_profile_bindings: {},
        session_profile_bindings: {},
      },
      null,
      2,
    ),
  );
}

async function waitForHealth(
  url,
  child,
  spawnError,
  healthPath = "/health",
  processName = "prodex broker",
) {
  const deadline = Date.now() + 15_000;
  let lastError = null;
  while (Date.now() < deadline) {
    if (spawnError()) {
      throw spawnError();
    }
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(
        `${processName} exited before ready with code ${child.exitCode} signal ${child.signalCode}`,
      );
    }
    try {
      const response = await fetch(`${url}${healthPath}`);
      if (response.ok) {
        return;
      }
    } catch (error) {
      lastError = error;
    }
    await sleep(100);
  }
  throw new Error(`${processName} health check timed out: ${lastError?.message ?? "no response"}`);
}

async function startProxy(args, upstreamBaseUrl) {
  const root = args.prodexHome
    ? path.resolve(args.prodexHome)
    : await mkdtemp(path.join(os.tmpdir(), "prodex-load-home-"));
  const runtimeLogDir = args.runtimeLogDir
    ? path.resolve(args.runtimeLogDir)
    : await mkdtemp(path.join(os.tmpdir(), "prodex-load-runtime-"));
  await prepareProdexHome(root, args.profiles);
  await mkdir(runtimeLogDir, { recursive: true });
  const listenAddr = args.listenAddr ?? `127.0.0.1:${await freePort()}`;
  const childArgs = ["__runtime-broker"];
  const bootstrap = {
    version: 1,
    current_profile: "load-1",
    upstream_base_url: upstreamBaseUrl,
    include_code_review: false,
    upstream_no_proxy: args.upstreamNoProxy,
    smart_context_enabled: false,
    model_context_window_tokens: null,
    broker_key: "load-harness",
    instance_id: `load-instance-${process.pid}`,
    admin_token: `load-admin-${process.pid}`,
    listen_addr: listenAddr,
  };
  const child = spawn(path.resolve(args.prodex), childArgs, {
    env: {
      ...process.env,
      PRODEX_HOME: root,
      PRODEX_RUNTIME_LOG_DIR: runtimeLogDir,
      PRODEX_RUNTIME_LOG_FORMAT: "text",
      NO_PROXY: ["127.0.0.1", "localhost", "::1", process.env.NO_PROXY ?? ""]
        .filter(Boolean)
        .join(","),
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdin.end(JSON.stringify(bootstrap));
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  let childSpawnError = null;
  child.once("error", (error) => {
    childSpawnError = error;
  });
  child.stdout.on("data", (chunk) => process.stdout.write(chunk));
  child.stderr.on("data", (chunk) => process.stderr.write(chunk));
  const proxyRoot = `http://${listenAddr}`;
  await waitForHealth(proxyRoot, child, () => childSpawnError);
  args.runtimeLogDir = runtimeLogDir;
  return {
    child,
    root: proxyRoot,
    adminToken: bootstrap.admin_token,
    target: `${proxyRoot}/backend-api`,
    prodexHome: root,
    runtimeLogDir,
  };
}

function gatewayBindRace(error, stderr) {
  return /address already in use|failed to bind|os error 98/i.test(
    `${error instanceof Error ? error.message : String(error)}\n${stderr}`,
  );
}

async function startGateway(args, upstreamBaseUrl) {
  const root = await mkdtemp(path.join(os.tmpdir(), "prodex-load-gateway-"));
  const runtimeLogDir = path.join(root, "runtime-logs");
  try {
    await mkdir(runtimeLogDir, { recursive: true, mode: 0o700 });
    await writeFile(path.join(root, "policy.toml"), gatewayPolicy(upstreamBaseUrl), {
      mode: 0o600,
    });
    for (let attempt = 0; attempt < 2; attempt += 1) {
      const listenAddr = `127.0.0.1:${await freePort()}`;
      const child = spawn(path.resolve(args.gatewayBinary), ["serve", "--listen", listenAddr], {
        env: {
          PRODEX_HOME: root,
          PRODEX_RUNTIME_LOG_DIR: runtimeLogDir,
          PRODEX_RUNTIME_LOG_FORMAT: "text",
          NO_PROXY: "127.0.0.1,localhost,::1",
        },
        stdio: ["ignore", "pipe", "pipe"],
      });
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      let childSpawnError = null;
      let stderr = "";
      child.once("error", (error) => {
        childSpawnError = error;
      });
      child.stdout.on("data", (chunk) => process.stdout.write(chunk));
      child.stderr.on("data", (chunk) => {
        stderr = `${stderr}${chunk}`.slice(-MAX_BODY_CAPTURE_BYTES);
        process.stderr.write(chunk);
      });
      const gatewayRoot = `http://${listenAddr}`;
      try {
        await waitForHealth(
          gatewayRoot,
          child,
          () => childSpawnError,
          "/readyz",
          "prodex gateway",
        );
        args.runtimeLogDir = runtimeLogDir;
        return {
          child,
          target: `${gatewayRoot}/v1`,
          cleanupRoot: root,
          runtimeLogDir,
        };
      } catch (error) {
        await stopChild(child);
        if (attempt === 0 && gatewayBindRace(error, stderr)) {
          continue;
        }
        throw error;
      }
    }
    throw new Error("prodex gateway could not reserve a loopback listen address");
  } catch (error) {
    await rm(root, { recursive: true, force: true });
    throw error;
  }
}

async function stopChild(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const exited = new Promise((resolve) => child.once("exit", () => resolve(true)));
  child.kill("SIGTERM");
  if (!(await Promise.race([exited, sleep(2_000).then(() => false)]))) {
    child.kill("SIGKILL");
    await Promise.race([exited, sleep(2_000)]);
  }
}

function printSummary(result) {
  const { summary, thresholds, failures, scenario, stages, target, runtimeLogDir, evidence } =
    result;
  process.stdout.write(
    [
      `scenario=${scenario} target=${target}`,
      `requests=${summary.total} ok=${summary.ok} failures=${summary.failures} error_rate=${round(summary.errorRate, 4)}`,
      `ttft_ms p50=${summary.ttftMs.p50} p95=${summary.ttftMs.p95} p99=${summary.ttftMs.p99} max=${summary.ttftMs.max}`,
      `latency_ms p50=${summary.latencyMs.p50} p95=${summary.latencyMs.p95} p99=${summary.latencyMs.p99} max=${summary.latencyMs.max}`,
      `admission_pressure responses=${summary.admissionPressure.responses} markers=${summary.admissionPressure.markers} rate=${round(summary.admissionPressure.rate, 4)}`,
      `by_status=${JSON.stringify(summary.byStatus)} by_route=${JSON.stringify(summary.byRoute)}`,
      `performance_evidence=${JSON.stringify(evidence)}`,
      `thresholds=${JSON.stringify(thresholds)}`,
      `stages=${stages.map((stage) => `${stage.name}:${stage.results.length}`).join(",")}`,
      runtimeLogDir ? `runtime_log_dir=${runtimeLogDir}` : null,
      failures.length > 0 ? `FAILED ${failures.join("; ")}` : "PASSED",
    ]
      .filter(Boolean)
      .join("\n") + "\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node tests/load/runtime-proxy-load.mjs [--scenario NAME] [--target http://127.0.0.1:9901/backend-api]",
        "",
        "Useful local modes:",
        "  --start-mock                         Start mock upstream and load it directly.",
        "  --start-mock --start-proxy           Start mock upstream, temp Prodex home, and hidden runtime broker.",
        "  --start-mock --gateway-binary PATH   Start mock upstream and a temp dedicated gateway.",
        "  --requests N --concurrency N         Override scenario stages.",
        "  --client-read-delay-ms N             Delay each response-body read after the first chunk.",
        "  --request-timeout-ms N               Bound each request independently.",
        "  --max-error-rate N --max-ttft-p95-ms N --max-admission-pressure-rate N",
        "  --dry-run                             Validate and print a scenario without opening sockets.",
        "  --self-test                           Validate all checked-in scenarios and bounds.",
      ].join("\n") + "\n",
    );
    return;
  }

  validateLaunchMode(args);
  const scenarios = await loadScenarios();
  if (args.selfTest) {
    selfTestScenarios(scenarios);
    process.stdout.write("runtime-proxy-load self-test: ok\n");
    return;
  }
  const scenario = scenarios[args.scenario];
  if (!scenario) {
    throw new Error(`unknown scenario: ${args.scenario}`);
  }
  if (args.dryRun) {
    process.stdout.write(
      `${JSON.stringify(
        {
          scenario: args.scenario,
          stages: stagePlan(scenario, args),
          client: clientConfig(scenario, args),
          mock: scenario.mock ?? {},
          thresholds: thresholdsFor(scenario, args),
        },
        null,
        2,
      )}\n`,
    );
    return;
  }

  let mock = null;
  let proxy = null;
  let gateway = null;
  try {
    if (args.startMock) {
      mock = await startMock(args);
    }
    const upstreamBaseUrl = mock?.baseUrl;
    if (args.startProxy) {
      if (!upstreamBaseUrl && !args.target) {
        throw new Error("--start-proxy requires --start-mock or --target upstream base URL");
      }
      proxy = await startProxy(args, upstreamBaseUrl ?? args.target);
    }
    if (args.gatewayBinary) {
      if (!upstreamBaseUrl && !args.target) {
        throw new Error("--gateway-binary requires --start-mock or --target upstream base URL");
      }
      gateway = await startGateway(
        args,
        upstreamBaseUrl ? `${upstreamBaseUrl}/codex` : args.target,
      );
    }
    const target = normalizeBaseUrl(
      gateway?.target ?? proxy?.target ?? args.target ?? mock?.baseUrl ?? "",
    );
    if (!target) {
      throw new Error("target required: pass --target or --start-mock");
    }
    const metricsStart = await readBrokerPerformanceMetrics(proxy);

    const stages = [];
    const state = { sequence: 0 };
    for (const stage of stagePlan(scenario, args)) {
      process.stdout.write(
        `stage=${stage.name ?? "stage"} concurrency=${stage.concurrency ?? 1} requests=${stage.requests ?? "-"} duration_sec=${stage.durationSec ?? "-"}\n`,
      );
      stages.push(await runStage(stage, scenario, args, target, state));
    }
    const results = stages.flatMap((stage) => stage.results);
    const metricsEnd = await readBrokerPerformanceMetrics(proxy);
    const pressureMarkers = await scanPressureMarkers(args);
    const summary = summarize(results, pressureMarkers);
    const thresholds = thresholdsFor(scenario, args);
    const failures = thresholdFailures(summary, thresholds);
    const output = {
      scenario: args.scenario,
      target,
      runtimeLogDir: gateway?.runtimeLogDir ?? proxy?.runtimeLogDir ?? args.runtimeLogDir,
      stages,
      summary,
      evidence: performanceEvidence(Boolean(proxy), metricsStart, metricsEnd, summary.total),
      thresholds,
      failures,
    };
    if (args.json) {
      process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
    } else {
      printSummary(output);
    }
    if (failures.length > 0) {
      process.exitCode = 1;
    }
  } finally {
    await stopChild(gateway?.child);
    await stopChild(proxy?.child);
    await stopChild(mock?.child);
    if (gateway?.cleanupRoot) {
      await rm(gateway.cleanupRoot, { recursive: true, force: true });
    }
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`runtime-proxy-load: ${message}\n`);
  process.exitCode = 1;
}
