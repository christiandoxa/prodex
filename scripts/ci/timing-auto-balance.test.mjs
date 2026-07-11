import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  formatRuntimeStressHints,
  parseArgs,
  parseTimingTelemetry,
  suggestedRuntimeStressHints,
  suggestedWeights,
  summarizeTimings,
} from "./timing-auto-balance.mjs";

const scriptPath = new URL("./timing-auto-balance.mjs", import.meta.url).pathname;

test("parseArgs accepts output modes and file inputs", () => {
  assert.deepEqual(
    parseArgs([
      "node",
      "scripts/ci/timing-auto-balance.mjs",
      "--file",
      "fast.log",
      "-f",
      "serial.log",
      "--limit",
      "5",
      "--weights-json",
    ]),
    {
      files: ["fast.log", "serial.log"],
      json: false,
      limit: 5,
      runtimeStressHints: false,
      weightsJson: true,
    },
  );
});

test("parseTimingTelemetry reads runner lines and raw JSON arrays", () => {
  const records = parseTimingTelemetry(
    [
      'test-fast: timings-json [{"label":"crate:a","elapsedMs":1000,"attempts":1}]',
      '[{"label":"crate:b","elapsedMs":2000}]',
    ].join("\n"),
  );

  assert.deepEqual(records, [
    { attempts: 1, elapsedMs: 1000, label: "crate:a" },
    { attempts: 1, elapsedMs: 2000, label: "crate:b" },
  ]);
});

test("summarizeTimings aggregates slow labels and generic weights", () => {
  const summary = summarizeTimings([
    { attempts: 1, elapsedMs: 1000, label: "fast" },
    { attempts: 2, elapsedMs: 3000, label: "slow" },
    { attempts: 1, elapsedMs: 5000, label: "slow" },
  ]);

  assert.equal(summary.recordCount, 3);
  assert.deepEqual(
    summary.labels.map((entry) => ({
      attempts: entry.attempts,
      averageMs: entry.averageMs,
      label: entry.label,
      runs: entry.runs,
      weightSeconds: entry.weightSeconds,
    })),
    [
      { attempts: 3, averageMs: 4000, label: "slow", runs: 2, weightSeconds: 4 },
      { attempts: 1, averageMs: 1000, label: "fast", runs: 1, weightSeconds: 1 },
    ],
  );
  assert.deepEqual(suggestedWeights(summary)[0], {
    averageMs: 4000,
    label: "slow",
    maxMs: 5000,
    runs: 2,
    weightSeconds: 4,
  });
});

test("suggestedRuntimeStressHints maps serial stress labels to runtime shard hints", () => {
  const summary = summarizeTimings([
    {
      attempts: 1,
      elapsedMs: 4400,
      label: "serial:stress:main_internal_tests::runtime_proxy_demo::slow_a",
    },
    {
      attempts: 1,
      elapsedMs: 2600,
      label: "serial:continuation:1:main_internal_tests::runtime_proxy_demo::continuation_a",
    },
    {
      attempts: 1,
      elapsedMs: 3200,
      label: "serial:continuation:2:main_internal_tests::runtime_proxy_demo::continuation_a",
    },
  ]);

  const hints = suggestedRuntimeStressHints(summary);
  assert.deepEqual(
    hints.map((hint) => ({
      name: hint.name,
      runs: hint.runs,
      weightSeconds: hint.weightSeconds,
    })),
    [
      { name: "slow_a", runs: 1, weightSeconds: 4 },
      { name: "continuation_a", runs: 2, weightSeconds: 3 },
    ],
  );
  assert.equal(
    formatRuntimeStressHints(hints),
    [
      "export const RUNTIME_STRESS_WEIGHT_HINTS = Object.freeze([",
      "  {",
      '    name: "slow_a",',
      "    weightSeconds: 4,",
      "  },",
      "  {",
      '    name: "continuation_a",',
      "    weightSeconds: 3,",
      "  },",
      "]);",
    ].join("\n"),
  );
});

test("CLI emits weights JSON from stdin telemetry", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--weights-json"], {
    encoding: "utf8",
    input: 'test-serial: timings-json [{"label":"serial:stress:main_internal_tests::runtime_proxy_demo::slow_a","elapsedMs":2200,"attempts":1}]\n',
  });

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), [
    {
      averageMs: 2200,
      label: "serial:stress:main_internal_tests::runtime_proxy_demo::slow_a",
      maxMs: 2200,
      runs: 1,
      weightSeconds: 2,
    },
  ]);
});
