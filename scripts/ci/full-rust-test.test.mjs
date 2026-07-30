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

test("scheduled full suite runs workspace and prodex-app partitions in parallel", () => {
  const workflow = readFileSync(".github/workflows/full-test.yml", "utf8");

  assert.match(workflow, /name: Full tests \(\$\{\{ matrix\.label \}\}\)/);
  assert.match(workflow, /- suite: workspace/);
  assert.match(workflow, /- suite: prodex-app-lib/);
  assert.equal(workflow.match(/save_cache: true/g)?.length, 1);
  assert.equal(workflow.match(/save_cache: false/g)?.length, 1);
  assert.match(workflow, /--timings-json \\\n\s+--no-prodex-app-lib/);
  assert.match(
    workflow,
    /cargo test --locked -q -p prodex-app --lib --all-features -- \\\n\s+--test-threads=1/,
  );
});
