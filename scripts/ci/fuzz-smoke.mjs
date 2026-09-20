#!/usr/bin/env node

import { pathToFileURL } from "node:url";
import { runStepsParallel } from "./main-internal-test-runner.mjs";

export const FUZZ_TARGETS = Object.freeze([
  "profile_export_envelope",
]);

export const FUZZ_SMOKE_JOBS = 4;

export function fuzzSmokeSteps() {
  return FUZZ_TARGETS.map((target) => ({
    label: `fuzz:${target}`,
    command: "cargo",
    args: [
      "+nightly-2026-07-11",
      "fuzz",
      "run",
      target,
      "--fuzz-dir",
      "fuzz",
      "--",
      "-max_total_time=10",
      "-timeout=5",
      "-rss_limit_mb=2048",
    ],
  }));
}

async function main() {
  await runStepsParallel(fuzzSmokeSteps(), { jobs: FUZZ_SMOKE_JOBS });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(`fuzz-smoke: ${error.message}\n`);
    process.exitCode = 1;
  }
}
