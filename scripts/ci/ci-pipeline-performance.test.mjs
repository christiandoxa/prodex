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


test("Apple Silicon Intel release benchmark is opt-in, source-pinned and cannot publish", () => {
  const workflow = readFileSync(".github/workflows/release-macos-cross-calibration.yml", "utf8");
  assert.match(workflow, /name: macOS Intel cross-build calibration/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /^  push:/m);
  assert.match(workflow, /runs-on: macos-14/);
  assert.match(workflow, /permissions:\n  contents: read\n  actions: read/);
  assert.match(workflow, /\.head_sha == \$sha/);
  assert.match(workflow, /\.conclusion == "success"/);
  assert.match(workflow, /\.event == "push"/);
  assert.match(workflow, /- name: Check out exact CI source/);
  assert.match(workflow, /run-id:/);
  assert.match(workflow, /name: mojo-runtime-archive-x86_64-apple-darwin/);
  assert.match(workflow, /PRODEX_MOJO_REQUIRED: "1"/);
  assert.match(workflow, /cargo build --release --locked --target x86_64-apple-darwin/);
  assert.match(workflow, /lipo -archs/);
  assert.match(workflow, /arch -x86_64/);
  assert.match(workflow, /"implementation": "mojo-compiled-in"/);
  assert.doesNotMatch(workflow, /gh release (?:create|upload|edit)/);
  assert.doesNotMatch(workflow, /git push/);
  assert.doesNotMatch(workflow, /attest-build-provenance/);
});


test("official Intel macOS release uses measured Apple Silicon cross-build with Rosetta safety", () => {
  const build = workflowJob(release, "build", "attest-binaries");
  const intel = build.match(/- target: x86_64-apple-darwin\n([\s\S]*?)- target: aarch64-apple-darwin/)?.[1];
  assert.ok(intel, "x86_64 macOS release matrix entry missing");
  assert.match(intel, /runs-on: macos-14/);
  assert.doesNotMatch(intel, /macos-15-intel/);
  assert.match(intel, /can-native-smoke: true/);
  assert.match(intel, /can-mojo-runtime-smoke: true/);

  assert.match(build, /name: Require Rosetta for Intel macOS cross-target smoke/);
  assert.match(build, /test "\$\(uname -m\)" = arm64/);
  assert.match(build, /softwareupdate --install-rosetta --agree-to-license/);
  assert.match(build, /key: release-[^\n]*-arm64-cross/);
  assert.match(build, /name: Verify macOS Intel artifact architecture and Rosetta execution/);
  assert.match(build, /test "\$\(lipo -archs "\$\{binary\}"\)" = x86_64/);
  assert.match(build, /arch -x86_64 "\$\{binary\}" --version/);
  assert.match(build, /name: Verify compiled-in Mojo release/);
  assert.match(build, /"implementation": "mojo-compiled-in"/);
  assert.match(build, /"self_test": "passed"/);
  assert.match(build, /name: Native smoke test/);
  assert.match(build, /name: Upload binary artifact/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
});
