import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  githubMatrix,
  PRODEX_APP_LIB_FILTERS,
  PRODEX_APP_LIB_SHARDS,
  validateShards,
} from "./prodex-app-test-shards.mjs";

function runPlanner(...args) {
  return spawnSync(process.execPath, ["scripts/ci/prodex-app-test-shards.mjs", ...args], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
}

test("prodex-app shard manifest is disjoint and remainder-complete", () => {
  assert.deepEqual(validateShards(), []);
  assert.equal(PRODEX_APP_LIB_SHARDS.length, 12);
  assert.deepEqual(
    PRODEX_APP_LIB_SHARDS.filter((shard) => shard.filters).flatMap((shard) => shard.filters),
    PRODEX_APP_LIB_FILTERS,
  );
  assert.deepEqual(PRODEX_APP_LIB_SHARDS.at(-1).skipFilters, PRODEX_APP_LIB_FILTERS);

  const appMatrix = githubMatrix();
  const fullMatrix = githubMatrix({ includeWorkspace: true });
  assert.equal(appMatrix.include.length, 12);
  assert.equal(fullMatrix.include.length, 13);
  assert.equal(appMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(fullMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(fullMatrix.include[0].suite, "workspace");
});

test("shard planner dry-run and matrix output are compile-free", () => {
  const dryRun = runPlanner("--dry-run");
  assert.equal(dryRun.status, 0, dryRun.stderr);
  assert.match(dryRun.stdout, /dry-run: 13 full-test shard\(s\)/);
  assert.match(dryRun.stdout, /selection: cargo test .*--test-threads=1/);
  assert.match(dryRun.stdout, /remainder: cargo test .*--skip/);

  const matrixRun = runPlanner("--github-matrix");
  assert.equal(matrixRun.status, 0, matrixRun.stderr);
  assert.deepEqual(JSON.parse(matrixRun.stdout), githubMatrix());

  const fullMatrixRun = runPlanner("--full-test-matrix");
  assert.equal(fullMatrixRun.status, 0, fullMatrixRun.stderr);
  assert.deepEqual(JSON.parse(fullMatrixRun.stdout), githubMatrix({ includeWorkspace: true }));
});

test("CI consumes generated app shards and retains required safety gates", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const fullWorkflow = readFileSync(".github/workflows/full-test.yml", "utf8");

  assert.match(workflow, /prodex_app_matrix: \$\{\{ steps\.prodex-app-matrix\.outputs\.matrix \}\}/);
  assert.match(workflow, /matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.prodex_app_matrix\) \}\}/);
  assert.match(workflow, /PRODEX_APP_SKIP_FILTERS/);
  assert.match(workflow, /PRODEX_APP_FILTERS/);
  assert.match(workflow, /--test-threads=1/);
  assert.match(workflow, /prodex-app shard matched no tests/);
  assert.match(fullWorkflow, /full_test_shards:/);
  assert.match(fullWorkflow, /--full-test-matrix/);
  assert.match(fullWorkflow, /fromJSON\(needs\.full_test_shards\.outputs\.matrix\)/);
  assert.match(fullWorkflow, /prodex-app shard matched no tests/);
  assert.match(fullWorkflow, /- name: Install Node\.js\n        if: matrix\.suite == 'workspace'/);

  for (const job of [
    "docs-lint",
    "secret-scan",
    "windows-security",
    "windows-workspace",
    "windows-prodex-app",
    "redis-integration",
    "backup-restore-drill",
    "process-guard",
  ]) {
    assert.match(workflow, new RegExp(`^  ${job}:`, "m"), `${job} job missing`);
  }
  for (const command of [
    "npm run docs:lint",
    "npm run ci:secret-boundary-guard",
    "npm run ci:crate-boundary",
    "npm run ci:deployment-security-guard",
    "npm run ci:backup-restore-drill",
    "npm run ci:storage-postgres-proof",
    "redis_rate_limit_runtime",
  ]) {
    assert.match(workflow, new RegExp(command.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")), `${command} missing`);
  }
});

test("manifest rejects a remainder that drops a targeted filter", () => {
  const broken = PRODEX_APP_LIB_SHARDS.map((shard) =>
    shard.skipFilters ? { ...shard, skipFilters: shard.skipFilters.slice(1) } : shard,
  );
  assert.ok(validateShards(broken).some((issue) => issue.includes("remainder shard")));
});
