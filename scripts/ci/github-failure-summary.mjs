#!/usr/bin/env node
import { spawnSync } from "node:child_process";

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

function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--run-id") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--run-id requires a value");
      }
      args.runId = argv[index];
      continue;
    }
    if (value === "--repo") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--repo requires a value");
      }
      args.repo = argv[index];
      continue;
    }
    if (value === "--attempt") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--attempt requires a value");
      }
      args.attempt = argv[index];
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

function runGh(args, extraArgs = []) {
  const result = spawnSync("gh", [...extraArgs, ...args], {
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

function collectMarkers(text) {
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

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/github-failure-summary.mjs [--run-id <id>] [--repo <owner/repo>] [--attempt <n>]",
        "",
        "Summarizes failed GitHub jobs and runtime markers from gh run logs.",
      ].join("\n") + "\n",
    );
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

  const repo = args.repo ?? process.env.GITHUB_REPOSITORY;
  const attempt = args.attempt ?? process.env.GITHUB_RUN_ATTEMPT;
  const ghExtraArgs = [
    ...(repo ? ["--repo", repo] : []),
    ...(attempt ? ["--attempt", String(attempt)] : []),
  ];
  const raw = runGh(["run", "view", runId, "--json", "jobs"], ghExtraArgs);
  const payload = JSON.parse(raw);
  const failedJobs = (payload.jobs ?? []).filter((job) => job.conclusion && job.conclusion !== "success");

  if (failedJobs.length === 0) {
    process.stdout.write(`run ${runId}: no failed jobs\n`);
    return;
  }

  process.stdout.write(`run ${runId}: ${failedJobs.length} failed job(s)\n`);
  for (const job of failedJobs) {
    process.stdout.write(`- ${job.name} (${job.conclusion})\n`);
    const jobId = job.databaseId ?? job.id;
    if (!jobId) {
      continue;
    }
    const log = runGh(["run", "view", runId, "--job", String(jobId), "--log-failed"], ghExtraArgs);
    const markers = collectMarkers(log).slice(0, 5);
    if (markers.length === 0) {
      process.stdout.write("  markers: none\n");
      continue;
    }
    process.stdout.write("  markers:\n");
    for (const [marker, count] of markers) {
      process.stdout.write(`  - ${marker} x${count}\n`);
    }
  }
}

await main();
