import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  budgetExceeded,
  calibrateRuntimeStressWeightHints,
  evaluateBudget,
  formatDuration,
  formatRuntimeStressWeightHints,
  parseRuntimeStressTestListText,
  parseArgs,
  parsePayload,
  replaceRuntimeStressWeightHintsBlock,
  summarize,
} from "./github-job-durations.mjs";

const scriptPath = new URL("./github-job-durations.mjs", import.meta.url).pathname;
const now = new Date("2026-05-13T00:10:00Z");

test("parseArgs accepts duration budgets", () => {
  assert.deepEqual(
    parseArgs([
      "node",
      "scripts/ci/github-job-durations.mjs",
      "--limit",
      "5",
      "--exclude-name",
      "CI duration telemetry",
      "--max-wall-minutes",
      "6.5",
      "--max-runner-minutes",
      "60",
    ]),
    {
      excludeNames: ["CI duration telemetry"],
      json: false,
      limit: 5,
      maxWallMs: 390000,
      maxRunnerMs: 3600000,
    },
  );
});

test("summarize computes wall, runner, states, and longest jobs", () => {
  const summary = summarize(
    {
      jobs: [
        {
          name: "fmt",
          status: "completed",
          conclusion: "success",
          startedAt: "2026-05-13T00:00:00Z",
          completedAt: "2026-05-13T00:03:00Z",
        },
        {
          name: "windows release",
          status: "completed",
          conclusion: "success",
          startedAt: "2026-05-13T00:01:00Z",
          completedAt: "2026-05-13T00:05:00Z",
        },
        {
          name: "CI duration telemetry",
          status: "completed",
          conclusion: "success",
          startedAt: "2026-05-13T00:05:00Z",
          completedAt: "2026-05-13T00:06:00Z",
        },
      ],
    },
    {
      excludeNames: ["CI duration telemetry"],
      limit: 2,
    },
    now,
  );

  assert.equal(summary.totalJobs, 2);
  assert.equal(summary.wall.durationMs, 300000);
  assert.equal(summary.runnerMs, 420000);
  assert.deepEqual(summary.states, { success: 2 });
  assert.deepEqual(
    summary.longestJobs.map((job) => job.name),
    ["windows release", "fmt"],
  );
});

test("budget marks exceeded runner duration without failing wall duration", () => {
  const summary = summarize(
    {
      jobs: [
        {
          name: "fast",
          status: "completed",
          conclusion: "success",
          startedAt: "2026-05-13T00:00:00Z",
          completedAt: "2026-05-13T00:02:00Z",
        },
        {
          name: "slow",
          status: "completed",
          conclusion: "success",
          startedAt: "2026-05-13T00:00:00Z",
          completedAt: "2026-05-13T00:04:00Z",
        },
      ],
    },
    {
      excludeNames: [],
      limit: 10,
      maxWallMs: 300000,
      maxRunnerMs: 300000,
    },
    now,
  );

  assert.equal(summary.budget.status, "exceeded");
  assert.equal(summary.budget.checks[0].status, "ok");
  assert.equal(summary.budget.checks[1].status, "exceeded");
  assert.equal(summary.budget.checks[1].overMs, 60000);
  assert.equal(budgetExceeded(summary), true);
});

test("budget reports unavailable wall timing when jobs are untimed", () => {
  const summary = summarize(
    {
      jobs: [{ name: "queued", status: "queued", conclusion: null }],
    },
    {
      excludeNames: [],
      limit: 10,
      maxWallMs: 300000,
    },
    now,
  );

  assert.equal(summary.wall, null);
  assert.equal(summary.budget.status, "unavailable");
  assert.equal(summary.budget.checks[0].actualMs, null);
  assert.equal(budgetExceeded(summary), false);
});

test("evaluateBudget returns null when no budget is configured", () => {
  assert.equal(evaluateBudget({ wall: null, runnerMs: 0 }, {}), null);
});

test("parsePayload accepts object or raw job array", () => {
  assert.deepEqual(parsePayload(JSON.stringify([{ name: "fmt" }])), { jobs: [{ name: "fmt" }] });
  assert.deepEqual(parsePayload(JSON.stringify({ jobs: [{ name: "test" }] })), { jobs: [{ name: "test" }] });
});

