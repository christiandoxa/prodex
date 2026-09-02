import assert from "node:assert/strict";
import test from "node:test";
import {
  cargoFeatureArgs,
  cargoIntegrationTestFilterStep,
  formatStepTimingSummary,
  runStep,
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
        "--locked",
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

test("targeted steps reject zero tests across output chunks", async () => {
  await assert.rejects(
    runStep({
      label: "fragmented-zero-test",
      command: process.execPath,
      args: [
        "-e",
        "process.stdout.write('running '); setTimeout(() => process.stdout.write('0 tests\\n'), 10);",
      ],
      failOnZeroTests: true,
    }),
    /fragmented-zero-test matched no tests/,
  );
});

test("targeted steps allow an auxiliary zero-test harness after positive tests", async () => {
  const result = await runStep({
    label: "positive-then-zero-test",
    command: process.execPath,
    args: ["-e", "process.stdout.write('running 2 tests\\n'); process.stdout.write('running 0 tests\\n');"],
    failOnZeroTests: true,
  });
  assert.equal(result.label, "positive-then-zero-test");
});

test("timed-out steps terminate their process group and report the timeout", async () => {
  await assert.rejects(
    runStep({
      label: "timed-out-step",
      command: process.execPath,
      args: ["-e", "setInterval(() => {}, 1000);"],
      timeoutMs: 200,
    }),
    /timed-out-step timed out after 200 ms/,
  );
});
