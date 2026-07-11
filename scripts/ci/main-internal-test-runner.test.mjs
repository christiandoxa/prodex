import assert from "node:assert/strict";
import test from "node:test";
import {
  cargoFeatureArgs,
  cargoIntegrationTestFilterStep,
  formatStepTimingSummary,
  sortedStepTimings,
} from "./main-internal-test-runner.mjs";

test("timing summary sorts slowest steps and can emit JSON", () => {
  const timings = [
    { label: "fast", elapsedMs: 900, attempts: 1 },
    { label: "slow", elapsedMs: 61000, attempts: 2 },
    { label: "medium", elapsedMs: 1500, attempts: 1 },
  ];

  assert.deepEqual(
    sortedStepTimings(timings).map((timing) => timing.label),
    ["slow", "medium", "fast"],
  );
  assert.equal(
    formatStepTimingSummary(timings, { label: "demo", limit: 2, json: true }),
    [
      "demo: 3 completed step(s), summed runtime 1m 03s, slowest 2:",
      "  1. slow: 1m 01s (61000 ms)",
      "  2. medium: 2s (1500 ms)",
      'demo: timings-json [{"label":"slow","elapsedMs":61000,"attempts":2},{"label":"medium","elapsedMs":1500,"attempts":1},{"label":"fast","elapsedMs":900,"attempts":1}]',
      "",
    ].join("\n"),
  );
});

test("cargo helpers include all-features before filters and harness args", () => {
  assert.deepEqual(cargoFeatureArgs({ allFeatures: true }), ["--all-features"]);
  assert.deepEqual(cargoFeatureArgs({ allFeatures: false }), []);
  assert.deepEqual(
    cargoIntegrationTestFilterStep(
      "auto",
      "auto_rotate",
      "run::example",
      ["--test-threads=1"],
      { allFeatures: true },
    ),
    {
      args: [
        "test",
        "--test",
        "auto_rotate",
        "--all-features",
        "run::example",
        "--",
        "--test-threads=1",
      ],
      command: "cargo",
      failOnZeroTests: true,
      label: "auto",
    },
  );
});
