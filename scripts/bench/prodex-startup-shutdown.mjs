#!/usr/bin/env node

import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";

const argv = process.argv.slice(2);
const value = (name, fallback) => {
  const index = argv.indexOf(name);
  return index < 0 ? fallback : argv[index + 1];
};
const before = value("--before", "/home/doxa/.local/bin/prodex");
const after = value("--after", "./target/debug/prodex");
const mode = value("--mode", "run");
const samples = Number(value("--samples", "5"));
const outputPath = value("--output", null);
const counts = value("--sessions", "0,10,100,1000,5000")
  .split(",")
  .map(Number);
const jsonlBytes = Number(value("--bytes", "128"));
if (!new Set(["run", "super"]).has(mode)) throw new Error("--mode expects run or super");

const fakeCodex = `#!/usr/bin/env node
import { appendFileSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
const log = process.env.PRODEX_BENCH_LOG;
const record = (event) => log && appendFileSync(log, event + "|" + Number(process.hrtime.bigint()) + "\\n");
const args = process.argv.slice(2);
record(args[0] === "app-server" ? "app-server" : "interactive");
if (args.includes("--version")) process.exit(0);
if (args[0] !== "app-server") { record("ui"); record("child-exit"); process.exit(0); }
const pages = Number(process.env.PRODEX_BENCH_PAGES || 1), shared = process.env.PRODEX_BENCH_SHARED;
const scan = (root) => { record("directory-walk:" + root.split("/").at(-4)); for (const entry of readdirSync(root, { withFileTypes: true })) { const path = join(root, entry.name); if (entry.isDirectory()) scan(path); else if (entry.name.endsWith(".jsonl")) readFileSync(path); } };
let input = "";
const handle = (line) => { if (!line.trim()) return; const request = JSON.parse(line); if (request.method === "initialize") { process.stdout.write(JSON.stringify({ id: request.id, result: {} }) + "\\n"); return; } if (request.method !== "thread/list") return; const archived = request.params?.archived ? "archived_sessions" : "sessions"; const page = Number(request.params?.cursor || 0); if (!page) scan(join(shared, archived)); record("thread-list:" + archived); process.stdout.write(JSON.stringify({ id: request.id, result: { data: [], nextCursor: page + 1 < pages ? String(page + 1) : null } }) + "\\n"); };
process.stdin.setEncoding("utf8"); process.stdin.on("data", (chunk) => { input += chunk; const lines = input.split(/\\r?\\n/); input = lines.pop(); for (const line of lines) handle(line); });
`;

const percentile = (values, p) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)];
};
const nowNs = () => Number(process.hrtime.bigint());
const timing = (output, name) => Number(output.match(new RegExp(`stage=${name} duration_ms=([0-9.]+)`))?.[1] || 0);
const count = (output, name) => Number(output.match(new RegExp(`${name}=([0-9]+)`))?.[1] || 0);
const stageTimings = (output) => Object.fromEntries(
  [...output.matchAll(/prodex_runtime_timing stage=([^ ]+) duration_ms=([0-9.]+)/g)]
    .map(([, stage, duration]) => [stage, Number(duration)]),
);

function makeFixture(root, sessions, bytes) {
  const home = join(root, "prodex-home"), shared = join(root, "shared-codex");
  const now = new Date(), recent = join(shared, "sessions", String(now.getUTCFullYear()), String(now.getUTCMonth() + 1).padStart(2, "0"), String(now.getUTCDate()).padStart(2, "0"));
  for (const directory of [home, recent, join(shared, "sessions/2000/01/01"), join(shared, "archived_sessions/2000/01/01")]) mkdirSync(directory, { recursive: true });
  writeFileSync(join(recent, "rollout-recent.jsonl"), `{"timestamp":"${now.toISOString()}"}\n`);
  for (let index = 0; index < sessions; index += 1) {
    const prefix = `{"timestamp":"2000-01-01T00:00:00Z","index":${index},"payload":"`;
    const body = prefix + "x".repeat(Math.max(0, bytes - prefix.length - 3)) + `"}\n`;
    writeFileSync(join(shared, "sessions/2000/01/01", `rollout-${index}.jsonl`), body);
    writeFileSync(join(shared, "archived_sessions/2000/01/01", `rollout-${index}.jsonl`), body);
  }
  return { home, shared };
}

function envFor(fixture, fake, log, pages) {
  return { ...process.env, PRODEX_HOME: fixture.home, PRODEX_SHARED_CODEX_HOME: fixture.shared, PRODEX_CODEX_BIN: fake, PRODEX_BENCH_LOG: log, PRODEX_BENCH_SHARED: fixture.shared, PRODEX_BENCH_PAGES: String(pages), PRODEX_RUNTIME_TIMINGS: "1" };
}