test("runtime stress calibration derives deterministic hint updates from successful broad shard jobs", () => {
  const calibration = calibrateRuntimeStressWeightHints({
    defaultWeightSeconds: 1,
    jobs: [
      {
        name: "Runtime stress (weighted broad shard 1 of 2)",
        state: "success",
        durationMs: 120000,
      },
      {
        name: "Runtime stress (weighted broad shard 2 of 2)",
        state: "success",
        durationMs: 60000,
      },
    ],
    skipTests: [],
    testNames: [
      "main_internal_tests::runtime_proxy_demo::slow_a",
      "main_internal_tests::runtime_proxy_demo::slow_b",
      "main_internal_tests::runtime_proxy_demo::tiny_a",
      "main_internal_tests::runtime_proxy_demo::tiny_b",
    ],
    weightHints: [
      { name: "slow_a", weightSeconds: 4 },
      { name: "slow_b", weightSeconds: 2 },
    ],
  });

  assert.deepEqual(calibration.shards, [
    {
      estimatedWeightSeconds: 4,
      index: 0,
      observedSeconds: 120,
      ratio: 1.333,
      targetWeightSeconds: 5.333,
      testCount: 1,
    },
    {
      estimatedWeightSeconds: 4,
      index: 1,
      observedSeconds: 60,
      ratio: 0.667,
      targetWeightSeconds: 2.667,
      testCount: 3,
    },
  ]);
  assert.deepEqual(
    calibration.suggestions.map((suggestion) => ({
      action: suggestion.action,
      current: suggestion.currentWeightSeconds,
      suggested: suggestion.suggestedWeightSeconds,
      selector: `${suggestion.selectorType}:${suggestion.selector}`,
    })),
    [
      { action: "raise", current: 4, suggested: 5, selector: "name:slow_a" },
      { action: "lower", current: 2, suggested: 1, selector: "name:slow_b" },
    ],
  );
  assert.equal(calibration.changedHints, 2);
  assert.deepEqual(calibration.suggestedHints, [
    { name: "slow_a", weightSeconds: 5 },
    { name: "slow_b", weightSeconds: 1 },
  ]);
});

test("runtime stress calibration refuses incomplete successful shard telemetry", () => {
  assert.throws(
    () =>
      calibrateRuntimeStressWeightHints({
        jobs: [
          {
            name: "Runtime stress (weighted broad shard 1 of 2)",
            state: "success",
            durationMs: 120000,
          },
          {
            name: "Runtime stress (weighted broad shard 2 of 2)",
            state: "failure",
            durationMs: 60000,
          },
        ],
        testNames: ["main_internal_tests::runtime_proxy_demo::slow_a"],
        weightHints: [{ name: "slow_a", weightSeconds: 4 }],
      }),
    /requires successful completed telemetry/,
  );
});

test("runtime stress hint formatting and replacement are stable", () => {
  const hints = [
    { filter: "main_internal_tests::runtime_proxy_demo::", weightSeconds: 3 },
    { name: "specific_runtime_test", weightSeconds: 5 },
  ];
  const formatted = formatRuntimeStressWeightHints(hints);

  assert.equal(
    formatted,
    [
      "export const RUNTIME_STRESS_WEIGHT_HINTS = Object.freeze([",
      "  {",
      '    filter: "main_internal_tests::runtime_proxy_demo::",',
      "    weightSeconds: 3,",
      "  },",
      "  {",
      '    name: "specific_runtime_test",',
      "    weightSeconds: 5,",
      "  },",
      "]);",
    ].join("\n"),
  );
  assert.equal(
    replaceRuntimeStressWeightHintsBlock(
      [
        "before",
        "export const RUNTIME_STRESS_WEIGHT_HINTS = Object.freeze([",
        "  {",
        '    name: "old",',
        "    weightSeconds: 1,",
        "  },",
        "]);",
        "after",
      ].join("\n"),
      hints,
    ),
    ["before", formatted, "after"].join("\n"),
  );
});

