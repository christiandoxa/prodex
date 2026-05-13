#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS,
  RUNTIME_STRESS_SKIP_TESTS,
  RUNTIME_STRESS_WEIGHT_HINTS,
} from "./runtime-test-manifest.mjs";
import {
  runtimeStressWeightSeconds,
  weightedRuntimeStressShards,
} from "./runtime-stress.mjs";

export const DEFAULT_LIMIT = 10;
export const DEFAULT_RUNTIME_STRESS_MANIFEST_PATH = "scripts/ci/runtime-test-manifest.mjs";
const RUNTIME_STRESS_CARGO_LIST_ARGS = [
  "test",
  "-p",
  "prodex-app",
  "--lib",
  "main_internal_tests::runtime_proxy_",
  "--",
  "--list",
];

export function parseArgs(argv) {
  const args = {
    excludeNames: [],
    json: false,
    limit: DEFAULT_LIMIT,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--run-id") {
      index += 1;
      args.runId = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--repo") {
      index += 1;
      args.repo = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--attempt") {
      index += 1;
      args.attempt = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--limit") {
      index += 1;
      args.limit = parsePositiveInt(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--exclude-name") {
      index += 1;
      args.excludeNames.push(requiredValue(argv[index], value));
      continue;
    }
    if (value === "--max-wall-minutes") {
      index += 1;
      args.maxWallMs = parsePositiveMinutes(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--max-runner-minutes") {
      index += 1;
      args.maxRunnerMs = parsePositiveMinutes(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--runtime-stress-calibration") {
      args.runtimeStressCalibration = true;
      continue;
    }
    if (value === "--runtime-stress-test-list") {
      index += 1;
      args.runtimeStressTestList = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--runtime-stress-manifest") {
      index += 1;
      args.runtimeStressManifest = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--write-runtime-stress-hints") {
      args.writeRuntimeStressHints = true;
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

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function parsePositiveInt(value, name) {
  const result = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(result) || result < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return result;
}

function parsePositiveMinutes(value, name) {
  const result = Number(value);
  if (!Number.isFinite(result) || result <= 0) {
    throw new Error(`${name} must be a positive minute value`);
  }
  return Math.round(result * 60 * 1000);
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/github-job-durations.mjs [--run-id <id>] [--repo <owner/repo>] [--attempt <n>] [--limit <n>]",
      "",
      "Summarizes GitHub Actions job durations.",
      "",
      "Inputs:",
      "  stdin              gh run view --json jobs output. Preferred for offline/local diagnostics.",
      "  --run-id <id>      Fetch gh run view <id> --json jobs when stdin is empty.",
      "  --repo <owner/repo> Repository for gh run view. Defaults to GITHUB_REPOSITORY.",
      "  --attempt <n>      Workflow run attempt. Defaults to GITHUB_RUN_ATTEMPT.",
      "  --limit <n>        Number of longest jobs to print. Defaults to 10.",
      "  --exclude-name <n> Exact job name to omit. Repeatable.",
      "  --max-wall-minutes <n>   Fail when run wall time exceeds this budget.",
      "  --max-runner-minutes <n> Fail when summed job runner time exceeds this budget.",
      "  --json             Print machine-readable summary.",
      "  --runtime-stress-calibration",
      "                     Suggest RUNTIME_STRESS_WEIGHT_HINTS from successful runtime-stress weighted broad shard jobs.",
      "  --runtime-stress-test-list <path>",
      "                     Cargo --list output or JSON string array. Avoids running cargo during calibration.",
      "  --runtime-stress-manifest <path>",
      "                     Manifest path to update with --write-runtime-stress-hints.",
      "  --write-runtime-stress-hints",
      "                     Rewrite scripts/ci/runtime-test-manifest.mjs with suggested weights. Requires calibration.",
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
  return input.trim();
}

function runGhView(args) {
  const runId = args.runId ?? process.env.GITHUB_RUN_ID;
  if (!runId) {
    throw new Error("stdin was empty; --run-id or GITHUB_RUN_ID is required to fetch job durations");
  }

  const ghArgs = ["run", "view", String(runId), "--json", "jobs"];
  const repo = args.repo ?? process.env.GITHUB_REPOSITORY;
  const attempt = args.attempt ?? process.env.GITHUB_RUN_ATTEMPT;

  if (repo) {
    ghArgs.push("--repo", repo);
  }
  if (attempt) {
    ghArgs.push("--attempt", String(attempt));
  }

  const result = spawnSync("gh", ghArgs, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error((result.stderr || result.stdout || "gh run view failed").trim());
  }

  return {
    input: result.stdout,
    runId: String(runId),
  };
}

export function parsePayload(input) {
  const payload = JSON.parse(input);
  if (Array.isArray(payload)) {
    return { jobs: payload };
  }
  if (!payload || typeof payload !== "object" || !Array.isArray(payload.jobs)) {
    throw new Error("expected JSON object with jobs array, or a raw jobs array");
  }
  return payload;
}

function parseDate(value) {
  if (typeof value !== "string" || value.length === 0) {
    return null;
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function jobState(job) {
  const conclusion = typeof job.conclusion === "string" && job.conclusion ? job.conclusion : null;
  const status = typeof job.status === "string" && job.status ? job.status : null;
  const state = typeof job.state === "string" && job.state ? job.state : null;
  if (conclusion && status && status !== "completed") {
    return `${status}/${conclusion}`;
  }
  return conclusion ?? status ?? state ?? "unknown";
}

function formatIso(date) {
  return date.toISOString().replace(".000Z", "Z");
}

function normalizeJob(job, index, now) {
  if (typeof job.durationMs === "number" && Number.isFinite(job.durationMs)) {
    const name = typeof job.name === "string" && job.name.length > 0 ? job.name : `job ${index + 1}`;
    return {
      id: job.databaseId ?? job.id ?? null,
      name,
      state: jobState(job),
      startedAt: typeof job.startedAt === "string" ? job.startedAt : null,
      completedAt: typeof job.completedAt === "string" ? job.completedAt : null,
      durationMs: Math.max(0, job.durationMs),
      running: Boolean(job.running),
    };
  }

  const name = typeof job.name === "string" && job.name.length > 0 ? job.name : `job ${index + 1}`;
  const startedAt = parseDate(job.startedAt ?? job.started_at);
  const completedAt = parseDate(job.completedAt ?? job.completed_at);
  const endForDuration = completedAt ?? (startedAt ? now : null);
  const durationMs = startedAt && endForDuration ? Math.max(0, endForDuration.getTime() - startedAt.getTime()) : null;

  return {
    id: job.databaseId ?? job.id ?? null,
    name,
    state: jobState(job),
    startedAt: startedAt ? formatIso(startedAt) : null,
    completedAt: completedAt ? formatIso(completedAt) : null,
    durationMs,
    running: Boolean(startedAt && !completedAt),
  };
}

function stateCounts(jobs) {
  const counts = new Map();
  for (const job of jobs) {
    counts.set(job.state, (counts.get(job.state) ?? 0) + 1);
  }
  return Object.fromEntries([...counts.entries()].sort((left, right) => left[0].localeCompare(right[0])));
}

function budgetCheck(metric, actualMs, limitMs) {
  if (limitMs === undefined) {
    return null;
  }
  if (actualMs === null) {
    return {
      metric,
      status: "unavailable",
      actualMs: null,
      limitMs,
      overMs: null,
    };
  }
  const overMs = actualMs - limitMs;
  return {
    metric,
    status: overMs > 0 ? "exceeded" : "ok",
    actualMs,
    limitMs,
    overMs: Math.max(0, overMs),
  };
}

export function evaluateBudget(summary, args) {
  const checks = [
    budgetCheck("wall", summary.wall ? summary.wall.durationMs : null, args.maxWallMs),
    budgetCheck("runner", summary.runnerMs, args.maxRunnerMs),
  ].filter(Boolean);

  if (checks.length === 0) {
    return null;
  }

  let status = "ok";
  if (checks.some((check) => check.status === "exceeded")) {
    status = "exceeded";
  } else if (checks.some((check) => check.status === "unavailable")) {
    status = "unavailable";
  }

  return {
    status,
    checks,
  };
}

export function budgetExceeded(summary) {
  return summary.budget?.status === "exceeded";
}

export function summarize(payload, args, now = new Date()) {
  const excluded = new Set(args.excludeNames);
  const jobs = payload.jobs
    .map((job, index) => normalizeJob(job, index, now))
    .filter((job) => !excluded.has(job.name));
  const timedJobs = jobs.filter((job) => job.durationMs !== null);
  const startedJobs = timedJobs.filter((job) => job.startedAt);
  const longestJobs = [...timedJobs]
    .sort((left, right) => right.durationMs - left.durationMs || left.name.localeCompare(right.name))
    .slice(0, args.limit);
  const startTimes = startedJobs.map((job) => Date.parse(job.startedAt));
  const endTimes = timedJobs.map((job) => Date.parse(job.completedAt ?? formatIso(now)));
  const wallStartMs = startTimes.length > 0 ? Math.min(...startTimes) : null;
  const wallEndMs = endTimes.length > 0 ? Math.max(...endTimes) : null;
  const wallMs = wallStartMs !== null && wallEndMs !== null ? Math.max(0, wallEndMs - wallStartMs) : null;
  const runnerMs = timedJobs.reduce((total, job) => total + job.durationMs, 0);

  const summary = {
    generatedAt: formatIso(now),
    totalJobs: jobs.length,
    timedJobs: timedJobs.length,
    untimedJobs: jobs.length - timedJobs.length,
    runningJobs: timedJobs.filter((job) => job.running).length,
    jobs,
    wall: wallMs === null
      ? null
      : {
          startedAt: formatIso(new Date(wallStartMs)),
          completedAt: formatIso(new Date(wallEndMs)),
          durationMs: wallMs,
        },
    runnerMs,
    states: stateCounts(jobs),
    longestJobs,
  };

  summary.budget = evaluateBudget(summary, args);
  return summary;
}

export function parseRuntimeStressBroadShardName(name) {
  const match = String(name).match(/\bweighted broad shard\s+(\d+)\s+of\s+(\d+)\b/i);
  if (!match) {
    return null;
  }

  const oneBasedIndex = Number(match[1]);
  const shardCount = Number(match[2]);
  if (
    !Number.isSafeInteger(oneBasedIndex) ||
    !Number.isSafeInteger(shardCount) ||
    oneBasedIndex < 1 ||
    shardCount < 1 ||
    oneBasedIndex > shardCount
  ) {
    return null;
  }

  return {
    index: oneBasedIndex - 1,
    shardCount,
  };
}

function normalizeCalibrationJobs(jobs, now = new Date()) {
  return jobs.map((job, index) => normalizeJob(job, index, now));
}

export function extractRuntimeStressBroadShardTelemetry(jobs, now = new Date()) {
  const normalizedJobs = normalizeCalibrationJobs(jobs, now);
  const matchedJobs = [];
  const successfulByIndex = new Map();
  const shardCounts = new Set();

  for (const job of normalizedJobs) {
    const parsed = parseRuntimeStressBroadShardName(job.name);
    if (!parsed) {
      continue;
    }

    matchedJobs.push(job);
    shardCounts.add(parsed.shardCount);
    if (job.state !== "success" || job.running || job.durationMs === null) {
      continue;
    }

    if (successfulByIndex.has(parsed.index)) {
      throw new Error(`duplicate successful runtime-stress weighted broad shard ${parsed.index + 1}`);
    }

    successfulByIndex.set(parsed.index, {
      index: parsed.index,
      name: job.name,
      durationMs: job.durationMs,
    });
  }

  if (matchedJobs.length === 0) {
    throw new Error("found no runtime-stress weighted broad shard jobs in duration telemetry");
  }
  if (shardCounts.size !== 1) {
    throw new Error(
      `runtime-stress weighted broad shard jobs disagree on shard count: ${[...shardCounts].sort().join(", ")}`,
    );
  }

  const shardCount = [...shardCounts][0];
  const missing = [];
  for (let index = 0; index < shardCount; index += 1) {
    if (!successfulByIndex.has(index)) {
      missing.push(`${index + 1}/${shardCount}`);
    }
  }

  if (missing.length > 0) {
    const nonSuccess = matchedJobs
      .filter((job) => job.state !== "success" || job.running || job.durationMs === null)
      .map((job) => `${job.name}=${job.running ? "running" : job.state}`)
      .sort();
    throw new Error(
      [
        `runtime stress calibration requires successful completed telemetry for all weighted broad shards; missing ${missing.join(", ")}`,
        nonSuccess.length > 0 ? `ignored non-success shard jobs: ${nonSuccess.join(", ")}` : null,
      ]
        .filter(Boolean)
        .join("; "),
    );
  }

  return {
    shardCount,
    shards: [...successfulByIndex.values()].sort((left, right) => left.index - right.index),
  };
}

function testLeafName(testName) {
  return testName.split("::").at(-1);
}

function weightHintMatches(hint, testName) {
  if (typeof hint.name === "string") {
    return testLeafName(testName) === hint.name;
  }
  return typeof hint.filter === "string" && testName.includes(hint.filter);
}

function skippedRuntimeStressTest(testName, skipTests) {
  return skipTests.some((skipName) => testName.includes(skipName));
}

function roundCalibrationValue(value, digits = 3) {
  const multiplier = 10 ** digits;
  return Math.round(value * multiplier) / multiplier;
}

function selectorForHint(hint) {
  if (typeof hint.name === "string") {
    return {
      selectorType: "name",
      selector: hint.name,
    };
  }
  return {
    selectorType: "filter",
    selector: hint.filter,
  };
}

function suggestedHintWeight(currentWeightSeconds, ratio, defaultWeightSeconds) {
  const rounded = Math.round(currentWeightSeconds * ratio);
  return Math.max(defaultWeightSeconds, rounded);
}

export function calibrateRuntimeStressWeightHints({
  jobs,
  testNames,
  weightHints = RUNTIME_STRESS_WEIGHT_HINTS,
  defaultWeightSeconds = RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS,
  skipTests = RUNTIME_STRESS_SKIP_TESTS,
  now = new Date(),
}) {
  if (!Array.isArray(testNames) || testNames.length === 0) {
    throw new Error("runtime stress calibration requires a non-empty runtime test list");
  }

  const telemetry = extractRuntimeStressBroadShardTelemetry(jobs, now);
  const runnableTests = testNames
    .filter((testName) => typeof testName === "string" && testName.length > 0)
    .filter((testName) => !skippedRuntimeStressTest(testName, skipTests));
  if (runnableTests.length === 0) {
    throw new Error("runtime stress calibration matched zero runnable runtime stress tests after skips");
  }

  const shards = weightedRuntimeStressShards(
    runnableTests,
    telemetry.shardCount,
    weightHints,
    defaultWeightSeconds,
  );
  const observedSecondsByShard = new Map(
    telemetry.shards.map((shard) => [shard.index, shard.durationMs / 1000]),
  );
  const totalObservedSeconds = telemetry.shards.reduce((total, shard) => total + shard.durationMs / 1000, 0);
  const totalEstimatedWeightSeconds = shards.reduce((total, shard) => total + shard.weightSeconds, 0);
  if (totalObservedSeconds <= 0 || totalEstimatedWeightSeconds <= 0) {
    throw new Error("runtime stress calibration requires positive observed and estimated shard durations");
  }

  const observedToWeightScale = totalEstimatedWeightSeconds / totalObservedSeconds;
  const ratioByShard = new Map();
  const shardSummaries = shards.map((shard) => {
    const observedSeconds = observedSecondsByShard.get(shard.index);
    if (observedSeconds === undefined) {
      throw new Error(`runtime stress calibration is missing observed duration for shard ${shard.index + 1}`);
    }

    const targetWeightSeconds = observedSeconds * observedToWeightScale;
    const ratio = targetWeightSeconds / shard.weightSeconds;
    ratioByShard.set(shard.index, ratio);
    return {
      estimatedWeightSeconds: roundCalibrationValue(shard.weightSeconds),
      index: shard.index,
      observedSeconds: roundCalibrationValue(observedSeconds),
      ratio: roundCalibrationValue(ratio),
      targetWeightSeconds: roundCalibrationValue(targetWeightSeconds),
      testCount: shard.tests.length,
    };
  });

  const testShardIndex = new Map();
  for (const shard of shards) {
    for (const test of shard.tests) {
      testShardIndex.set(test.testName, shard.index);
    }
  }

  const suggestions = weightHints.map((hint) => {
    const matches = runnableTests.filter((testName) => weightHintMatches(hint, testName));
    const weightedRatio = matches.reduce(
      (accumulator, testName) => {
        const currentWeight = runtimeStressWeightSeconds(testName, weightHints, defaultWeightSeconds);
        const shardRatio = ratioByShard.get(testShardIndex.get(testName)) ?? 1;
        return {
          denominator: accumulator.denominator + currentWeight,
          numerator: accumulator.numerator + currentWeight * shardRatio,
        };
      },
      { denominator: 0, numerator: 0 },
    );
    const ratio = weightedRatio.denominator > 0 ? weightedRatio.numerator / weightedRatio.denominator : 1;
    const suggestedWeightSeconds = suggestedHintWeight(
      hint.weightSeconds,
      ratio,
      defaultWeightSeconds,
    );
    const action =
      suggestedWeightSeconds > hint.weightSeconds
        ? "raise"
        : suggestedWeightSeconds < hint.weightSeconds
          ? "lower"
          : "keep";

    return {
      ...selectorForHint(hint),
      action,
      currentWeightSeconds: hint.weightSeconds,
      matchCount: matches.length,
      ratio: roundCalibrationValue(ratio),
      suggestedWeightSeconds,
    };
  });
  const suggestedHints = weightHints.map((hint, index) => ({
    ...hint,
    weightSeconds: suggestions[index].suggestedWeightSeconds,
  }));

  return {
    changedHints: suggestions.filter((suggestion) => suggestion.action !== "keep").length,
    defaultWeightSeconds,
    runnableTestCount: runnableTests.length,
    shardCount: telemetry.shardCount,
    shards: shardSummaries,
    suggestedHints,
    suggestions,
    totalEstimatedWeightSeconds: roundCalibrationValue(totalEstimatedWeightSeconds),
    totalObservedSeconds: roundCalibrationValue(totalObservedSeconds),
  };
}

export function parseRuntimeStressTestListText(text) {
  const trimmed = text.trim();
  if (!trimmed) {
    return [];
  }

  try {
    const parsed = JSON.parse(trimmed);
    if (Array.isArray(parsed) && parsed.every((value) => typeof value === "string")) {
      return parsed;
    }
  } catch {
    // Fall through to cargo --list text parsing.
  }

  return trimmed
    .split(/\r?\n/)
    .map((line) => line.match(/^\s*(main_internal_tests::.*): test$/)?.[1])
    .filter(Boolean);
}

function loadRuntimeStressTestNames(args) {
  if (args.runtimeStressTestList) {
    return parseRuntimeStressTestListText(readFileSync(args.runtimeStressTestList, "utf8"));
  }

  const result = spawnSync("cargo", RUNTIME_STRESS_CARGO_LIST_ARGS, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error((result.stderr || result.stdout || "cargo test --list failed").trim());
  }
  const testNames = parseRuntimeStressTestListText(result.stdout);
  if (testNames.length === 0) {
    throw new Error("cargo test --list returned zero runtime stress tests");
  }
  return testNames;
}

function formatHintSelectorLine(hint) {
  if (typeof hint.name === "string") {
    return `    name: ${JSON.stringify(hint.name)},`;
  }
  return `    filter: ${JSON.stringify(hint.filter)},`;
}

export function formatRuntimeStressWeightHints(hints) {
  return [
    "export const RUNTIME_STRESS_WEIGHT_HINTS = Object.freeze([",
    ...hints.flatMap((hint) => [
      "  {",
      formatHintSelectorLine(hint),
      `    weightSeconds: ${hint.weightSeconds},`,
      "  },",
    ]),
    "]);",
  ].join("\n");
}

export function replaceRuntimeStressWeightHintsBlock(text, hints) {
  const replacement = formatRuntimeStressWeightHints(hints);
  const pattern =
    /export const RUNTIME_STRESS_WEIGHT_HINTS = Object\.freeze\(\[\n[\s\S]*?\n\]\);/;
  if (!pattern.test(text)) {
    throw new Error("could not locate RUNTIME_STRESS_WEIGHT_HINTS block");
  }
  return text.replace(pattern, replacement);
}

function writeRuntimeStressWeightHints(manifestPath, hints) {
  const current = readFileSync(manifestPath, "utf8");
  const next = replaceRuntimeStressWeightHintsBlock(current, hints);
  if (next !== current) {
    writeFileSync(manifestPath, next);
    return true;
  }
  return false;
}

function assertRuntimeStressCalibrationWritable(calibration) {
  const unmatched = calibration.suggestions
    .filter((suggestion) => suggestion.matchCount === 0)
    .map(formatCalibrationSelector);
  if (unmatched.length > 0) {
    throw new Error(
      `refusing to write runtime stress hints because test list did not match ${unmatched.length} selector(s): ${unmatched.join(", ")}`,
    );
  }
}

function formatCalibrationSelector(suggestion) {
  return `${suggestion.selectorType}:${suggestion.selector}`;
}

function printRuntimeStressCalibration(calibration, writeResult) {
  process.stdout.write("\nruntime stress calibration:\n");
  process.stdout.write(
    `successful weighted broad shards: ${calibration.shardCount}; runnable tests: ${calibration.runnableTestCount}\n`,
  );
  for (const shard of calibration.shards) {
    process.stdout.write(
      `- shard ${shard.index + 1}/${calibration.shardCount}: observed ${formatDuration(shard.observedSeconds * 1000)}, estimated ${shard.estimatedWeightSeconds}s, ratio ${shard.ratio}, tests ${shard.testCount}\n`,
    );
  }

  const changed = calibration.suggestions.filter((suggestion) => suggestion.action !== "keep");
  process.stdout.write(`suggested hint changes: ${changed.length}/${calibration.suggestions.length}\n`);
  for (const suggestion of changed) {
    process.stdout.write(
      `- ${formatCalibrationSelector(suggestion)}: ${suggestion.currentWeightSeconds}s -> ${suggestion.suggestedWeightSeconds}s (${suggestion.action}, matches ${suggestion.matchCount}, ratio ${suggestion.ratio})\n`,
    );
  }
  if (writeResult) {
    process.stdout.write(`updated ${writeResult.path}: ${writeResult.changed ? "changed" : "already current"}\n`);
  }
  process.stdout.write("\nsuggested RUNTIME_STRESS_WEIGHT_HINTS:\n");
  process.stdout.write(`${formatRuntimeStressWeightHints(calibration.suggestedHints)}\n`);
}

export function formatDuration(durationMs) {
  const totalSeconds = Math.round(durationMs / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${seconds}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

function formatStateCounts(states) {
  return Object.entries(states)
    .map(([state, count]) => `${state}=${count}`)
    .join(", ");
}

function formatBudgetCheck(check) {
  if (check.status === "unavailable") {
    return `- ${check.metric}: unavailable (budget ${formatDuration(check.limitMs)})`;
  }
  if (check.status === "exceeded") {
    return `- ${check.metric}: ${formatDuration(check.actualMs)} exceeds budget ${formatDuration(check.limitMs)} by ${formatDuration(check.overMs)}`;
  }
  return `- ${check.metric}: ${formatDuration(check.actualMs)} within budget ${formatDuration(check.limitMs)}`;
}

function printSummary(summary, runId) {
  const label = runId ? `run ${runId}` : "run";
  process.stdout.write(`${label}: ${summary.timedJobs}/${summary.totalJobs} timed job(s)\n`);

  if (summary.wall) {
    process.stdout.write(
      `wall time: ${formatDuration(summary.wall.durationMs)} (${summary.wall.startedAt} -> ${summary.wall.completedAt})\n`,
    );
  } else {
    process.stdout.write("wall time: unavailable\n");
  }

  process.stdout.write(`runner time: ${formatDuration(summary.runnerMs)}\n`);
  process.stdout.write(`states: ${formatStateCounts(summary.states) || "none"}\n`);
  if (summary.untimedJobs > 0 || summary.runningJobs > 0) {
    process.stdout.write(`untimed jobs: ${summary.untimedJobs}; running jobs sampled: ${summary.runningJobs}\n`);
  }
  if (summary.budget) {
    process.stdout.write(`budget: ${summary.budget.status}\n`);
    for (const check of summary.budget.checks) {
      process.stdout.write(`${formatBudgetCheck(check)}\n`);
    }
  }

  process.stdout.write(`longest jobs (${summary.longestJobs.length}):\n`);
  for (const [index, job] of summary.longestJobs.entries()) {
    const suffix = job.running ? ", sampled while running" : "";
    process.stdout.write(`${index + 1}. ${job.name}: ${formatDuration(job.durationMs)} (${job.state}${suffix})\n`);
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  if (args.writeRuntimeStressHints && !args.runtimeStressCalibration) {
    throw new Error("--write-runtime-stress-hints requires --runtime-stress-calibration");
  }

  let runId = args.runId ?? process.env.GITHUB_RUN_ID ?? null;
  let input = await readStdin();
  if (!input) {
    if (args.runtimeStressCalibration) {
      throw new Error("runtime stress calibration requires job duration JSON on stdin; refusing live GitHub fetch");
    }
    const fetched = runGhView(args);
    input = fetched.input;
    runId = fetched.runId;
  }

  const payload = parsePayload(input);
  const summary = summarize(payload, args);
  let runtimeStressCalibration = null;
  let writeResult = null;
  if (args.runtimeStressCalibration) {
    runtimeStressCalibration = calibrateRuntimeStressWeightHints({
      jobs: summary.jobs,
      testNames: loadRuntimeStressTestNames(args),
    });
    if (args.writeRuntimeStressHints) {
      assertRuntimeStressCalibrationWritable(runtimeStressCalibration);
      const path = args.runtimeStressManifest ?? DEFAULT_RUNTIME_STRESS_MANIFEST_PATH;
      writeResult = {
        changed: writeRuntimeStressWeightHints(path, runtimeStressCalibration.suggestedHints),
        path,
      };
    }
  }

  if (args.json) {
    const output = { runId, ...summary };
    if (runtimeStressCalibration) {
      output.runtimeStressCalibration = runtimeStressCalibration;
    }
    process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
    if (budgetExceeded(summary)) {
      process.exitCode = 1;
    }
    return;
  }

  printSummary(summary, runId);
  if (runtimeStressCalibration) {
    printRuntimeStressCalibration(runtimeStressCalibration, writeResult);
  }
  if (budgetExceeded(summary)) {
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    process.stderr.write(`github-job-durations: ${error.message}\n`);
    process.exitCode = 1;
  });
}
