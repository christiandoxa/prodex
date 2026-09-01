import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

const script = "scripts/ci/release-artifact-smoke.mjs";

test("artifact smoke requires an explicit artifact and release version", () => {
  const help = spawnSync(process.execPath, [script, "--help"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /--binary PATH/);
  assert.match(help.stdout, /--version VERSION/);

  const missing = spawnSync(process.execPath, [script], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
  assert.notEqual(missing.status, 0);
  assert.match(missing.stderr, /--binary and --version are required/);
});

test("standalone release runs the downloaded artifact smoke before SBOM preparation", () => {
  const source = readFileSync(script, "utf8");
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const smoke = workflow.match(/\n  artifact-smoke:\n([\s\S]*?)\n  attest-binaries:/u)?.[1];
  const prepare = workflow.match(/\n  prepare-release:\n([\s\S]*?)\n  sync-release-docs:/u)?.[1];

  assert.ok(smoke, "artifact smoke job missing");
  assert.ok(prepare, "release preparation job missing");
  assert.match(smoke, /- verify-ci/);
  assert.match(smoke, /- build/);
  assert.match(smoke, /name: x86_64-unknown-linux-gnu/);
  assert.match(smoke, /binary="artifact\/prodex"/u);
  assert.match(smoke, /node scripts\/ci\/release-artifact-smoke\.mjs \\\n\s+--binary/u);
  assert.match(prepare, /- artifact-smoke/);
  assert.doesNotMatch(smoke, /cargo\s+(run|build)/u);
  assert.doesNotMatch(source, /target\/(?:debug|release)/u);
});
