import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, mkdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { githubMatrix, fullTestAppMatrix } from "./prodex-app-test-shards.mjs";
const workflow = readFileSync(".github/workflows/full-test.yml", "utf8");
function job(name) {
  const block = workflow.split(`\n  ${name}:\n`)[1];
  assert.ok(block, `missing ${name}`);
  return block.split(/\n  [a-z_][a-z0-9_-]*:/u)[0];
}
test("native full-test projection retains every original filter and exclusion exactly", () => {
  const full = githubMatrix({ includeWorkspace: true }).include;
  assert.deepEqual(fullTestAppMatrix().include, full.filter((entry) => entry.suite !== "workspace"));
  assert.equal(fullTestAppMatrix().include.length, 26);
  assert.equal(full.filter((entry) => entry.suite === "workspace").length, 1);
});
test("workspace starts independently while all app shards require the verified producer", () => {
  const producer = job("full_test_shards");
  const workspace = job("full-workspace");
  const consumer = job("full-test");
  assert.match(workspace, /needs: mojo-runtime-linux-archive/);
  assert.doesNotMatch(workspace, /needs:.*full_test_shards/);
  assert.match(producer, /native-test-artifact.mjs pack/);
  assert.match(producer, /manifest_sha256:.*steps.pack.outputs.manifest_sha256/);
  assert.match(consumer, /needs: full_test_shards/);
  assert.match(consumer, /TEST_MANIFEST_SHA256:.*needs.full_test_shards.outputs.manifest_sha256/);
  assert.match(consumer, /PRODEX_APP_FILTERS:.*matrix.filters/);
  assert.match(consumer, /PRODEX_APP_SKIP_FILTERS:.*matrix.skip_filters/);
  assert.match(consumer, /fail-fast: false/);
  assert.match(consumer, /max-parallel: 16/);
  assert.match(consumer, /full-test-partition.sh/);
  assert.doesNotMatch(consumer, /cargo test|Cache Rust|Install sccache/);
  assert.match(consumer, /TMPDIR=.*[\s\S]*PRODEX_APP_FILTERS=\$'runtime_tools::overlay::overlay_tests/);
  assert.match(consumer, /compiled_store_retention_caller_tests::/);
  assert.match(consumer, /tee "\$\{diagnostics\}\/symlink-temp-tests.log"/);
  assert.doesNotMatch(consumer, /desktop_tests|offline_self_test_accepts_system_temp_symlink_alias/);
  assert.match(workspace + consumer, /if: failure\(\)/);
});
for (const [suite, exitCode, output] of [
  ["workspace", 7, "fixture assertion failed"],
  ["commands", 9, "fixture artifact identity rejected"],
  ["remainder", 0, "running 0 tests"],
  ["commands", 0, "running 2 tests\ntest result: ok. 2 passed"],
]) test(`full-test diagnostics preserve ${suite} status ${exitCode} (${output.split("\n")[0]})`, {
  skip: process.platform === "win32",
}, () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "prodex-full-partition-"));
  try {
    const bin = path.join(dir, "bin"); mkdirSync(bin);
    const runtime = path.join(dir, "runtime"); mkdirSync(runtime);
    for (const command of ["node", "cargo", "rustc"]) {
      writeFileSync(path.join(bin, command), command === "node"
        ? `#!/bin/sh\nprintf '%s\n' "$FIXTURE_OUTPUT"\nexit "$FIXTURE_STATUS"\n`
        : "#!/bin/sh\necho fixture compiler\n", { mode: 0o755 });
    }
    const result = spawnSync("bash", ["scripts/ci/full-test-partition.sh"], {
      encoding: "utf8", timeout: 10000,
      env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, RUNNER_TEMP: dir,
        PRODEX_FULL_TEST_SUITE: suite, PRODEX_APP_FILTERS: "fixture::", PRODEX_APP_SKIP_FILTERS: "",
        PRODEX_RUNTIME_LOG_DIR: runtime, GITHUB_EVENT_NAME: "workflow_dispatch", GITHUB_REF: "refs/heads/main",
        GITHUB_SHA: "a".repeat(40), GITHUB_RUN_ID: "1", GITHUB_RUN_ATTEMPT: "1", RUNNER_OS: "Linux",
        FIXTURE_STATUS: String(exitCode), FIXTURE_OUTPUT: output },
    });
    const expected = suite === "remainder" ? 1 : exitCode;
    assert.equal(result.status, expected, result.stderr);
    const evidence = path.join(dir, `full-test-${suite}-diagnostics`);
    if (expected) {
      assert.ok(existsSync(path.join(evidence, "cargo-test-tail.txt")));
      assert.ok(readFileSync(path.join(evidence, "metadata.txt"), "utf8").includes(`exit_code=${expected}`));
      assert.ok(result.stdout.includes(output));
    } else assert.equal(existsSync(evidence), false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
