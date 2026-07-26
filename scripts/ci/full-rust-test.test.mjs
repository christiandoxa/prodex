import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("full Rust runner includes the explicitly disabled prodex-app lib target", () => {
  const result = spawnSync(
    process.execPath,
    ["scripts/ci/full-rust-test.mjs", "--dry-run", "--no-prebuild"],
    { cwd: process.cwd(), encoding: "utf8" },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(
    result.stdout,
    /prodex-app:all-lib-tests-serial: cargo test -q -p prodex-app --lib --all-features -- --test-threads=1/,
  );
});
