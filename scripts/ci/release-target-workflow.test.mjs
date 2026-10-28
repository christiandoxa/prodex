import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import test from "node:test";
import { validateReleaseContainerPublication } from "./supply-chain-guard.mjs";

const parent = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const childPath = ".github/workflows/release-target.yml";
function job(source, name) {
  const start = source.indexOf(`\n  ${name}:\n`);
  assert.ok(start >= 0, `missing job ${name}`);
  const next = source.slice(start + 1).search(/\n  [a-z][a-z0-9-]*:\n/);
  return source.slice(start, next < 0 ? source.length : start + 1 + next);
}

test("each release target owns its archive-to-binary dependency instead of waiting for other targets", () => {
  assert.ok(existsSync(childPath), "target-local reusable workflow is missing");
  const child = readFileSync(childPath, "utf8");
  const caller = job(parent, "build");
  const archive = job(child, "build-mojo-archives");
  const binary = job(child, "build");
  assert.match(caller, /uses: \.\/\.github\/workflows\/release-target\.yml/);
  assert.match(caller, /needs: release-preflight/);
  assert.doesNotMatch(caller, /build-mojo-archives|runs-on: \$\{\{ matrix/);
  assert.match(caller, /platforms: \$\{\{ format\('\[\{0\}\]', toJSON\(matrix\)\) \}\}/);
  const targets = [...caller.matchAll(/- target: ([a-z0-9_-]+)/g)].map((m) => m[1]);
  assert.deepEqual(targets, ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]);
  for (const source of [archive, binary]) {
    assert.match(source, /include: \$\{\{ fromJSON\(inputs.platforms\) \}\}/);
    assert.match(source, /fail-fast: false/);
    assert.doesNotMatch(source, /contents: write|attestations: write|id-token: write/);
  }
  assert.match(binary, /needs: build-mojo-archives/);
  assert.doesNotMatch(archive, /needs:.*(?:build|verify-ci)/);
  assert.match(archive, /Validate archive source identity/);
  assert.match(archive, /Verify target archive/);
  assert.match(binary, /Verify compiled-in Mojo release/);
  assert.match(binary, /Native smoke test/);
  assert.match(child, /^on:\n  workflow_call:/m);
  assert.doesNotMatch(child, /workflow_dispatch:|gh release|git push|attest-build-provenance/);
});

test("the aggregate target result is required before all release asset and publication paths", () => {
  for (const name of ["validate-release-assets", "verify-ci"]) {
    assert.match(job(parent, name), /      - build\n/, `${name} lost all-platform success`);
  }
  for (const name of ["attest-binaries", "sync-release-docs", "publish-github-release"]) {
    const body = job(parent, name);
    assert.match(body, /if: \$\{\{ inputs.publish \}\}/);
    assert.match(body, /      - verify-ci\n/);
    assert.match(body, /      - validate-release-assets\n/);
  }
});


test("supply-chain gate validates called target steps and rejects a foreign or writable workflow", () => {
  const child = readFileSync(childPath, "utf8");
  assert.deepEqual(validateReleaseContainerPublication(parent, child), []);
  for (const marker of ["needs: build-mojo-archives", "cargo install cross --version 0.2.5 --locked", "persist-credentials: false"]) {
    assert.ok(validateReleaseContainerPublication(parent, child.replace(marker, "removed")).length > 0, marker);
  }
  assert.ok(validateReleaseContainerPublication(parent, child.replace("contents: read", "contents: write")).length > 0);
  assert.ok(validateReleaseContainerPublication(parent.replace("./.github/workflows/release-target.yml", "untrusted/example/.github/workflows/build.yml@main"), child).length > 0);
  assert.ok(validateReleaseContainerPublication(parent.replace(job(parent, "build"), job(parent, "build").replace("needs: release-preflight", "needs: arbitrary")), child).length > 0);
});


test("superseded verification runs cancel without ever cancelling a real publication", () => {
  assert.match(parent, /group: \$\{\{ inputs.publish && 'standalone-release' \|\| format\('standalone-verify-\{0\}', inputs.target_sha\) \}\}/);
  assert.match(parent, /cancel-in-progress: \$\{\{ !inputs.publish \}\}/);
  assert.match(parent, /      publish:\n[\s\S]*?default: false\n\s+type: boolean/);
});
