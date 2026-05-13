import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  budgetExceeded,
  evaluateBudget,
  formatDuration,
  parseArgs,
  parsePayload,
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
