import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { validateReleaseContainerPublication, validateReleaseMalwareGate } from "./supply-chain-guard.mjs";

const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const jobs = new Map([...workflow.matchAll(/^  ([a-z][a-z0-9-]*):\n([\s\S]*?)(?=^  [a-z][a-z0-9-]*:|$(?![\s\S]))/gm)]
  .filter(([, name, body]) => body.includes("    runs-on:"))
  .map(([, name, body]) => [name, body]));

function needs(name) {
  const body = jobs.get(name);
  assert.ok(body, `missing job ${name}`);
  const value = body.match(/^    needs:([^\n]*)(?:\n((?:      - [^\n]*\n)*))?/m);
  if (!value) return [];
  return value[1].trim().replace(/[\[\]]/g, "").split(/,\s*/).filter(Boolean)
    .concat([...(value[2] ?? "").matchAll(/- ([a-z0-9-]+)/g)].map((match) => match[1]));
}

function ancestors(name, visiting = new Set()) {
  assert.ok(!visiting.has(name), `dependency cycle at ${name}`);
  const next = new Set(visiting).add(name);
  return new Set(needs(name).flatMap((dependency) => [dependency, ...ancestors(dependency, next)]));
}

test("manual release workflow defaults to nonpublishing verification", () => {
  assert.match(workflow, /      publish:\n[\s\S]*?default: false\n\s+type: boolean/);
  assert.match(workflow, /^run-name:.*inputs\.publish.*publish.*verify/m);
  for (const [name, body] of jobs) {
    if (/contents: write|attestations: write|id-token: write/.test(body)) {
      assert.match(body, /    if: \$\{\{ inputs\.publish \}\}/, `${name} has unguarded write permissions`);
    }
  }
  assert.match(jobs.get("publish-github-release"), /RELEASE_PUBLISH: \$\{\{ inputs\.publish \}\}/);
  assert.match(jobs.get("publish-github-release"), /test "\$\{RELEASE_PUBLISH\}" = true/);
});

test("read-only binaries, smoke and SBOM overlap full CI without losing source checks", () => {
  for (const name of ["build-mojo-archives", "build", "artifact-smoke", "prepare-release", "validate-release-assets"]) {
    const dependencies = ancestors(name);
    assert.ok(dependencies.has("release-preflight"), `${name} needs validated immutable source`);
    assert.ok(!dependencies.has("verify-ci"), `${name} must not idle behind full CI`);
    assert.doesNotMatch(jobs.get(name), /contents: write|attestations: write|id-token: write/);
  }
  assert.ok(needs("verify-ci").includes("release-preflight"));
  assert.match(jobs.get("verify-ci"), /Wait for CI success/);
  assert.match(jobs.get("release-preflight"), /exact dispatched main commit/);
  assert.match(jobs.get("release-preflight"), /Verify pinned Kiro CLI release/);
});

test("every publishing side effect waits for successful exact-source CI and asset validation", () => {
  for (const name of ["attest-binaries", "sync-release-docs", "publish-github-release"]) {
    const dependencies = ancestors(name);
    assert.ok(dependencies.has("verify-ci"), `${name} missing CI gate`);
    assert.ok(dependencies.has("validate-release-assets"), `${name} missing artifact verification`);
  }
  const publish = jobs.get("publish-github-release");
  assert.ok(ancestors("publish-github-release").has("attest-binaries"));
  assert.match(publish, /Verify release provenance/);
  assert.match(publish, /--source-digest "\$\{TARGET_SHA\}"/);
  assert.match(publish, /--source-ref refs\/heads\/main/);
  assert.match(publish, /--deny-self-hosted-runners/);
  assert.match(publish, /refusing to overwrite/);
  assert.match(jobs.get("validate-release-assets"), /sha256sum --check SHA256SUMS/);
  assert.match(jobs.get("validate-release-assets"), /Scan release assets for malware/);
  assert.match(jobs.get("validate-release-assets"), /EICAR/);
  assert.match(jobs.get("validate-release-assets"), /Validate curated release notes/);
});

test("verification verdict waits for every build and smoke gate, not skipped publishing jobs", () => {
  for (const name of ["verify-ci", "build-mojo-archives", "build", "artifact-smoke", "prepare-release", "validate-release-assets"]) {
    assert.ok(needs("verify-release").includes(name), `verification verdict omits ${name}`);
  }
  assert.match(jobs.get("verify-release"), /always\(\)/);
  assert.match(jobs.get("verify-release"), /result == "success"/);
  assert.match(jobs.get("validate-release-assets"), /name: validated-release-assets/);
  for (const body of jobs.values()) {
    assert.doesNotMatch(body, /gh release (?:edit|upload)|git tag ["']|push origin.*refs\/tags/);
  }
});


test("supply-chain guard rejects missing publication gates and unscanned asset substitution", () => {
  assert.deepEqual(validateReleaseContainerPublication(workflow), []);
  assert.deepEqual(validateReleaseMalwareGate(workflow), []);
  for (const marker of [
    "    if: ${{ inputs.publish }}",
    "      - verify-ci",
    "      - validate-release-assets",
    "      - attest-binaries",
    '--source-digest "${TARGET_SHA}"',
    'test "${RELEASE_PUBLISH}" = true',
  ]) {
    const damaged = workflow.replace(jobs.get("publish-github-release"), jobs.get("publish-github-release").replace(marker, ""));
    assert.ok(validateReleaseContainerPublication(damaged).length > 0, `guard accepted removed ${marker}`);
  }
  for (const marker of ["- validate-release-assets", "name: validated-release-assets", "sha256sum --check SHA256SUMS"]) {
    const damaged = workflow.replace(jobs.get("publish-github-release"), jobs.get("publish-github-release").replace(marker, ""));
    assert.ok(validateReleaseMalwareGate(damaged).length > 0, `guard accepted unscanned publication: ${marker}`);
  }
  const damaged = workflow.replace("- name: Scan release assets for malware", "- name: Scan omitted");
  assert.ok(validateReleaseMalwareGate(damaged).length > 0);
  const writable = workflow.replace(jobs.get("prepare-release"), jobs.get("prepare-release").replace("contents: read", "contents: write"));
  assert.ok(validateReleaseContainerPublication(writable).length > 0, "preparation must not gain publication authority");
});
