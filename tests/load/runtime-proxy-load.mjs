#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { mkdtemp, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCENARIO_FILE = path.join(__dirname, "scenarios.json");
const MOCK_UPSTREAM = path.join(__dirname, "mock-upstream.mjs");
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
  const booleans = new Set(["start-mock", "start-proxy", "json", "upstream-proxy"]);
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
  ]) {
    if (args[key] !== undefined && args[key] !== null) {
      args[key] = Number(args[key]);
    }
  }
  return args;
}

async function loadScenarios() {
  const raw = await readFile(SCENARIO_FILE, "utf8");
  return JSON.parse(raw).scenarios;
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

function routePath(route) {
  if (route === "compact") {
    return "/codex/responses/compact";
  }
  return "/codex/responses";
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

async function sendRequest(baseUrl, route, id, authToken) {
  const startedAt = performance.now();
  let status = 0;
  let firstByteAt = null;
  let body = "";
  try {
    const response = await fetch(`${baseUrl}${routePath(route)}`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${authToken}`,
        "content-type": "application/json",
        "chatgpt-account-id": "load-client-account",
        "session_id": `load-session-${id % 32}`,
        "x-codex-turn-state": `load-client-turn-${id % 64}`,
        "user-agent": "prodex-load-harness/1",
      },
      body: JSON.stringify(requestPayload(route, id)),
    });
    status = response.status;
    const reader = response.body?.getReader();
    if (!reader) {
      body = await response.text();
      firstByteAt = performance.now();
    } else {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          break;
        }
        if (firstByteAt === null) {
          firstByteAt = performance.now();
        }
        body += new TextDecoder().decode(value);
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
  if (args.requests || args.durationSec || args.concurrency) {
    return [
      {
        name: "override",
        requests: args.requests,
        durationSec: args.durationSec,
        concurrency: args.concurrency ?? 1,
      },
    ];
  }
  return scenario.stages ?? [{ name: args.scenario, requests: 100, concurrency: 8 }];
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

  async function worker() {
    while (issued < maxRequests) {
      if (endsAt && Date.now() >= endsAt) {
        break;
      }
      issued += 1;
      const id = ++state.sequence;
      const route = routeForRequest(scenario, args, id);
      results.push(await sendRequest(baseUrl, route, id, args.authToken));
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
  await mkdir(path.join(root, "profiles"), { recursive: true });
  const stateProfiles = {};
  for (let index = 1; index <= profiles; index += 1) {
    const name = `load-${index}`;
    const accountId = `${name}-account`;
    const profileHome = path.join(root, "profiles", name);
    await mkdir(profileHome, { recursive: true });
    await writeFile(
      path.join(profileHome, "auth.json"),
      JSON.stringify({ tokens: { access_token: `token-${name}`, account_id: accountId } }),
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

async function waitForHealth(url, child, spawnError) {
  const deadline = Date.now() + 15_000;
  let lastError = null;
  while (Date.now() < deadline) {
    if (spawnError()) {
      throw spawnError();
    }
    if (child.exitCode !== null) {
      throw new Error(`prodex broker exited before ready with code ${child.exitCode}`);
    }
    try {
      const response = await fetch(`${url}/health`);
      if (response.ok) {
        return;
      }
    } catch (error) {
      lastError = error;
    }
    await sleep(100);
  }
  throw new Error(`prodex broker health check timed out: ${lastError?.message ?? "no response"}`);
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
  const childArgs = [
    "__runtime-broker",
    "--current-profile",
    "load-1",
    "--upstream-base-url",
    upstreamBaseUrl,
    "--broker-key",
    "load-harness",
    "--instance-token",
    `load-instance-${process.pid}`,
    "--admin-token",
    `load-admin-${process.pid}`,
    "--listen-addr",
    listenAddr,
  ];
  if (args.upstreamNoProxy) {
    childArgs.push("--upstream-no-proxy");
  }
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
    stdio: ["ignore", "pipe", "pipe"],
  });
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
    target: `${proxyRoot}/backend-api`,
    prodexHome: root,
    runtimeLogDir,
  };
}

async function stopChild(child) {
  if (!child || child.exitCode !== null) {
    return;
  }
  child.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    sleep(2_000).then(() => {
      if (child.exitCode === null) {
        child.kill("SIGKILL");
      }
    }),
  ]);
}

function printSummary(result) {
  const { summary, thresholds, failures, scenario, stages, target, runtimeLogDir } = result;
  process.stdout.write(
    [
      `scenario=${scenario} target=${target}`,
      `requests=${summary.total} ok=${summary.ok} failures=${summary.failures} error_rate=${round(summary.errorRate, 4)}`,
      `ttft_ms p50=${summary.ttftMs.p50} p95=${summary.ttftMs.p95} p99=${summary.ttftMs.p99} max=${summary.ttftMs.max}`,
      `latency_ms p50=${summary.latencyMs.p50} p95=${summary.latencyMs.p95} p99=${summary.latencyMs.p99} max=${summary.latencyMs.max}`,
      `admission_pressure responses=${summary.admissionPressure.responses} markers=${summary.admissionPressure.markers} rate=${round(summary.admissionPressure.rate, 4)}`,
      `by_status=${JSON.stringify(summary.byStatus)} by_route=${JSON.stringify(summary.byRoute)}`,
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
        "Usage: node tests/load/runtime-proxy-load.mjs [--scenario baseline|stress|spike|soak] [--target http://127.0.0.1:9901/backend-api]",
        "",
        "Useful local modes:",
        "  --start-mock                         Start mock upstream and load it directly.",
        "  --start-mock --start-proxy           Start mock upstream, temp Prodex home, and hidden runtime broker.",
        "  --requests N --concurrency N         Override scenario stages.",
        "  --max-error-rate N --max-ttft-p95-ms N --max-admission-pressure-rate N",
      ].join("\n") + "\n",
    );
    return;
  }

  const scenarios = await loadScenarios();
  const scenario = scenarios[args.scenario];
  if (!scenario) {
    throw new Error(`unknown scenario: ${args.scenario}`);
  }

  let mock = null;
  let proxy = null;
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
    const target = normalizeBaseUrl(proxy?.target ?? args.target ?? mock?.baseUrl ?? "");
    if (!target) {
      throw new Error("target required: pass --target or --start-mock");
    }

    const stages = [];
    const state = { sequence: 0 };
    for (const stage of stagePlan(scenario, args)) {
      process.stdout.write(
        `stage=${stage.name ?? "stage"} concurrency=${stage.concurrency ?? 1} requests=${stage.requests ?? "-"} duration_sec=${stage.durationSec ?? "-"}\n`,
      );
      stages.push(await runStage(stage, scenario, args, target, state));
    }
    const results = stages.flatMap((stage) => stage.results);
    const pressureMarkers = await scanPressureMarkers(args);
    const summary = summarize(results, pressureMarkers);
    const thresholds = thresholdsFor(scenario, args);
    const failures = thresholdFailures(summary, thresholds);
    const output = {
      scenario: args.scenario,
      target,
      runtimeLogDir: proxy?.runtimeLogDir ?? args.runtimeLogDir,
      stages,
      summary,
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
    await stopChild(proxy?.child);
    await stopChild(mock?.child);
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`runtime-proxy-load: ${message}\n`);
  process.exitCode = 1;
}
