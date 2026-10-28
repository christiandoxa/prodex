import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { RUNTIME_TEST_TAGS, RUNTIME_CI_TEST_CASES } from "./runtime-test-manifest.mjs";

const ci = readFileSync(".github/workflows/ci.yml", "utf8");
function job(source, name) {
  const start = source.indexOf(`\n  ${name}:\n`);
  assert.ok(start >= 0, `missing ${name}`);
  const tail = source.slice(start + 1);
  const next = tail.search(/\n  [a-z][a-z0-9_-]*:\n/u);
  return tail.slice(0, next < 0 ? tail.length : next);
}

test("every CI platform archive has a native consumer; Intel macOS stays in release", () => {
  const targets = [...job(ci, "mojo-runtime-platform-archives").matchAll(/- target: ([\w-]+)/gu)].map(m => m[1]);
  assert.deepEqual(targets, ["x86_64-pc-windows-msvc", "aarch64-apple-darwin"]);
  for (const target of targets) assert.match(ci, new RegExp(`name: mojo-runtime-archive-${target}\\n`));
  assert.match(job(ci, "macos-workspace"), /runs-on: macos-14/);
  assert.match(job(ci, "macos-workspace"), /name: mojo-runtime-archive-aarch64-apple-darwin/);
  assert.match(job(ci, "macos-native-regressions"), /name: mojo-runtime-archive-aarch64-apple-darwin/);
  const release = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const productionTargets = [...job(release, "build").matchAll(/- target: ([\w-]+)/gu)].map(m => m[1]);
  assert.equal(productionTargets.length, 6);
  assert.ok(productionTargets.includes("x86_64-apple-darwin"));
  assert.match(readFileSync(".github/workflows/release-target.yml", "utf8"), /Verify macOS Intel artifact architecture and Rosetta execution/);
});

test("Linux installer retains one unconditional Node-suite owner instead of a duplicate runner", () => {
  assert.doesNotMatch(ci, /^  standalone-installer:/m);
  const node = job(ci, "process-guard");
  assert.doesNotMatch(node.split("    steps:")[0], /^    if:/m);
  assert.match(node, /- node\n/);
  assert.match(node, /name: Run all Node.js tests\n\s+if: matrix.lane == 'node'\n\s+run: npm run test:node/);
  const manifest = JSON.parse(readFileSync("package.json", "utf8"));
  assert.ok(manifest.scripts["test:node"].split(" ").includes("scripts/*.test.mjs"));
  assert.ok(existsSync("scripts/install.test.mjs"));
  assert.match(ci, /name: Test Windows installer/);
});

test("retired empty environment lane cannot report a false successful test run", () => {
  assert.doesNotMatch(ci, /^  env-sensitive-parallel-guard:/m);
  assert.ok(!existsSync("scripts/ci/runtime-env-parallel.mjs"));
  assert.ok(!Object.values(RUNTIME_TEST_TAGS).includes("runtime-env:parallel"));
  assert.ok(RUNTIME_CI_TEST_CASES.length > 0);
  assert.match(ci, /name: Run runtime stress/);
  assert.match(readFileSync("scripts/ci/runtime-test-manifest-guard.mjs", "utf8"), /RUNTIME_CI_TEST_CASES/);
  assert.doesNotMatch(readFileSync("docs/testing.md", "utf8"), /runtime-env-parallel\.mjs/);
  assert.doesNotMatch(readFileSync("scripts/ci/test-impact-manifest.json", "utf8"), /runtime-env-parallel\.mjs/);
});


test("fast app and runtime launch lanes do not execute nested launch cases twice", () => {
  const result = spawnSync(process.execPath, ["scripts/ci/test-fast.mjs", "--tests-only", "--no-prebuild", "--dry-run"], {encoding:"utf8"});
  assert.equal(result.status, 0, result.stderr);
  const app = result.stdout.split("\n").find(line => line.trim().startsWith("lib:app-commands:"));
  const launch = result.stdout.split("\n").find(line => line.trim().startsWith("lib:runtime-launch:"));
  assert.match(app, /--lib app_commands:: -- --test-threads=1$/);
  assert.match(launch, /--lib runtime_launch:: -- --test-threads=1 --skip app_commands::$/);
  const cases = ["app_commands::runtime_launch::tests::nested", "app_commands::ping::test", "runtime_launch::proxy_startup::test"];
  for (const name of cases) {
    const before = Number(name.includes("app_commands::")) + Number(name.includes("runtime_launch::"));
    const after = Number(name.includes("app_commands::")) + Number(name.includes("runtime_launch::") && !name.includes("app_commands::"));
    assert.ok(before > 0);
    assert.equal(after, 1, name);
  }
});
