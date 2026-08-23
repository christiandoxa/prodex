#!/usr/bin/env node

import { parsePositiveInteger, runStepsParallel } from "./main-internal-test-runner.mjs";

const steps = [
  ["rust-size", "scripts/ci/size-guard.mjs"],
  ["rust-allow", "scripts/ci/allow-attribute-guard.mjs"],
  ["mojo-no-fallback-self-test", "scripts/ci/mojo-no-fallback-guard.mjs", "--self-test"],
  ["mojo-no-fallback", "scripts/ci/mojo-no-fallback-guard.mjs"],
  ["optional-tools", "scripts/ci/optional-tools-guard.mjs", "--self-test"],
  ["smart-context", "scripts/ci/smart-context-guard.mjs", "--self-test"],
  ["full-test-shards", "scripts/ci/prodex-app-test-shards.mjs", "--check"],
  ["provider-subprocess", "--test", "scripts/lib/checked-subprocess.test.mjs"],
  ["provider-capabilities", "scripts/catalog/provider-capability-matrix.mjs"],
  ["runtime-manifest", "scripts/ci/runtime-test-manifest-guard.mjs"],
  ["runtime-hotpath-self-test", "scripts/ci/runtime-hotpath-guard.mjs", "--self-test"],
  ["runtime-hotpath", "scripts/ci/runtime-hotpath-guard.mjs"],
  ["super-wildcard", "scripts/ci/super-wildcard-guard.mjs"],
  ["env-mutation", "scripts/ci/env-mutation-guard.mjs"],
  ["secret-boundary", "scripts/ci/secret-boundary-guard.mjs", "--self-test"],
  ["supply-chain", "scripts/ci/supply-chain-guard.mjs", "--self-test"],
  ["crate-boundary-self-test", "scripts/ci/crate-boundary-guard.mjs", "--self-test"],
  ["crate-boundary", "scripts/ci/crate-boundary-guard.mjs"],
  ["domain-boundary-self-test", "scripts/ci/domain-boundary-guard.mjs", "--self-test"],
  ["domain-boundary", "scripts/ci/domain-boundary-guard.mjs"],
].map(([label, ...args]) => ({
  label: `static-guard:${label}`,
  command: "node",
  args,
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
