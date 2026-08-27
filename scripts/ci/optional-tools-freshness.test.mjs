import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import test from "node:test";

test("optional-tool freshness checker validates normalization and drift rules", () => {
  const output = execFileSync(
    process.execPath,
    ["scripts/ci/optional-tools-freshness.mjs", "--self-test"],
    { encoding: "utf8" },
  );
  assert.match(output, /optional-tools-freshness: self-test ok/u);
});
