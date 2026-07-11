#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const markerPatterns = [
  /runtime_proxy_[A-Za-z0-9_]+/g,
  /\bprofile_health\b/g,
  /\bprofile_retry_backoff\b/g,
  /\bprofile_transport_backoff\b/g,
  /\bprofile_inflight_saturated\b/g,
  /\bprofile_inflight\b/g,
  /upstream_[A-Za-z0-9_]+/g,
  /\bfirst_upstream_chunk\b/g,
  /\bfirst_local_chunk\b/g,
  /\bstream_read_error\b/g,
  /\bprecommit_budget_exhausted\b/g,
  /state_save_[A-Za-z0-9_]+/g,
];

const failedStepConclusions = new Set(["action_required", "cancelled", "failure", "timed_out"]);

export function parseArgs(argv) {
  const args = {};
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

function runGh(args) {
  const result = spawnSync("gh", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error((result.stderr || result.stdout || "gh command failed").trim());
  }
  return result.stdout;
}

export function collectMarkers(text) {
  const counts = new Map();
  for (const line of text.split(/\r?\n/)) {
    for (const pattern of markerPatterns) {
      for (const match of line.matchAll(pattern)) {
        const marker = match[0];
        counts.set(marker, (counts.get(marker) ?? 0) + 1);
      }
    }
  }
  return [...counts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
}

function ghRunViewArgs(args, runId) {
  const ghArgs = ["run", "view", String(runId), "--json", "jobs"];
  const repo = args.repo ?? process.env.GITHUB_REPOSITORY;
  const attempt = args.attempt ?? process.env.GITHUB_RUN_ATTEMPT;

  if (repo) {
    ghArgs.push("--repo", repo);
  }
  if (attempt) {
    ghArgs.push("--attempt", String(attempt));
  }

  return ghArgs;
}

function ghJobLogArgs(args, runId, jobId) {
  const ghArgs = ["run", "view", String(runId), "--job", String(jobId), "--log-failed"];
  const repo = args.repo ?? process.env.GITHUB_REPOSITORY;
  const attempt = args.attempt ?? process.env.GITHUB_RUN_ATTEMPT;

  if (repo) {
    ghArgs.push("--repo", repo);
  }
  if (attempt) {
    ghArgs.push("--attempt", String(attempt));
  }

  return ghArgs;
}

function jobState(job) {
  const conclusion = typeof job.conclusion === "string" && job.conclusion.length > 0 ? job.conclusion : null;
  const status = typeof job.status === "string" && job.status.length > 0 ? job.status : null;
  if (conclusion && status && status !== "completed") {
    return `${status}/${conclusion}`;
  }
  return conclusion ?? status ?? "unknown";
}

function failedSteps(job) {
  if (!Array.isArray(job.steps)) {
    return [];
  }

  return job.steps
    .filter((step) => failedStepConclusions.has(step.conclusion))
    .map((step, index) => ({
      number: step.number ?? index + 1,
      name: typeof step.name === "string" && step.name.length > 0 ? step.name : `step ${index + 1}`,
      status: typeof step.status === "string" && step.status.length > 0 ? step.status : "unknown",
      conclusion: step.conclusion,
    }));
}

function isFailedOrFailingJob(job) {
  if (typeof job.conclusion === "string" && job.conclusion.length > 0 && job.conclusion !== "success") {
    return true;
  }
  return failedSteps(job).length > 0;
}

export function failedJobsFromPayload(payload) {
  return (payload.jobs ?? []).filter(isFailedOrFailingJob);
}

function printFailedSteps(job) {
  const steps = failedSteps(job);
  if (steps.length === 0) {
    process.stdout.write("  failed steps: none reported by gh JSON\n");
    return;
  }

  process.stdout.write("  failed steps:\n");
  for (const step of steps) {
    const state = step.status === "completed" ? step.conclusion : `${step.status}/${step.conclusion}`;
    process.stdout.write(`  - ${step.number}. ${step.name} (${state})\n`);
  }
}

function printMarkers(markers) {
  if (markers.length === 0) {
    process.stdout.write("  markers: none\n");
    return;
  }

  process.stdout.write("  markers:\n");
  for (const [marker, count] of markers) {
    process.stdout.write(`  - ${marker} x${count}\n`);
  }
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/github-failure-summary.mjs [--run-id <id>] [--repo <owner/repo>] [--attempt <n>]",
      "",
      "Summarizes failed GitHub jobs and runtime markers from gh run logs.",
      "For pending runs, falls back to failed step data from gh run view --json jobs.",
    ].join("\n") + "\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const ghCheck = spawnSync("gh", ["--version"], { stdio: "ignore" });
  if (ghCheck.error || ghCheck.status !== 0) {
    process.stderr.write("gh not available on PATH\n");
    process.exitCode = 1;
    return;
  }

  const runId = args.runId ?? process.env.GITHUB_RUN_ID;
  if (!runId) {
    throw new Error("--run-id or GITHUB_RUN_ID is required");
  }

  const raw = runGh(ghRunViewArgs(args, runId));
  const payload = JSON.parse(raw);
  const failedJobs = failedJobsFromPayload(payload);

  if (failedJobs.length === 0) {
    process.stdout.write(`run ${runId}: no failed jobs\n`);
    return;
  }

  process.stdout.write(`run ${runId}: ${failedJobs.length} failed job(s)\n`);
  for (const job of failedJobs) {
    process.stdout.write(`- ${job.name} (${jobState(job)})\n`);
    const jobId = job.databaseId ?? job.id;
    if (!jobId) {
      printFailedSteps(job);
      continue;
    }

    if (job.status && job.status !== "completed") {
      printFailedSteps(job);
      continue;
    }

    try {
      const log = runGh(ghJobLogArgs(args, runId, jobId));
      printMarkers(collectMarkers(log).slice(0, 5));
    } catch (error) {
      process.stdout.write(`  markers: unavailable (${error.message})\n`);
      printFailedSteps(job);
    }
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    process.stderr.write(`github-failure-summary: ${error.message}\n`);
    process.exitCode = 1;
  });
}
