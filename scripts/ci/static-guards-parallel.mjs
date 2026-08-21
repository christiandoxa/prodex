#!/usr/bin/env node

import { parsePositiveInteger, runStepsParallel } from "./main-internal-test-runner.mjs";

const steps = [
  ["rust-size", "ci:size-guard"],
  ["rust-allow", "ci:allow-guard"],
  ["mojo-no-fallback", "ci:mojo-no-fallback-guard"],
  ["optional-tools", "ci:optional-tools-guard"],
  ["smart-context", "ci:smart-context-guard"],
  ["full-test-shards", "ci:full-test-shards"],
  ["provider-capabilities", "docs:provider-capabilities:check"],
  ["runtime-manifest", "ci:runtime-manifest"],
  ["runtime-hotpath", "ci:runtime-hotpath-guard"],
  ["super-wildcard", "ci:super-wildcard-guard"],
  ["env-mutation", "ci:env-mutation-guard"],
  ["secret-boundary", "ci:secret-boundary-guard"],
  ["supply-chain", "ci:supply-chain-guard"],
  ["crate-boundary", "ci:crate-boundary"],
  ["domain-boundary", "ci:domain-boundary-guard"],
].map(([label, script]) => ({
  label: `static-guard:${label}`,
  command: "npm",
  args: ["run", script],
}));

const jobs = parsePositiveInteger(process.env.PRODEX_STATIC_GUARD_JOBS || "6", "PRODEX_STATIC_GUARD_JOBS");

try {
  await runStepsParallel(steps, {
    jobs,
    timingSummary: { label: "static-guards", limit: steps.length, json: true },
  });
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
