import assert from "node:assert/strict";
import test from "node:test";
import { collectMarkers, failedJobsFromPayload, parseArgs } from "./github-failure-summary.mjs";

test("keeps completed non-success jobs in failure summary", () => {
  const jobs = failedJobsFromPayload({
    jobs: [
      { name: "fmt", status: "completed", conclusion: "success" },
      { name: "runtime proxy", status: "completed", conclusion: "failure" },
      { name: "dependent shard", status: "completed", conclusion: "skipped" },
    ],
  });

  assert.deepEqual(
    jobs.map((job) => job.name),
    ["runtime proxy", "dependent shard"],
  );
});

test("includes active jobs with failed steps from gh JSON", () => {
  const jobs = failedJobsFromPayload({
    jobs: [
      {
        name: "runtime stress",
        status: "in_progress",
        conclusion: null,
        steps: [
          { number: 1, name: "Check out", status: "completed", conclusion: "success" },
          { number: 2, name: "Run runtime stress", status: "completed", conclusion: "failure" },
          { number: 3, name: "Upload diagnostics", status: "in_progress", conclusion: null },
        ],
      },
    ],
  });

  assert.equal(jobs.length, 1);
  assert.equal(jobs[0].name, "runtime stress");
});

test("does not report active jobs that have no failed steps yet", () => {
  const jobs = failedJobsFromPayload({
    jobs: [
      {
        name: "runtime stress",
        status: "in_progress",
        conclusion: null,
        steps: [
          { number: 1, name: "Check out", status: "completed", conclusion: "success" },
          { number: 2, name: "Run runtime stress", status: "in_progress", conclusion: null },
        ],
      },
      {
        name: "queued shard",
        status: "queued",
        conclusion: null,
        steps: [],
      },
    ],
  });

  assert.deepEqual(jobs, []);
});

test("collects runtime marker counts in stable order", () => {
  const markers = collectMarkers(
    [
      "runtime_proxy_lane_limit_reached lane=responses",
      "profile_health route=responses profile=a",
      "runtime_proxy_lane_limit_reached lane=responses",
      "first_local_chunk",
    ].join("\n"),
  );

  assert.deepEqual(markers.slice(0, 3), [
    ["runtime_proxy_lane_limit_reached", 2],
    ["first_local_chunk", 1],
    ["profile_health", 1],
  ]);
});

test("parseArgs accepts run view selectors", () => {
  assert.deepEqual(
    parseArgs([
      "node",
      "scripts/ci/github-failure-summary.mjs",
      "--run-id",
      "123",
      "--repo",
      "owner/repo",
      "--attempt",
      "2",
    ]),
    {
      runId: "123",
      repo: "owner/repo",
      attempt: "2",
    },
  );
});
