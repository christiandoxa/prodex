#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { formatDurationMs } from "./main-internal-test-runner.mjs";

const DEFAULT_LIMIT = 10;

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function parsePositiveInteger(value, name) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

export function parseArgs(argv) {
  const args = {
    files: [],
    json: false,
    limit: DEFAULT_LIMIT,
    runtimeStressHints: false,
    weightsJson: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--file" || value === "-f") {
      index += 1;
      args.files.push(requiredValue(argv[index], value));
      continue;
    }
    if (value === "--limit") {
      index += 1;
      args.limit = parsePositiveInteger(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--weights-json") {
      args.weightsJson = true;
      continue;
    }
    if (value === "--runtime-stress-hints") {
      args.runtimeStressHints = true;
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
      "Usage: node scripts/ci/timing-auto-balance.mjs [--file <path> ...] [--limit <n>] [--json|--weights-json|--runtime-stress-hints]",
      "",
      "Reads CI runner timing telemetry from stdin and/or files.",
      "",
      "Accepted inputs:",
      "  - lines containing '<label>: timings-json [...]' from test-fast/test-serial",
      "  - raw JSON timing arrays such as [{\"label\":\"serial:stress:...\",\"elapsedMs\":1200}]",
      "",
      "Outputs:",
      "  default                 human slowest-label summary plus runtime stress suggestions when possible",
      "  --json                  full machine-readable summary",
      "  --weights-json          generic [{label, weightSeconds, ...}] rebalance input",
      "  --runtime-stress-hints  RUNTIME_STRESS_WEIGHT_HINTS snippet for runtime stress labels",
    ].join("\n") + "\n",
  );
}

async function readStdin() {
  if (process.stdin.isTTY) {
    return "";
  }

  process.stdin.setEncoding("utf8");
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
  }
  return input;
}

async function readInputs(args) {
  const chunks = args.files.map((path) => readFileSync(path, "utf8"));
  const stdin = await readStdin();
  if (stdin.trim()) {
    chunks.push(stdin);
  }
  return chunks.join("\n");
}

