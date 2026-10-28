import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import test from "node:test";
import { validateReleaseContainerPublication } from "./supply-chain-guard.mjs";

const ci = readFileSync(".github/workflows/ci.yml", "utf8");
const release = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const target = readFileSync(".github/workflows/release-target.yml", "utf8");
const gate = release.split("\n  verify-ci:\n")[1].split("\n  antivirus-health:\n")[0];
const prerequisite = gate.split("      - name: Require all artifact gates to succeed\n")[1].split("\n      - name:")[0];
const script = prerequisite.split("        run: |\n")[1].replace(/^          /gm, "");
const required = ["antivirus-health", "build", "release-preflight", "validate-release-assets"];

test("informational tails consume no extra runner while timing analysis remains available on demand", () => {
  assert.doesNotMatch(ci, /^  ci-duration-telemetry:/m);
  assert.doesNotMatch(release, /^  (?:verify-release|publish-duration-telemetry):/m);
  assert.ok(existsSync("scripts/ci/github-job-durations.mjs"));
  assert.match(gate, /name: Record release verification verdict/);
  assert.ok(gate.indexOf("Require all artifact gates") < gate.indexOf("Check out exact release source"));
  assert.ok(gate.indexOf("Wait for CI success") < gate.indexOf("Record release verification verdict"));
});

for (const result of ["success", "failure", "cancelled", "skipped", "pending"]) {
  test(`combined verdict rejects non-success dependency: ${result}`, {skip: process.platform === "win32"}, () => {
    for (const owner of required) {
      const results = Object.fromEntries(required.map((name) => [name, {result: name === owner ? result : "success"}]));
      const run = spawnSync("bash", ["-c", script], {encoding: "utf8", timeout: 5000,
        env: {...process.env, RESULTS: JSON.stringify(results)}});
      assert.equal(run.status === 0, result === "success", run.stderr);
    }
  });
}

test("empty and missing artifact verdicts fail closed", {skip: process.platform === "win32"}, () => {
  for (const results of [{}, {build: {result: "success"}}, null]) {
    const run = spawnSync("bash", ["-c", script], {encoding: "utf8", timeout: 5000,
      env: {...process.env, RESULTS: JSON.stringify(results)}});
    assert.notEqual(run.status, 0);
  }
});

test("combined gate cannot be detached, made optional, or report success before CI approval", () => {
  assert.deepEqual(validateReleaseContainerPublication(release, target), []);
  for (const marker of ["      - build", "      - validate-release-assets", "      - antivirus-health",
    "RESULTS: ${{ toJSON(needs) }}", 'all(.[]; .result == "success")']) {
    assert.ok(validateReleaseContainerPublication(release.replace(gate, gate.replace(marker, "removed")), target).length > 0, marker);
  }
  for (const name of ["Require all artifact gates to succeed", "Wait for CI success"]) {
    for (const override of ["if: false", "continue-on-error: true"]) {
      const damaged = release.replace(`      - name: ${name}\n`, `      - name: ${name}\n        ${override}\n`);
      assert.ok(validateReleaseContainerPublication(damaged, target).length > 0, `${name} ${override}`);
    }
  }
});
