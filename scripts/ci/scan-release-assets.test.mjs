import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const scanner = fileURLToPath(new URL("./scan-release-assets.sh", import.meta.url));
const imageDigest = "clamav/clamav@sha256:64e30c0bf976198024deb3dc6d5cfe2485fb969c0f73549ba0fdb364b9469614";
const finding = "/scan/prodex-x86_64-pc-windows-msvc.exe: Win.Trojan.Virut-225 FOUND";
const reviewed172 = "97a738a530dd51c0bd1ad2c1dfbaef8864a727f57a596ee850969b747a0f590c";

function runPolicy(overrides = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "prodex-scan-policy-"));
  const bin = path.join(root, "bin");
  const scope = path.join(root, overrides.scope ?? "release-assets");
  const calls = path.join(root, "calls.txt");
  mkdirSync(bin);
  mkdirSync(scope);
  writeFileSync(path.join(scope, "prodex-x86_64-pc-windows-msvc.exe"), "synthetic policy fixture, not an executable\n");
  // Only external process results are substituted; the production shell policy runs unchanged.
  writeFileSync(path.join(bin, "docker"), `#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> "$PRODEX_SCAN_TEST_CALLS"
case "$1" in
  pull) exit 0 ;;
  image) printf '%s\\n' "$PRODEX_SCAN_TEST_DIGEST" ;;
  volume)
    if [[ "$2" == create ]]; then printf '%s\\n' isolated-test-volume; fi
    ;;
  run)
    if [[ " $* " == *" --entrypoint freshclam "* ]]; then
      exit "$PRODEX_SCAN_TEST_FRESHCLAM_STATUS"
    fi
    if [[ " $* " == *" --entrypoint clamscan "* ]]; then
      printf '%s' "$PRODEX_SCAN_TEST_OUTPUT"
      exit "$PRODEX_SCAN_TEST_SCAN_STATUS"
    fi
    exit 97
    ;;
  *) exit 98 ;;
esac
`);
  writeFileSync(path.join(bin, "sha256sum"), `#!/usr/bin/env bash
set -euo pipefail
printf '%s  %s\\n' "$PRODEX_SCAN_TEST_SHA256" "$1"
`);
  chmodSync(path.join(bin, "docker"), 0o755);
  chmodSync(path.join(bin, "sha256sum"), 0o755);
  try {
    const result = spawnSync("bash", [scanner, scope], {
      encoding: "utf8",
      timeout: 10_000,
      env: {
        ...process.env,
        PATH: `${bin}${path.delimiter}${process.env.PATH ?? ""}`,
        CLAMAV_IMAGE: `docker.io/${imageDigest}`,
        PRODEX_RELEASE_SCAN_VERSION: overrides.version ?? "0.172.0",
        PRODEX_SCAN_TEST_CALLS: calls,
        PRODEX_SCAN_TEST_DIGEST: overrides.digest ?? imageDigest,
        PRODEX_SCAN_TEST_FRESHCLAM_STATUS: String(overrides.freshclamStatus ?? 0),
        PRODEX_SCAN_TEST_SCAN_STATUS: String(overrides.status ?? 1),
        PRODEX_SCAN_TEST_OUTPUT: overrides.output ?? finding,
        PRODEX_SCAN_TEST_SHA256: overrides.sha256 ?? reviewed172,
      },
    });
    assert.ifError(result.error);
    return { status: result.status, output: result.stdout + result.stderr, calls: readFileSync(calls, "utf8") };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const cases = [
  { name: "clean scan", status: 0, output: "", expected: 0 },
  { name: "first reviewed 0.171.2 build", version: "0.171.2", sha256: "10229478760ac554717473cf7771afb5f6c8fdbd83b66fb896a0e8cfd92bfc3f", expected: 0 },
  { name: "second reviewed 0.171.2 build", version: "0.171.2", sha256: "334d93fb3b399453221c61724c1112b8f6c069d5b57ea854505b1a0c0a120350", expected: 0 },
  { name: "reviewed 0.171.3 build", version: "0.171.3", sha256: "65630e75517dde4bc94e35c042894885776eafc17e7b107ed369a27c099fdf73", expected: 0 },
  { name: "reviewed 0.172.0 build", expected: 0 },
  { name: "different release version", version: "0.172.1", expected: 1 },
  { name: "new executable with an older reviewed version", version: "0.171.3", expected: 1 },
  { name: "older reviewed executable with the new version", sha256: "65630e75517dde4bc94e35c042894885776eafc17e7b107ed369a27c099fdf73", expected: 1 },
  { name: "different executable bytes", sha256: "0".repeat(64), expected: 1 },
  { name: "additional finding", output: `${finding}\n/scan/install.sh: Another.Signature FOUND\n`, expected: 1 },
  { name: "duplicate finding", output: `${finding}\n${finding}\n`, expected: 1 },
  { name: "different executable path", output: finding.replace("x86_64", "aarch64"), expected: 1 },
  { name: "scanner error", status: 2, expected: 2 },
  { name: "stale definitions", status: 2, output: "LibClamAV Error: virus database is older than the permitted age\n", expected: 2 },
  { name: "scanner warning with finding", output: `${finding}\nWARNING: incomplete scan\n`, expected: 1 },
  { name: "engine health directory", scope: "scan-health", expected: 1 },
  { name: "definition update error", freshclamStatus: 2, expected: 2, scanExpected: false },
  { name: "image digest mismatch", digest: "clamav/clamav@sha256:untrusted", expected: 1, scanExpected: false },
];

for (const entry of cases) {
  test(`release malware policy: ${entry.name}`, {
    skip: process.platform === "win32" ? "The production scanner runs in Linux release jobs" : false,
  }, () => {
    const result = runPolicy(entry);
    assert.equal(result.status, entry.expected, result.output);
    assert.equal(result.calls.includes("--entrypoint clamscan"), entry.scanExpected ?? true);
    if (entry.expected === 0 && entry.status !== 0) {
      assert.match(result.output, /Reviewed ClamAV false positive/);
    }
  });
}