function parseTimingRecord(value, source) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${source}: timing entry must be an object`);
  }
  if (typeof value.label !== "string" || value.label.length === 0) {
    throw new Error(`${source}: timing entry needs a non-empty label`);
  }
  if (typeof value.elapsedMs !== "number" || !Number.isFinite(value.elapsedMs) || value.elapsedMs < 0) {
    throw new Error(`${source}: timing entry ${value.label} needs non-negative elapsedMs`);
  }
  const attempts = value.attempts === undefined ? 1 : value.attempts;
  if (!Number.isInteger(attempts) || attempts < 1) {
    throw new Error(`${source}: timing entry ${value.label} needs positive integer attempts`);
  }
  return {
    attempts,
    elapsedMs: value.elapsedMs,
    label: value.label,
  };
}

function parseTimingArray(value, source) {
  if (!Array.isArray(value)) {
    throw new Error(`${source}: expected JSON timing array`);
  }
  return value.map((entry, index) => parseTimingRecord(entry, `${source}[${index}]`));
}

function tryParseJsonArray(text, source) {
  const parsed = JSON.parse(text);
  return parseTimingArray(parsed, source);
}

export function parseTimingTelemetry(input) {
  const records = [];
  const rawJsonChunks = [];
  const lines = input.split(/\r?\n/);

  for (const [lineIndex, line] of lines.entries()) {
    const markerIndex = line.indexOf("timings-json ");
    if (markerIndex >= 0) {
      const jsonText = line.slice(markerIndex + "timings-json ".length).trim();
      records.push(...tryParseJsonArray(jsonText, `line ${lineIndex + 1}`));
      continue;
    }
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      rawJsonChunks.push({ text: trimmed, source: `line ${lineIndex + 1}` });
    }
  }

  if (records.length === 0 && rawJsonChunks.length === 0) {
    const trimmed = input.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      records.push(...tryParseJsonArray(trimmed, "stdin"));
    }
  }

  for (const chunk of rawJsonChunks) {
    records.push(...tryParseJsonArray(chunk.text, chunk.source));
  }

  return records;
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) {
    return sorted[middle];
  }
  return (sorted[middle - 1] + sorted[middle]) / 2;
}

function percentile(values, percentileValue) {
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil((percentileValue / 100) * sorted.length) - 1));
  return sorted[index];
}

export function summarizeTimings(records) {
  const byLabel = new Map();
  for (const record of records) {
    const current = byLabel.get(record.label) ?? {
      attempts: 0,
      elapsed: [],
      label: record.label,
      runs: 0,
      totalMs: 0,
    };
    current.attempts += record.attempts;
    current.elapsed.push(record.elapsedMs);
    current.runs += 1;
    current.totalMs += record.elapsedMs;
    byLabel.set(record.label, current);
  }

  const labels = [...byLabel.values()].map((entry) => ({
    attempts: entry.attempts,
    averageMs: Math.round(entry.totalMs / entry.runs),
    label: entry.label,
    maxMs: Math.max(...entry.elapsed),
    medianMs: Math.round(median(entry.elapsed)),
    minMs: Math.min(...entry.elapsed),
    p90Ms: percentile(entry.elapsed, 90),
    runs: entry.runs,
    totalMs: entry.totalMs,
    weightSeconds: Math.max(1, Math.round(entry.totalMs / entry.runs / 1000)),
  }));
  labels.sort((left, right) => right.averageMs - left.averageMs || right.maxMs - left.maxMs || left.label.localeCompare(right.label));

  return {
    labels,
    recordCount: records.length,
    totalMs: records.reduce((total, record) => total + record.elapsedMs, 0),
    uniqueLabels: labels.length,
  };
}

function runtimeStressSelector(label) {
  const stressMatch = label.match(/^serial:stress:(main_internal_tests::.+)$/);
  if (stressMatch) {
    return { name: stressMatch[1].split("::").at(-1), sourceLabel: label, testName: stressMatch[1] };
  }

  const continuationMatch = label.match(/^serial:continuation:\d+:(main_internal_tests::.+)$/);
  if (continuationMatch) {
    return { name: continuationMatch[1].split("::").at(-1), sourceLabel: label, testName: continuationMatch[1] };
  }

  if (label.startsWith("main_internal_tests::runtime_proxy_")) {
    return { name: label.split("::").at(-1), sourceLabel: label, testName: label };
  }

  return null;
}

export function suggestedWeights(summary) {
  return summary.labels.map((entry) => ({
    averageMs: entry.averageMs,
    label: entry.label,
    maxMs: entry.maxMs,
    runs: entry.runs,
    weightSeconds: entry.weightSeconds,
  }));
}

export function suggestedRuntimeStressHints(summary) {
  const byName = new Map();
  for (const entry of summary.labels) {
    const selector = runtimeStressSelector(entry.label);
    if (!selector) {
      continue;
    }
    const current = byName.get(selector.name);
    const next = {
      ...selector,
      averageMs: entry.averageMs,
      labels: current ? [...current.labels, entry.label] : [entry.label],
      runs: (current?.runs ?? 0) + entry.runs,
      weightSeconds: Math.max(current?.weightSeconds ?? 1, entry.weightSeconds),
    };
    byName.set(selector.name, next);
  }

  return [...byName.values()]
    .sort((left, right) => right.weightSeconds - left.weightSeconds || left.name.localeCompare(right.name))
    .map((entry) => ({
      averageMs: entry.averageMs,
      labels: entry.labels,
      name: entry.name,
      runs: entry.runs,
      testName: entry.testName,
      weightSeconds: entry.weightSeconds,
    }));
}

export function formatRuntimeStressHints(hints) {
  return [
    "export const RUNTIME_STRESS_WEIGHT_HINTS = Object.freeze([",
    ...hints.flatMap((hint) => [
      "  {",
      `    name: ${JSON.stringify(hint.name)},`,
      `    weightSeconds: ${hint.weightSeconds},`,
      "  },",
    ]),
    "]);",
  ].join("\n");
}

function printHuman(summary, args) {
  const slowest = summary.labels.slice(0, args.limit);
  process.stdout.write(
    `timing-auto-balance: ${summary.recordCount} timing record(s), ${summary.uniqueLabels} label(s), summed runtime ${formatDurationMs(summary.totalMs)}\n`,
  );
  process.stdout.write(`slowest labels (${slowest.length}):\n`);
  for (const [index, entry] of slowest.entries()) {
    process.stdout.write(
      `${index + 1}. ${entry.label}: avg ${formatDurationMs(entry.averageMs)}, max ${formatDurationMs(entry.maxMs)}, runs ${entry.runs}, suggested weight ${entry.weightSeconds}s\n`,
    );
  }

  const hints = suggestedRuntimeStressHints(summary);
  if (hints.length > 0) {
    process.stdout.write(`runtime stress hint candidates (${Math.min(args.limit, hints.length)}/${hints.length}):\n`);
    for (const hint of hints.slice(0, args.limit)) {
      process.stdout.write(`- ${hint.name}: ${hint.weightSeconds}s (${hint.runs} run(s), ${hint.testName})\n`);
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const input = await readInputs(args);
  const records = parseTimingTelemetry(input);
  if (records.length === 0) {
    throw new Error("no timing telemetry found on stdin or --file input");
  }

  const summary = summarizeTimings(records);
  const weights = suggestedWeights(summary);
  const runtimeStressHints = suggestedRuntimeStressHints(summary);

  if (args.json) {
    process.stdout.write(`${JSON.stringify({ ...summary, runtimeStressHints, weights }, null, 2)}\n`);
    return;
  }
  if (args.weightsJson) {
    process.stdout.write(`${JSON.stringify(weights, null, 2)}\n`);
    return;
  }
  if (args.runtimeStressHints) {
    process.stdout.write(`${formatRuntimeStressHints(runtimeStressHints)}\n`);
    return;
  }

  printHuman(summary, args);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`timing-auto-balance: ${message}\n`);
    process.exitCode = 1;
  });
}
