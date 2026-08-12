import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  ciGithubMatrix,
  githubMatrix,
  PRODEX_APP_LIB_FILTERS,
  PRODEX_APP_LIB_SHARDS,
  validateShards,
  windowsGithubMatrix,
} from "./prodex-app-test-shards.mjs";

function runPlanner(...args) {
  return spawnSync(process.execPath, ["scripts/ci/prodex-app-test-shards.mjs", ...args], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
}

test("prodex-app shard manifest is disjoint and remainder-complete", () => {
  assert.deepEqual(validateShards(), []);
  assert.equal(PRODEX_APP_LIB_SHARDS.length, 13);
  assert.deepEqual(
    PRODEX_APP_LIB_SHARDS.filter((shard) => shard.filters).flatMap((shard) => shard.filters),
    PRODEX_APP_LIB_FILTERS,
  );
  assert.deepEqual(PRODEX_APP_LIB_SHARDS.at(-1).skipFilters, PRODEX_APP_LIB_FILTERS);
  assert.equal(PRODEX_APP_LIB_SHARDS.find((shard) => shard.suite === "admission-core").filters.length, 8);
  assert.equal(PRODEX_APP_LIB_SHARDS.find((shard) => shard.suite === "admission-affinity").filters.length, 6);

  const appMatrix = githubMatrix();
  const ciMatrix = ciGithubMatrix();
  const fullMatrix = githubMatrix({ includeWorkspace: true });
  const windowsMatrix = windowsGithubMatrix();
  assert.equal(appMatrix.include.length, 13);
  assert.equal(ciMatrix.include.length, 9);
  assert.equal(fullMatrix.include.length, 14);
  assert.equal(windowsMatrix.include.length, 5);
  assert.equal(appMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(fullMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(windowsMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(fullMatrix.include[0].suite, "workspace");
  const ciFilters = ciMatrix.include.flatMap((entry) => entry.filters.split("\n")).filter(Boolean);
  assert.deepEqual(
    new Set(ciFilters),
    new Set(PRODEX_APP_LIB_FILTERS.filter((filter) => !filter.startsWith("main_internal_tests::"))),
  );
  assert.deepEqual(
    ciMatrix.include.at(-1).skip_filters.split("\n"),
    [
      ...PRODEX_APP_LIB_FILTERS.filter((filter) => !filter.startsWith("main_internal_tests::")),
      "main_internal_tests::",
      "profile_commands_internal_tests::",
    ],
  );
  for (const entry of ciMatrix.include) {
    const skips = entry.skip_filters.split("\n");
    assert.ok(skips.includes("main_internal_tests::"));
    assert.ok(skips.includes("profile_commands_internal_tests::"));
  }
  const fullFilters = fullMatrix.include.flatMap((entry) => entry.filters.split("\n")).filter(Boolean);
  assert.deepEqual(new Set(fullFilters), new Set(PRODEX_APP_LIB_FILTERS));
  const windowsFilters = windowsMatrix.include
    .flatMap((entry) => entry.filters.split("\n"))
    .filter(Boolean);
  assert.equal(windowsFilters.length, PRODEX_APP_LIB_FILTERS.length);
  assert.deepEqual(new Set(windowsFilters), new Set(PRODEX_APP_LIB_FILTERS));
  assert.deepEqual(
    windowsMatrix.include.at(-1).skip_filters.split("\n"),
    PRODEX_APP_LIB_FILTERS,
  );
});

test("shard planner dry-run and matrix output are compile-free", () => {
  const check = runPlanner("--check");
  assert.equal(check.status, 0, check.stderr);
  assert.match(check.stdout, /13 app shard\(s\), one cache writer/);

  const dryRun = runPlanner("--dry-run");
  assert.equal(dryRun.status, 0, dryRun.stderr);
  assert.match(dryRun.stdout, /dry-run: 14 full-test shard\(s\)/);
  assert.match(dryRun.stdout, /selection: cargo test .*--test-threads=1/);
  assert.match(dryRun.stdout, /remainder: cargo test .*--skip/);

  const matrixRun = runPlanner("--github-matrix");
  assert.equal(matrixRun.status, 0, matrixRun.stderr);
  assert.deepEqual(JSON.parse(matrixRun.stdout), ciGithubMatrix());

  const windowsMatrixRun = runPlanner("--windows-github-matrix");
  assert.equal(windowsMatrixRun.status, 0, windowsMatrixRun.stderr);
  assert.deepEqual(JSON.parse(windowsMatrixRun.stdout), windowsGithubMatrix());

  const fullMatrixRun = runPlanner("--full-test-matrix");
  assert.equal(fullMatrixRun.status, 0, fullMatrixRun.stderr);
  assert.deepEqual(JSON.parse(fullMatrixRun.stdout), githubMatrix({ includeWorkspace: true }));
});

test("CI consumes generated app shards and retains required safety gates", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const fullWorkflow = readFileSync(".github/workflows/full-test.yml", "utf8");

  assert.match(workflow, /prodex_app_matrix: \$\{\{ steps\.prodex-app-matrix\.outputs\.matrix \}\}/);
  assert.match(
    workflow,
    /windows_prodex_app_matrix: \$\{\{ steps\.prodex-app-matrix\.outputs\.windows_matrix \}\}/,
  );
  assert.match(workflow, /matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.prodex_app_matrix\) \}\}/);
  assert.match(
    workflow,
    /matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.windows_prodex_app_matrix\) \}\}/,
  );
  assert.match(workflow, /PRODEX_APP_SKIP_FILTERS/);
  assert.match(workflow, /PRODEX_APP_FILTERS/);
  assert.match(workflow, /--test-threads=1/);
  assert.match(workflow, /save-if: \$\{\{ matrix\.save_cache \}\}/);
  assert.match(workflow, /prodex-app shard matched no tests/);
  assert.match(workflow, /"\$\{skip_args\[@\]\}"/);
  assert.match(workflow, /^  profile-commands-internal:/m);
  assert.match(workflow, /^  main-internal-core:/m);
  assert.match(workflow, /^  main-internal-runtime-proxy:/m);
  assert.match(fullWorkflow, /full_test_shards:/);
  assert.match(fullWorkflow, /--full-test-matrix/);
  assert.match(fullWorkflow, /fromJSON\(needs\.full_test_shards\.outputs\.matrix\)/);
  assert.match(fullWorkflow, /prodex-app shard matched no tests/);
  assert.match(fullWorkflow, /- name: Install Node\.js\n        if: matrix\.suite == 'workspace'/);
  assert.match(fullWorkflow, /^  CARGO_PROFILE_DEV_DEBUG: "0"$/m);
  assert.match(fullWorkflow, /^  CARGO_PROFILE_TEST_DEBUG: "0"$/m);

  const macosWorkspace = workflow.match(/\n  macos-workspace:\n([\s\S]*?)\n  process-guard:/)?.[1];
  assert.ok(macosWorkspace, "macOS workspace job missing");
  assert.match(macosWorkspace, /CARGO_PROFILE_DEV_DEBUG: "0"/);
  assert.match(macosWorkspace, /CARGO_PROFILE_TEST_DEBUG: "0"/);

  for (const job of [
    "docs-lint",
    "secret-scan",
    "windows-workspace",
    "windows-prodex-app",
    "redis-integration",
    "backup-restore-drill",
    "smart-context-evidence",
    "process-guard",
  ]) {
    assert.match(workflow, new RegExp(`^  ${job}:`, "m"), `${job} job missing`);
  }
  assert.doesNotMatch(workflow, /^  windows-security:/m);
  assert.ok(PRODEX_APP_LIB_FILTERS.includes("app_commands::"));
  assert.ok(PRODEX_APP_LIB_FILTERS.includes("runtime_broker::"));
  const processGuard = workflow.match(/\n  process-guard:\n([\s\S]*?)\n  redis-integration:/)?.[1];
  assert.ok(processGuard, "process-guard job missing");
  assert.match(processGuard, /RUSTC_WRAPPER: sccache/);
  assert.match(processGuard, /mozilla-actions\/sccache-action@/);
  assert.match(processGuard, /Swatinem\/rust-cache@/);
  assert.doesNotMatch(processGuard, /npm run docs:smart-context-evidence:check/);
  assert.equal(
    processGuard.match(/if: matrix\.lane == 'static' \|\| matrix\.lane == 'enterprise-storage'/g)
      ?.length,
    3,
  );
  const smartContextEvidence = workflow.match(
    /\n  smart-context-evidence:\n([\s\S]*?)\n  process-guard:/,
  )?.[1];
  assert.ok(smartContextEvidence, "smart-context-evidence job missing");
  assert.match(smartContextEvidence, /actions\/setup-node@/);
  assert.match(smartContextEvidence, /dtolnay\/rust-toolchain@/);
  assert.match(smartContextEvidence, /mozilla-actions\/sccache-action@/);
  assert.match(smartContextEvidence, /Swatinem\/rust-cache@/);
  assert.match(smartContextEvidence, /npm run docs:smart-context-evidence:check/);
  const telemetry = workflow.match(/\n  ci-duration-telemetry:\n([\s\S]*)$/)?.[1];
  assert.match(telemetry, /- smart-context-evidence/);
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
