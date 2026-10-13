import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const ci = readFileSync(".github/workflows/ci.yml", "utf8");
const release = readFileSync(".github/workflows/standalone-release.yml", "utf8");

function workflowJob(source, name, nextName) {
  const start = source.indexOf("\n  " + name + ":\n");
  const end = source.indexOf("\n  " + nextName + ":\n", start + 1);
  assert.ok(start >= 0 && end > start, "missing job " + name);
  return source.slice(start, end);
}

test("Mojo parity fans out without dropping production tests", () => {
  const lint = workflowJob(ci, "real-mojo", "real-mojo-parity");
  const parity = workflowJob(ci, "real-mojo-parity", "ci-duration-telemetry");
  const archive = workflowJob(ci, "mojo-runtime-linux-archive", "mojo-runtime-platform-archives");
  assert.match(lint, /needs: \[changes, mojo-runtime-linux-archive\]/);
  assert.match(parity, /needs: \[changes, mojo-runtime-linux-archive\]/);
  assert.match(lint, /cargo clippy --locked --workspace --all-targets --all-features -- -D warnings/);
  assert.match(lint, /cargo build --locked --features mojo-core --bin prodex/);
  assert.match(lint, /"implementation": "mojo-compiled-in"/);
  assert.match(lint, /"fallback": false/);
  assert.match(lint, /- name: Download strict Linux Mojo archive/);
  assert.doesNotMatch(lint, /Install pinned Mojo toolchain/);
  assert.match(archive, /Install pinned Mojo toolchain/);
  assert.match(archive, /PRODEX_MOJO_REQUIRED=1/);

  const groups = [...parity.matchAll(/^\s+- group: ([a-z-]+)$/gm)].map((match) => match[1]);
  assert.deepEqual(groups, ["package", "provider", "app-core"]);
  for (const group of groups) {
    assert.ok(parity.includes("if: $" + "{{ matrix.group == '" + group + "' }}"));
  }
  const labels = [...parity.matchAll(/^\s+run_test ([a-z-]+) cargo test --locked -q -p ([a-z-]+) ([^\n]+)$/gm)];
  assert.equal(labels.length, 14, "original 14 tests retained");
  const names = labels.map((match) => match[1]);
  assert.equal(new Set(names).size, 14);
  assert.deepEqual(names, [
    "quota", "runtime-proxy", "runtime-quota", "provider-spi",
    "provider-core", "runtime-tuning", "runtime-json-only", "runtime-launch",
    "context", "runtime-policy", "gateway-constraints",
    "smart-context-rehydrate", "mojo-cli", "mojo-core",
  ]);
  for (const match of labels) {
    assert.match(match[0], /--test-threads=1/, "missing regression boundary for " + match[1]);
  }
  assert.equal([...parity.matchAll(/wait "\$\{pids\[index\]\}"/g)].length, 3);
  assert.match(parity, /PRODEX_MOJO_REQUIRED: "1"/);
  assert.match(parity, /PRODEX_MOJO_ARCHIVE:/);
  assert.match(parity, /fail-fast: false/);
  const telemetry = ci.slice(ci.indexOf("\n  ci-duration-telemetry:\n"));
  assert.match(telemetry, /- real-mojo-parity\n/);
  assert.match(telemetry, /- real-mojo\n/);
});

test("release archives overlap CI verification but binary publication remains gated", () => {
  const verification = workflowJob(release, "verify-ci", "build-mojo-archives");
  const archive = workflowJob(release, "build-mojo-archives", "build");
  const build = workflowJob(release, "build", "attest-binaries");
  const attest = workflowJob(release, "attest-binaries", "prepare-release");
  assert.match(verification, /Wait for CI success/);
  assert.doesNotMatch(archive.split("    steps:")[0], /needs:\s*verify-ci/);
  assert.match(archive, /Validate archive source identity before expensive compilation/);
  assert.match(archive, /RELEASE_TARGET_SHA:/);
  assert.match(archive, /RELEASE_VERSION:/);
  assert.match(archive, /test "\$\{GITHUB_SHA\}" = "\$\{RELEASE_TARGET_SHA\}"/);
  assert.match(archive, /test "\$\{actual_version\}" = "\$\{RELEASE_VERSION\}"/);
  assert.match(archive, /PRODEX_MOJO_REQUIRED=1/);
  assert.match(archive, /Build target Mojo archive/);
  assert.match(archive, /Verify target archive/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
  assert.match(build, /PRODEX_MOJO_ARCHIVE=/);
  assert.match(build, /Verify compiled-in Mojo release/);
  assert.match(attest, /needs:\s*\n\s*- build\n\s*- verify-ci/);
  assert.match(release, /Verify release provenance/);
  assert.match(release, /sha256sum --check SHA256SUMS/);
  assert.match(release, /Scan release assets for malware/);
});