test("runtime stress test list parser accepts JSON arrays and cargo list output", () => {
  assert.deepEqual(parseRuntimeStressTestListText('["main_internal_tests::runtime_proxy_demo::a"]'), [
    "main_internal_tests::runtime_proxy_demo::a",
  ]);
  assert.deepEqual(
    parseRuntimeStressTestListText(
      [
        "main_internal_tests::runtime_proxy_demo::a: test",
        "main_internal_tests::other::b: test",
        "doc_tests::ignored: test",
      ].join("\n"),
    ),
    ["main_internal_tests::runtime_proxy_demo::a", "main_internal_tests::other::b"],
  );
});

test("formatDuration uses compact human units", () => {
  assert.equal(formatDuration(59000), "59s");
  assert.equal(formatDuration(61000), "1m 1s");
  assert.equal(formatDuration(3661000), "1h 1m 1s");
});

test("CLI exits nonzero when duration budget is exceeded", () => {
  const result = spawnSync(
    process.execPath,
    [scriptPath, "--run-id", "123", "--max-runner-minutes", "3", "--json"],
    {
      encoding: "utf8",
      input: JSON.stringify({
        jobs: [
          {
            name: "windows release",
            status: "completed",
            conclusion: "success",
            startedAt: "2026-05-13T00:00:00Z",
            completedAt: "2026-05-13T00:04:00Z",
          },
        ],
      }),
    },
  );

  assert.equal(result.status, 1);
  const summary = JSON.parse(result.stdout);
  assert.equal(summary.runId, "123");
  assert.equal(summary.budget.status, "exceeded");
  assert.equal(summary.budget.checks[0].metric, "runner");
});

test("CLI runtime stress calibration reads local telemetry and test list without gh", () => {
  const tempDir = mkdtempSync(join(tmpdir(), "prodex-runtime-calibration-"));
  const testListPath = join(tempDir, "runtime-tests.json");
  writeFileSync(
    testListPath,
    JSON.stringify([
      "main_internal_tests::runtime_proxy_selection_and_pressure::state::runtime_state_save_scheduler_persists_latest_snapshot",
      "main_internal_tests::runtime_proxy_claude_and_anthropic::request_translation::translate_runtime_anthropic_messages_request_maps_tools_and_tool_results",
    ]),
  );

  try {
    const result = spawnSync(
      process.execPath,
      [
        scriptPath,
        "--runtime-stress-calibration",
        "--runtime-stress-test-list",
        testListPath,
        "--json",
      ],
      {
        encoding: "utf8",
        env: { ...process.env, PATH: "" },
        input: JSON.stringify({
          jobs: [
            {
              name: "Runtime stress (weighted broad shard 1 of 2)",
              status: "completed",
              conclusion: "success",
              startedAt: "2026-05-13T00:00:00Z",
              completedAt: "2026-05-13T00:02:00Z",
            },
            {
              name: "Runtime stress (weighted broad shard 2 of 2)",
              status: "completed",
              conclusion: "success",
              startedAt: "2026-05-13T00:00:00Z",
              completedAt: "2026-05-13T00:01:00Z",
            },
          ],
        }),
      },
    );

    assert.equal(result.status, 0, result.stderr);
    const summary = JSON.parse(result.stdout);
    assert.equal(summary.runtimeStressCalibration.shardCount, 2);
    assert.equal(summary.runtimeStressCalibration.runnableTestCount, 2);
  } finally {
    rmSync(tempDir, { force: true, recursive: true });
  }
});

test("CLI runtime stress calibration refuses live GitHub fetch", () => {
  const tempDir = mkdtempSync(join(tmpdir(), "prodex-runtime-calibration-"));
  const testListPath = join(tempDir, "runtime-tests.json");
  writeFileSync(testListPath, JSON.stringify(["main_internal_tests::runtime_proxy_demo::slow_a"]));

  try {
    const result = spawnSync(
      process.execPath,
      [
        scriptPath,
        "--runtime-stress-calibration",
        "--runtime-stress-test-list",
        testListPath,
        "--run-id",
        "123",
      ],
      {
        encoding: "utf8",
        env: { ...process.env, PATH: "" },
        input: "",
      },
    );

    assert.equal(result.status, 1);
    assert.match(result.stderr, /refusing live GitHub fetch/);
  } finally {
    rmSync(tempDir, { force: true, recursive: true });
  }
});
