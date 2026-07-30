import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

test("full Rust runner includes the explicitly disabled prodex-app lib target", () => {
  const result = spawnSync(
    process.execPath,
    ["scripts/ci/full-rust-test.mjs", "--dry-run", "--no-prebuild"],
    { cwd: process.cwd(), encoding: "utf8" },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(
    result.stdout,
    /prodex-app:all-lib-tests-serial: cargo test --locked -q -p prodex-app --lib --all-features -- --test-threads=1/,
  );

  const platformResult = spawnSync(
    process.execPath,
    ["scripts/ci/full-rust-test.mjs", "--dry-run", "--no-prodex-app-lib"],
    { cwd: process.cwd(), encoding: "utf8" },
  );
  assert.equal(platformResult.status, 0, platformResult.stderr);
  assert.doesNotMatch(platformResult.stdout, /prodex-app.*lib/);
});

test("full Rust runner locks every direct cargo test command", () => {
  const result = spawnSync(process.execPath, ["scripts/ci/full-rust-test.mjs", "--dry-run"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const cargoTestLines = result.stdout.split("\n").filter((line) => line.includes(": cargo test "));
  assert.ok(cargoTestLines.length > 0);
  assert.ok(cargoTestLines.every((line) => line.includes("cargo test --locked ")));
});

test("scheduled full suite runs disjoint workspace and prodex-app partitions in parallel", () => {
  const workflow = readFileSync(".github/workflows/full-test.yml", "utf8");

  assert.match(workflow, /name: Full tests \(\$\{\{ matrix\.label \}\}\)/);
  assert.match(workflow, /- suite: workspace/);
  assert.match(workflow, /- suite: prodex-app-selection/);
  assert.match(workflow, /- suite: prodex-app-admission/);
  assert.match(workflow, /- suite: prodex-app-rotation/);
  assert.match(workflow, /- suite: prodex-app-remainder/);
  assert.equal(workflow.match(/save_cache: true/g)?.length, 1);
  assert.equal(workflow.match(/save_cache: false/g)?.length, 4);
  assert.match(workflow, /--timings-json \\\n\s+--no-prodex-app-lib/);
  const partitionFilters = ["selection", "pressure", "incidents", "admission", "state", "rotation", "health"];
  for (const filter of partitionFilters) {
    const path = `main_internal_tests::runtime_proxy_selection_and_pressure::${filter}::`;
    assert.equal(workflow.match(new RegExp(`'${path}'`, "g"))?.length, 2, `${path} must run once and be skipped once`);
  }
  assert.match(workflow, /prodex-app-remainder\)[\s\S]*?cargo test --locked -q -p prodex-app --lib --all-features --/);
});

test("runtime proxy matrix is generated before fan-out without a runner barrier", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const changes = workflow.match(/\n  changes:\n([\s\S]*?)\n  fmt:/)?.[1];
  const runtimeProxy = workflow.match(/\n  main-internal-runtime-proxy:\n([\s\S]*?)\n  runtime-proxy-bench-smoke:/)?.[1];

  assert.ok(changes, "changes job missing");
  assert.ok(runtimeProxy, "main-internal-runtime-proxy job missing");
  assert.match(changes, /runtime_proxy_matrix: \$\{\{ steps\.runtime-matrix\.outputs\.matrix \}\}/);
  assert.match(changes, /node scripts\/ci\/runtime-proxy-ci-matrix\.mjs --github-matrix/);
  assert.match(runtimeProxy, /needs: changes/);
  assert.match(runtimeProxy, /fromJSON\(needs\.changes\.outputs\.runtime_proxy_matrix\)/);
  assert.doesNotMatch(workflow, /\n  runtime-proxy-shard-matrix:/);
});

test("runtime proxy timing packs reduce runner pressure without losing filters", () => {
  const result = spawnSync(process.execPath, ["scripts/ci/runtime-proxy-ci-matrix.mjs", "--github-matrix"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const matrix = JSON.parse(result.stdout);
  const filters = matrix.include.flatMap((entry) => entry.filters.split("\n"));
  assert.equal(matrix.include.length, 12);
  assert.equal(filters.length, 36);
  assert.equal(new Set(filters).size, filters.length);
});
