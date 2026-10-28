import assert from "node:assert/strict";
import { readFileSync, writeFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { validateReleaseContainerPublication } from "./supply-chain-guard.mjs";
const parent = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const target = readFileSync(".github/workflows/release-target.yml", "utf8");
function job(source, name) {
  const block = source.split(`\n  ${name}:\n`)[1];
  assert.ok(block, `missing job ${name}`);
  return block.split(/\n  [a-z][a-z0-9-]*:/u)[0];
}
function step(name) {
  const body = target.split(`      - name: ${name}\n`)[1];
  assert.ok(body, `missing step ${name}`);
  return body.split("\n      - name:")[0];
}
function shell(name) {
  return step(name).split("        run: |\n")[1].replace(/^          /gmu, "");
}
test("release CI verdict waits without allocating a runner before target builds", () => {
  const verify = job(parent, "verify-ci");
  assert.match(verify, /needs: \[release-preflight, build\]/);
  assert.match(job(parent, "build"), /needs: release-preflight/);
  assert.doesNotMatch(job(parent, "build"), /needs:.*verify-ci/);
  assert.match(verify, /get\("headSha"\) == target/);
  assert.match(verify, /run.get\("conclusion"\) == "success"/);
  for (const name of ["attest-binaries", "sync-release-docs", "publish-github-release"]) {
    assert.match(job(parent, name), /- verify-ci/);
    assert.match(job(parent, name), /- validate-release-assets/);
    assert.match(job(parent, name), /if: \$\{\{ inputs.publish \}\}/);
  }
  assert.match(parent, /publish:[\s\S]*?default: false/);
});
test("no release target has a permanent skipped smoke job or compile-only fallback", () => {
  assert.doesNotMatch(target, /\n  artifact-smoke:|Verify non-native Mojo release artifact/);
  const matrix = job(parent, "build");
  assert.equal((matrix.match(/- target:/gu) ?? []).length, 6);
  assert.equal((matrix.match(/can-mojo-runtime-smoke: true/gu) ?? []).length, 6);
  assert.match(step("Verify compiled-in Mojo release"), /self_test.*passed/);
  for (const name of ["Download official Linux artifact", "Run official artifact smoke"]) {
    assert.match(step(name), /if: \$\{\{ matrix.target == 'x86_64-unknown-linux-gnu' \}\}/);
  }
  assert.ok(target.indexOf("Upload binary artifact") < target.indexOf("Download official Linux artifact"));
});
test("cross driver cache is version/host pinned and validates the restored executable", () => {
  assert.match(step("Cache pinned cross tool"), /github.ref == 'refs\/heads\/main'/);
  assert.match(step("Cache pinned cross tool"), /key: prodex-cross-tool-v1-0.2.5-rust1.99.1-.*runner.os.*runner.arch/);
  assert.doesNotMatch(step("Cache pinned cross tool"), /restore-keys:/);
  const install = shell("Install cross");
  assert.match(install, /cargo install cross --version 0.2.5 --locked --root/);
  assert.match(install, /cross_version=.*bin\/cross-util.*--version/);
  assert.match(install, /cached cross version did not match/);
  assert.doesNotMatch(install, /command -v cross|for attempt|sleep/);
});
test("relocated release smoke remains a mandatory downloaded-bytes gate", () => {
  assert.deepEqual(validateReleaseContainerPublication(parent, target), []);
  for (const marker of ["Download official Linux artifact", "Run official artifact smoke", "path: artifact"]) {
    assert.ok(validateReleaseContainerPublication(parent, target.replace(marker, "removed")).length > 0, marker);
  }
  const broken = target.replace("      - name: Run official artifact smoke\n", "      - name: Run official artifact smoke\n        continue-on-error: true\n");
  assert.ok(validateReleaseContainerPublication(parent, broken).length > 0);
});
for (const [platform, cross] of [["x86_64-unknown-linux-gnu", true], ["x86_64-pc-windows-msvc", false], ["aarch64-apple-darwin", false]]) {
  test(`compiler failure is returned once without retrying the entire ${platform} build`, { skip: process.platform === "win32" }, () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "prodex-build-failure-"));
    try {
      const bin = path.join(root, "bin"); mkdirSync(bin);
      const archive = path.join(root, "target/mojo-release", platform); mkdirSync(archive, { recursive: true });
      writeFileSync(path.join(archive, platform.endsWith("msvc") ? "prodex_mojo_core.lib" : "libprodex_mojo_core.a"), "synthetic archive existence fixture");
      for (const command of ["cross", "cargo"]) writeFileSync(path.join(bin, command), '#!/bin/sh\nprintf "%s\n" "$*" >> "$PRODEX_FIXTURE_CALLS"\nexit 17\n', { mode: 0o755 });
      const replacements = { target: platform, "use-cross": String(cross), "mojo-release": "true", "mojo-target-cpu": "generic", "mojo-features": "mojo-core" };
      const command = shell("Build prodex").replace(/\$\{\{ matrix.([a-z-]+) \}\}/gu, (_, key) => {
        assert.ok(Object.hasOwn(replacements, key)); return replacements[key];
      });
      const calls = path.join(root, "calls");
      const result = spawnSync("bash", ["-c", command], { cwd: root, encoding: "utf8", timeout: 5000,
        env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, MOJO_VERSION: "1.2.0", PRODEX_FIXTURE_CALLS: calls } });
      assert.equal(result.status, 17, result.stderr);
      const lines = readFileSync(calls, "utf8").trim().split("\n");
      assert.equal(lines.length, 1);
      assert.match(lines[0], /build --locked --release --target/);
      assert.match(lines[0], /--features mojo-core/);
      assert.match(target, /CARGO_NET_RETRY: "3"/);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });
}

for (const version of ["cross 0.2.5", "cross 0.3.0"]) {
  test(`cross cache ${version} uses the pure companion probe, never Cargo delegation`, { skip: process.platform === "win32" }, () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "prodex-cross-cache-"));
    try {
      const bin = path.join(root, ".cache/prodex-ci/cross-0.2.5/bin"); mkdirSync(bin, { recursive: true });
      writeFileSync(path.join(bin, "cross"), '#!/bin/sh\necho "cross version must not launch Cargo" >&2\nexit 99\n', { mode: 0o755 });
      writeFileSync(path.join(bin, "cross-util"), `#!/bin/sh\nprintf '%s\n' '${version}'\n`, { mode: 0o755 });
      const output = path.join(root, "github-path");
      const result = spawnSync("bash", ["-c", shell("Install cross")], { cwd: root, encoding: "utf8", timeout: 5000,
        env: { ...process.env, HOME: root, GITHUB_PATH: output } });
      assert.equal(result.status, version === "cross 0.2.5" ? 0 : 1, result.stderr);
      if (version === "cross 0.2.5") assert.equal(readFileSync(output, "utf8").trim(), bin);
      else assert.match(result.stderr, /cached cross version did not match/);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });
}
