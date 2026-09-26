#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const crateBoundaryGuard = path.join(repoRoot, "scripts/ci/crate-boundary-guard.mjs");

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const result = spawnSync(process.execPath, [crateBoundaryGuard, "--self-test"], { stdio: "inherit" });
  if (result.error) throw result.error;
  assertSelfTest(result.status === 0, "crate dependency-direction self-test failed");
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const result = spawnSync(process.execPath, [crateBoundaryGuard, ...process.argv.slice(2)], { stdio: "inherit" });
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`application-boundary-guard: ${message}\n`);
  process.exitCode = 1;
});
