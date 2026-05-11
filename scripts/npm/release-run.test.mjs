import test from "node:test";
import assert from "node:assert/strict";
import {
  isGhTimeoutError,
  parseArgs,
  pendingStepsForRun,
  releaseSteps,
  releaseSubject,
  selectSteps,
  shouldRequireCleanWorktree,
  shouldResolveGithubRepo,
} from "./release-run.mjs";

test("parseArgs defaults to the full release flow", () => {
  const args = parseArgs(["node", "release-run.mjs"]);
  assert.equal(args.branch, "main");
  assert.equal(args.dryRun, false);
  assert.deepEqual(args.steps, releaseSteps);
});

test("parseArgs supports dry-run resume and bounded step selection", () => {
  const args = parseArgs([
    "node",
    "release-run.mjs",
    "--dry-run",
    "--resume",
    "--version",
    "0.93.0",
    "--from",
    "watch-ci",
    "--to",
    "verify",
    "--poll-seconds",
    "1",
  ]);

  assert.equal(args.dryRun, true);
  assert.equal(args.resume, true);
  assert.equal(args.version, "0.93.0");
  assert.deepEqual(args.steps, ["watch-ci", "trigger-publish", "watch-publish", "verify"]);
  assert.equal(args.pollSeconds, 1);
});

test("selectSteps keeps --only steps in canonical order", () => {
  assert.deepEqual(
    selectSteps({ onlySteps: ["verify", "trigger-publish", "watch-publish"] }),
    ["trigger-publish", "watch-publish", "verify"],
  );
});

test("releaseSubject matches release guard subject exactly", () => {
  assert.equal(releaseSubject("0.93.0"), "chore(release): release 0.93.0");
});

test("pendingStepsForRun skips completed resume steps only", () => {
  assert.deepEqual(
    pendingStepsForRun(["bump", "sync", "test"], {
      resume: true,
      completed: { bump: "2026-05-11T00:00:00.000Z" },
    }),
    ["sync", "test"],
  );
  assert.deepEqual(
    pendingStepsForRun(["bump", "sync"], {
      resume: false,
      completed: { bump: "2026-05-11T00:00:00.000Z" },
    }),
    ["bump", "sync"],
  );
});

test("clean worktree is required only before pending mutating metadata steps", () => {
  assert.equal(shouldRequireCleanWorktree({ steps: ["bump"] }), true);
  assert.equal(shouldRequireCleanWorktree({ steps: ["sync"] }), true);
  assert.equal(shouldRequireCleanWorktree({ steps: ["commit", "push"] }), false);
  assert.equal(shouldRequireCleanWorktree({ steps: ["bump"], dryRun: true }), false);
  assert.equal(
    shouldRequireCleanWorktree({
      steps: ["bump", "sync", "commit"],
      resume: true,
      completed: { bump: "2026-05-11T00:00:00.000Z", sync: "2026-05-11T00:00:01.000Z" },
    }),
    false,
  );
});

test("github repo is resolved only for pending remote workflow steps", () => {
  assert.equal(shouldResolveGithubRepo({ steps: ["bump", "sync", "test", "commit"] }), false);
  assert.equal(shouldResolveGithubRepo({ steps: ["push"] }), false);
  assert.equal(shouldResolveGithubRepo({ steps: ["watch-ci"] }), true);
  assert.equal(shouldResolveGithubRepo({ steps: ["trigger-publish"] }), true);
  assert.equal(shouldResolveGithubRepo({ steps: ["watch-publish"] }), true);
  assert.equal(shouldResolveGithubRepo({ steps: ["verify"] }), true);
  assert.equal(shouldResolveGithubRepo({ steps: ["verify"], skipVerifyGithub: true }), false);
  assert.equal(
    shouldResolveGithubRepo({
      steps: ["watch-ci", "trigger-publish"],
      resume: true,
      completed: {
        "watch-ci": "2026-05-11T00:00:00.000Z",
        "trigger-publish": "2026-05-11T00:00:01.000Z",
      },
    }),
    false,
  );
});

test("parseArgs rejects invalid versions and unknown steps", () => {
  assert.throws(
    () => parseArgs(["node", "release-run.mjs", "--version", "not-a-version"]),
    /invalid --version/,
  );
  assert.throws(
    () => parseArgs(["node", "release-run.mjs", "--from", "publish"]),
    /unknown --from step/,
  );
});

test("gh timeout classifier matches transient API failures only", () => {
  assert.equal(isGhTimeoutError("Post https://api.github.com: context deadline exceeded"), true);
  assert.equal(isGhTimeoutError("net/http: TLS handshake timeout"), true);
  assert.equal(isGhTimeoutError("HTTP 504 gateway timeout"), true);
  assert.equal(isGhTimeoutError("HTTP 401 bad credentials"), false);
  assert.equal(isGhTimeoutError("validation failed: ref is required"), false);
});
