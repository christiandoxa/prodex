import assert from "node:assert/strict";
import test from "node:test";
import { FUZZ_SMOKE_JOBS, FUZZ_TARGETS, fuzzSmokeSteps } from "./fuzz-smoke.mjs";

test("fuzz smoke keeps every target and runs with bounded parallelism", () => {
  const steps = fuzzSmokeSteps();

  assert.equal(FUZZ_SMOKE_JOBS, 4);
  assert.equal(FUZZ_TARGETS.length, 6);
  assert.equal(new Set(FUZZ_TARGETS).size, FUZZ_TARGETS.length);
  assert.deepEqual(
    steps.map((step) => step.args[3]),
    [...FUZZ_TARGETS],
  );
  for (const step of steps) {
    assert.equal(step.command, "cargo");
    assert.deepEqual(step.args.slice(0, 3), ["+nightly-2026-07-11", "fuzz", "run"]);
    assert.deepEqual(step.args.slice(-4), [
      "--",
      "-max_total_time=10",
      "-timeout=5",
      "-rss_limit_mb=2048",
    ]);
  }
});
