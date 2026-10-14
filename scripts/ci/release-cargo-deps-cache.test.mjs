import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("release Rust dependency cache key excludes workspace versions but tracks external checksums", () => {
  const result = spawnSync(
    "python3",
    ["-m", "unittest", "discover", "-s", "scripts/ci", "-p", "release_cargo_deps_cache_key_test.py"],
    {
      encoding: "utf8",
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
      timeout: 15_000,
      maxBuffer: 2 * 1024 * 1024,
    },
  );
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stdout + "\n" + result.stderr);
  assert.match(result.stderr, /Ran 5 tests/);
});
