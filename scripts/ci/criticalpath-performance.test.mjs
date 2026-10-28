import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { validateLinuxNativeTests, validateReleaseContainerPublication, validateReleaseCodexExclusion } from "./supply-chain-guard.mjs";
const ci = readFileSync(".github/workflows/ci.yml", "utf8");
const release = readFileSync(".github/workflows/release-target.yml", "utf8");
function job(source, name) {
  const start = source.indexOf(`\n  ${name}:\n`);
  assert.ok(start >= 0, `missing ${name}`);
  const next = source.slice(start + 1).search(/\n  [a-z][a-z0-9-]*:\n/);
  return source.slice(start, next < 0 ? source.length : start + next + 1);
}
test("Linux application partitions share one source-bound harness, not repeated compilation", () => {
  const producer = job(ci, "linux-app-test-build");
  const consumer = job(ci, "prodex-app-lib");
  assert.match(producer, /needs: \[changes, mojo-runtime-linux-archive\]/);
  assert.match(producer, /native-test-artifact\.mjs pack/);
  assert.match(producer, /PRODEX_MOJO_ARCHIVE:/);
  assert.match(producer, /manifest_sha256: \$\{\{ steps.pack.outputs.manifest_sha256 \}\}/);
  assert.match(consumer, /needs: \[changes, linux-app-test-build\]/);
  assert.match(consumer, /TEST_MANIFEST_SHA256: \$\{\{ needs.linux-app-test-build.outputs.manifest_sha256 \}\}/);
  assert.match(consumer, /native-test-artifact\.mjs run/);
  assert.match(consumer, /PRODEX_APP_FILTERS: \$\{\{ matrix.filters \}\}/);
  assert.match(consumer, /PRODEX_APP_SKIP_FILTERS: \$\{\{ matrix.skip_filters \}\}/);
  assert.match(consumer, /fromJSON\(needs.changes.outputs.prodex_app_matrix\)/);
  assert.doesNotMatch(consumer, /cargo test|Install sccache|Cache Rust dependencies/);
  assert.match(job(ci, "ci-duration-telemetry"), /- linux-app-test-build\n/);
});
test("macOS native regressions run independently of the workspace suite without dropping filters", () => {
  const workspace = job(ci, "macos-workspace");
  const native = job(ci, "macos-native-regressions");
  for (const part of [workspace, native]) {
    assert.match(part, /needs: \[changes, mojo-runtime-platform-archives\]/);
    assert.match(part, /runs-on: macos-14/);
    assert.match(part, /PRODEX_MOJO_ARCHIVE:/);
    assert.match(part, /shared-key: macos-workspace/);
  }
  assert.match(workspace, /test:full -- --jobs 2 --test-threads 2 --no-prodex-app-lib/);
  assert.doesNotMatch(workspace, /Run native macOS/);
  assert.match(native, /runtime_broker -- --test-threads=1/);
  assert.match(native, /run_strategy_auto_routes_kiro_resume_sessions_to_provider_bridge/);
  assert.match(native, /grep -Fq 'running 0 tests'/);
  assert.match(native, /save-if: false/);
  assert.match(native, /SCCACHE_GHA_RW_MODE: READ_ONLY/);
  assert.match(job(ci, "ci-duration-telemetry"), /- macos-native-regressions\n/);
});

test("Linux release artifact smoke starts after its own binary, not unrelated platforms", () => {
  const parent = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const build = job(release, "build");
  const smoke = build.slice(build.indexOf("      - name: Download official Linux artifact\n"));
  assert.ok(build.indexOf("Upload binary artifact") < build.indexOf("Download official Linux artifact"));
  assert.doesNotMatch(release, /\n  artifact-smoke:/);
  assert.match(smoke, /if: \$\{\{ matrix.target == 'x86_64-unknown-linux-gnu' \}\}/);
  assert.match(smoke, /name: x86_64-unknown-linux-gnu/);
  assert.match(smoke, /RELEASE_VERSION: \$\{\{ inputs.version \}\}/);
  assert.match(smoke, /release-artifact-smoke\.mjs/);
  assert.match(smoke, /npm-package-smoke\.mjs --binary-dir artifact/);
  assert.match(build, /persist-credentials: false/);
  assert.doesNotMatch(smoke, /contents: write|attestations: write|id-token: write|cargo (build|run)/);
  assert.doesNotMatch(parent, /\n  artifact-smoke:\n/);
  for (const name of ["prepare-release", "verify-release"]) {
    assert.match(job(parent, name), /      - build\n/);
  }
});

test("trust gates reject detached Linux artifacts and skipped release smoke", () => {
  const parent = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  assert.deepEqual(validateLinuxNativeTests(ci), []);
  for (const marker of ["needs: [changes, linux-app-test-build]", "TEST_MANIFEST_SHA256: ${{ needs.linux-app-test-build.outputs.manifest_sha256 }}"]) {
    assert.ok(validateLinuxNativeTests(ci.replace(marker, "removed")).length > 0);
  }
  assert.deepEqual(validateReleaseContainerPublication(parent, release), []);
  assert.deepEqual(validateReleaseCodexExclusion(parent, release), []);
  const build = job(release, "build");
  const smoke = build.slice(build.indexOf("      - name: Download official Linux artifact\n"));
  for (const marker of ["path: artifact", "node scripts/ci/release-artifact-smoke.mjs", "matrix.target"]) {
    const damaged = release.replace(smoke, smoke.replace(marker, "removed"));
    assert.ok(validateReleaseContainerPublication(parent, damaged).length > 0, marker);
  }
  assert.ok(validateReleaseContainerPublication(parent, release.replace("      - name: Run official artifact smoke\n", "      - name: Run official artifact smoke\n        continue-on-error: true\n")).length > 0);
  assert.ok(validateReleaseCodexExclusion(parent, release.replace("npm-package-smoke.mjs --binary-dir artifact", "removed")).length > 0);
});
