#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { DEFAULT_BASELINE_PATH, requiredValue } from "./upstream-compat-common.mjs";
import { repoRoot } from "../npm/common.mjs";

function parseArgs(argv) {
  const args = {
    artifactDir:
      process.env.PRODEX_UPSTREAM_COMPAT_ARTIFACT_DIR ||
      (process.env.RUNNER_TEMP ? path.join(process.env.RUNNER_TEMP, "upstream-compat") : path.join(repoRoot, "target/upstream-compat")),
    baseline: process.env.PRODEX_UPSTREAM_BASELINE_PATH || DEFAULT_BASELINE_PATH,
    failOnDrift: process.env.PRODEX_UPSTREAM_FAIL_ON_DRIFT ?? "true",
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--artifact-dir") {
      index += 1;
      args.artifactDir = path.resolve(requiredValue(argv[index], value));
      continue;
    }
    if (value === "--baseline") {
      index += 1;
      args.baseline = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--fail-on-drift") {
      index += 1;
      args.failOnDrift = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  args.failOnDrift = String(args.failOnDrift) === "false" ? "false" : "true";
  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/compat/watch-upstream-ci.mjs [--artifact-dir <dir>] [--baseline <path>] [--fail-on-drift true|false]",
      "",
      "Runs the GitHub Actions upstream compatibility watchdog and writes report artifacts.",
      "",
      "Environment defaults:",
      "  PRODEX_UPSTREAM_COMPAT_ARTIFACT_DIR",
      "  PRODEX_UPSTREAM_BASELINE_PATH",
      "  PRODEX_UPSTREAM_FAIL_ON_DRIFT",
    ].join("\n") + "\n",
  );
}

function runNode(scriptPath, args, outputPath) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [scriptPath, ...args], {
      cwd: repoRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const chunks = [];
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      chunks.push(chunk);
    });
    child.stderr.on("data", (chunk) => {
      chunks.push(chunk);
    });
    child.on("error", reject);
    child.on("close", async (code, signal) => {
      const output = chunks.join("");
      await fs.writeFile(outputPath, output);
      resolve({
        code: signal ? 1 : code ?? 0,
        signal,
        output,
      });
    });
  });
}

async function appendStepSummary(summaryPath) {
  const stepSummary = process.env.GITHUB_STEP_SUMMARY;
  if (!stepSummary) {
    return;
  }
  await fs.appendFile(stepSummary, await fs.readFile(summaryPath, "utf8"));
}

function firstLines(contents, limit) {
  return contents.split(/\r?\n/).slice(0, limit).join("\n");
}

async function writeGuardFailureSummary({ baseline, guardText, summaryPath }) {
  const guardOutput = await fs.readFile(guardText, "utf8");
  await fs.writeFile(
    summaryPath,
    [
      "## Upstream compatibility",
      "",
      `- Baseline: \`${baseline}\``,
      "- Offline guard: `failed`",
      "",
      "### Guard output",
      "```",
      firstLines(guardOutput, 120),
      "```",
      "",
    ].join("\n"),
  );
}

async function writeMetadata({ artifactDir, baseline }) {
  await fs.writeFile(
    path.join(artifactDir, "metadata.json"),
    `${JSON.stringify(
      {
        event_name: process.env.GITHUB_EVENT_NAME,
        ref: process.env.GITHUB_REF,
        sha: process.env.GITHUB_SHA,
        run_id: process.env.GITHUB_RUN_ID,
        run_attempt: process.env.GITHUB_RUN_ATTEMPT,
        baseline_path: baseline,
      },
      null,
      2,
    )}\n`,
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const artifactDir = path.resolve(args.artifactDir);
  const report = path.join(artifactDir, "report.json");
  const textReport = path.join(artifactDir, "watchdog.txt");
  const guardReport = path.join(artifactDir, "baseline-guard.json");
  const guardText = path.join(artifactDir, "baseline-guard.txt");
  const summary = path.join(artifactDir, "summary.md");

  await fs.mkdir(artifactDir, { recursive: true });
  await writeMetadata({ artifactDir, baseline: args.baseline });

  const guard = await runNode(
    "scripts/compat/check-upstream-baseline.mjs",
    ["--baseline", args.baseline, "--report", guardReport],
    guardText,
  );

  if (guard.code !== 0) {
    await writeGuardFailureSummary({ baseline: args.baseline, guardText, summaryPath: summary });
    await appendStepSummary(summary);
    process.exitCode = guard.code;
    return;
  }

  const watch = await runNode(
    "scripts/compat/watch-upstream.mjs",
    ["--baseline", args.baseline, "--report", report],
    textReport,
  );

  const summaryResult = await runNode(
    "scripts/compat/upstream-compat-summary.mjs",
    [report, summary, String(watch.code), args.baseline, args.failOnDrift],
    path.join(artifactDir, "summary-render.txt"),
  );
  if (summaryResult.code !== 0) {
    process.stderr.write(summaryResult.output);
    process.exitCode = summaryResult.code;
    return;
  }

  await appendStepSummary(summary);

  if (watch.code !== 0 && args.failOnDrift === "true") {
    process.exitCode = watch.code;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`upstream-compat-ci: ${message}\n`);
  process.exitCode = 1;
}