function setup(binary, fixture, fake) {
  const result = spawnSync(binary, ["profile", "add", "bench", "--activate", "--insecure"], { env: envFor(fixture, fake, "", 1), encoding: "utf8", timeout: 20_000 });
  if (result.status !== 0) throw new Error(`profile setup failed: ${result.stderr || result.error}`);
}

function launch(binary, fixture, fake, log, pages) {
  const args = mode === "super"
    ? ["s", "--no-presidio", "--no-sub-agent", "--model", "gpt-5.6-luna", "-c", 'model_provider="openai"', "--no-auto-rotate", "--skip-quota-check", "--no-proxy", "exec", "benchmark"]
    : ["run", "--no-auto-rotate", "--skip-quota-check", "--no-proxy"];
  writeFileSync(log, "");
  const started = performance.now();
  const startedNs = nowNs();
  const result = spawnSync(binary, args, { env: envFor(fixture, fake, log, pages), encoding: "utf8", timeout: 20_000 });
  if (result.status !== 0) throw new Error(`launch failed: ${result.stderr || result.error}`);
  const output = result.stdout + result.stderr, events = readFileSync(log, "utf8");
  const eventTime = (name) => Number(events.match(new RegExp(`^${name}\\|(\\d+)$`, "m"))?.[1] || 0);
  const wall = performance.now() - started;
  return { wall, startup: timing(output, "startup\\.total_ms") || wall, shutdown: timing(output, "shutdown\\.post_child_ms"), ui: (eventTime("ui") - startedNs) / 1e6, childInternalUi: (eventTime("ui") - eventTime("interactive")) / 1e6, externalShutdown: (nowNs() - eventTime("child-exit")) / 1e6, files: count(output, "session_files_opened"), bytes: count(output, "session_bytes_read"), sessionsWalked: count(output, "sessions_walked"), archivedSessionsWalked: count(output, "archived_sessions_walked"), appServers: (events.match(/^app-server\|/gm) || []).length, threadLists: (events.match(/^thread-list:/gm) || []).length, directoryWalks: (events.match(/^directory-walk:/gm) || []).length, stages: stageTimings(output) };
}

const root = mkdtempSync(join(tmpdir(), "prodex-startup-shutdown-")), fake = join(root, "codex.mjs");
writeFileSync(fake, fakeCodex); chmodSync(fake, 0o700);
const records = [];
try {
  for (const sessions of counts) for (const [variant, binary] of [["before", before], ["after", after]]) {
    const fixture = makeFixture(join(root, `${variant}-${sessions}`), sessions, jsonlBytes); setup(binary, fixture, fake);
    const log = join(fixture.home, "events.log");
    for (let sample = 0; sample < samples; sample += 1) records.push({ variant, sessions, sample, ...launch(binary, fixture, fake, log, Math.max(1, Math.ceil(sessions / 100))) });
  }
} finally { rmSync(root, { recursive: true, force: true }); }

const summary = counts.flatMap((sessions) => ["before", "after"].map((variant) => {
  const rows = records.filter((row) => row.sessions === sessions && row.variant === variant);
  const p50 = (key) => percentile(rows.map((row) => row[key]), 0.5), p95 = (key) => percentile(rows.map((row) => row[key]), 0.95);
  const stageNames = [...new Set(rows.flatMap((row) => Object.keys(row.stages)))];
  return { variant, sessions, startup_p50_ms: p50("startup"), startup_p95_ms: p95("startup"), shutdown_p50_ms: p50("externalShutdown"), shutdown_p95_ms: p95("externalShutdown"), ui_p50_ms: p50("ui"), stages_p50_ms: Object.fromEntries(stageNames.map((stage) => [stage, percentile(rows.map((row) => row.stages[stage] || 0), 0.5)])), app_server_processes: rows[0].appServers, thread_list_requests: rows[0].threadLists, directory_walks: rows.map((row) => row.directoryWalks), sessions_walked: rows.map((row) => row.sessionsWalked), archived_sessions_walked: rows.map((row) => row.archivedSessionsWalked), session_files_opened: rows.map((row) => row.files), session_bytes_read: rows.map((row) => row.bytes) };
}));
const report = JSON.stringify({ mode, samples, jsonl_bytes: jsonlBytes, session_counts: counts, summary }, null, 2);
if (outputPath) writeFileSync(outputPath, `${report}\n`);
else console.log(report);
