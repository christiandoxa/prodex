import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("release Mojo CI archive reuse rejects bad provenance and keeps a strict compile fallback", () => {
  const result = spawnSync(
    "python3",
    ["-m", "unittest", "discover", "-s", "scripts/ci", "-p", "reuse_mojo_ci_archive_test.py"],
    {
      encoding: "utf8",
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
      timeout: 20_000,
      maxBuffer: 2 * 1024 * 1024,
    },
  );
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stdout + "\n" + result.stderr);
  assert.match(result.stderr, /Ran 8 tests/);
});
