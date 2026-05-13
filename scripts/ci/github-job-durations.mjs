#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const DEFAULT_LIMIT = 10;

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
  if (conclusion && status && status !== "completed") {
    return `${status}/${conclusion}`;
  }
  return conclusion ?? status ?? "unknown";
}

function formatIso(date) {
  return date.toISOString().replace(".000Z", "Z");
}

function normalizeJob(job, index, now) {
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

  let runId = args.runId ?? process.env.GITHUB_RUN_ID ?? null;
  let input = await readStdin();
  if (!input) {
    const fetched = runGhView(args);
    input = fetched.input;
    runId = fetched.runId;
  }

  const payload = parsePayload(input);
  const summary = summarize(payload, args);

  if (args.json) {
    process.stdout.write(`${JSON.stringify({ runId, ...summary }, null, 2)}\n`);
    if (budgetExceeded(summary)) {
      process.exitCode = 1;
    }
    return;
  }

  printSummary(summary, runId);
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
