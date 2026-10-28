import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { validateReleaseContainerPublication, validateReleaseMalwareGate } from "./supply-chain-guard.mjs";

const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const targetWorkflow = readFileSync(".github/workflows/release-target.yml", "utf8");
const jobs = new Map([...workflow.matchAll(/^  ([a-z][a-z0-9-]*):\n([\s\S]*?)(?=^  [a-z][a-z0-9-]*:|$(?![\s\S]))/gm)]
  .filter(([, name, body]) => (body.includes("    runs-on:") || body.includes("    uses:")))
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
  for (const name of ["build", "validate-release-assets"]) {
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
  assert.match(jobs.get("antivirus-health"), /EICAR/);
  assert.match(jobs.get("validate-release-assets"), /Validate curated release notes/);
});

test("verification verdict waits for every build and smoke gate, not skipped publishing jobs", () => {
  for (const name of ["verify-ci", "build", "validate-release-assets"]) {
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
  assert.deepEqual(validateReleaseContainerPublication(workflow, targetWorkflow), []);
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
    assert.ok(validateReleaseContainerPublication(damaged, targetWorkflow).length > 0, `guard accepted removed ${marker}`);
  }
  for (const marker of ["- validate-release-assets", "name: validated-release-assets", "sha256sum --check SHA256SUMS"]) {
    const damaged = workflow.replace(jobs.get("publish-github-release"), jobs.get("publish-github-release").replace(marker, ""));
    assert.ok(validateReleaseMalwareGate(damaged).length > 0, `guard accepted unscanned publication: ${marker}`);
  }
  const damaged = workflow.replace("- name: Scan release assets for malware", "- name: Scan omitted");
  assert.ok(validateReleaseMalwareGate(damaged).length > 0);
  const writable = workflow.replace(jobs.get("validate-release-assets"), jobs.get("validate-release-assets").replace("contents: read", "contents: write"));
  assert.ok(validateReleaseContainerPublication(writable, targetWorkflow).length > 0, "preparation must not gain publication authority");
});


test("the same pinned antivirus engine is proven during compilation, before final assets can pass", () => {
  assert.ok(jobs.has("antivirus-health"), "early antivirus health proof missing");
  assert.deepEqual(needs("antivirus-health"), ["release-preflight"]);
  assert.ok(!ancestors("antivirus-health").has("build"));
  assert.ok(ancestors("validate-release-assets").has("antivirus-health"));
  assert.ok(needs("verify-release").includes("antivirus-health"));
  for (const name of ["antivirus-health", "validate-release-assets"]) {
    const body=jobs.get(name);
    assert.match(body, /Verify pinned antivirus image/);
    assert.match(body, /RepoDigests/);
    assert.match(body, /clamav_update: "True"/);
    assert.doesNotMatch(body, /contents: write|id-token: write/);
  }
  assert.match(jobs.get("antivirus-health"), /EICAR/);
  assert.doesNotMatch(jobs.get("validate-release-assets"), /Prepare antivirus engine health check/);
  const noHealth=workflow.replace(jobs.get("validate-release-assets"),jobs.get("validate-release-assets").replace("      - antivirus-health\n", ""));
  assert.ok(validateReleaseMalwareGate(noHealth).length>0, "engine proof must gate clean assets");
});

test("one read-only artifact consumer generates mandatory SBOM before validating scanned assets", () => {
  assert.ok(!jobs.has("prepare-release"));
  const validation = jobs.get("validate-release-assets");
  assert.equal((workflow.match(/- name: Download built binaries/g) ?? []).length, 1);
  assert.doesNotMatch(workflow, /- name: (?:Upload|Download) release SBOM/);
  assert.ok(needs("validate-release-assets").includes("build"));
  for (const marker of ["- name: Generate release SBOM", "dir:/source -o spdx-json > release-sbom.spdx.json",
    "codex-purity-guard.mjs sbom-input", "codex-purity-guard.mjs release-sbom.spdx.json",
    "cp release-sbom.spdx.json release-assets/release-sbom.spdx.json"]) {
    const damaged = workflow.replace(validation, validation.replace(marker, "removed"));
    assert.ok(validateReleaseContainerPublication(damaged, targetWorkflow).length > 0, marker);
  }
  for (const control of ["if: false", "continue-on-error: true"]) {
    const damaged = workflow.replace("- name: Generate release SBOM\n", `- name: Generate release SBOM\n        ${control}\n`);
    assert.ok(validateReleaseContainerPublication(damaged, targetWorkflow).length > 0, control);
  }
});
