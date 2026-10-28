import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

const WINDOWS_TEST_SUITES = Object.freeze([
  "members-foundation",
  "members-runtime",
  "root-0",
  "root-1",
  "root-tests",
  "installer",
]);

function dryRun(...args) {
  return spawnSync(process.execPath, ["scripts/ci/auto-rotate-shards.mjs", "--dry-run", ...args], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
}

function shardIds(output) {
  return new Set(
    output
      .split(/\r?\n/)
      .filter((line) => line.startsWith("auto-rotate:"))
      .map((line) => line.split(":", 2)[1]),
  );
}

test("auto-rotate CI shards are disjoint and cover every group", () => {
  const full = dryRun();
  const first = dryRun("--shard-index", "0", "--shard-count", "2");
  const second = dryRun("--shard-index", "1", "--shard-count", "2");

  assert.equal(full.status, 0, full.stderr);
  assert.equal(first.status, 0, first.stderr);
  assert.equal(second.status, 0, second.stderr);

  const allIds = shardIds(full.stdout);
  const firstIds = shardIds(first.stdout);
  const secondIds = shardIds(second.stdout);
  assert.equal(allIds.size, 23);
  assert.deepEqual(firstIds.intersection(secondIds), new Set());
  assert.deepEqual(firstIds.union(secondIds), allIds);
});

test("auto-rotate CI shard index must be in range", () => {
  const result = dryRun("--shard-index", "2", "--shard-count", "2");
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--shard-index must be an integer between 0 and 1/);
});

test("Windows tests retain every lane while integration consumers share one build", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const job = (name) => workflow.match(new RegExp(`^  ${name}:\\n[\\s\\S]*?(?=^  [a-z][a-z0-9_-]*:|$(?![\\s\\S]))`, "gm"))?.[0];
  const workspace = job("windows-workspace");
  const consumer = job("windows-integration");
  const producer = job("windows-integration-build");
  const suites = (body) => [...body.matchAll(/^\s+- suite: (\S+)$/gm)].map((match) => match[1]);
  assert.deepEqual(new Set([...suites(workspace), ...suites(consumer)]), new Set(WINDOWS_TEST_SUITES));
  assert.equal(suites(workspace).length + suites(consumer).length, WINDOWS_TEST_SUITES.length);
  assert.equal(workspace.match(/save_cache: true/g)?.length, 1);
  assert.match(workspace, /-p prodex --lib --bins --examples --test internal_commands/);
  assert.match(workspace, /-p prodex --doc/);
  assert.match(workspace, /--workspace --exclude prodex --exclude prodex-app --exclude 'prodex-runtime-\*'/);
  assert.match(workspace, /-p 'prodex-runtime-\*' -- --test-threads/);
  assert.doesNotMatch(workspace, /auto-rotate-shards|Build Windows installer fixture binary/);
  assert.match(consumer, /needs: \[changes, windows-integration-build\]/);
  assert.match(consumer, /auto-rotate-shards\.mjs --jobs 4 --shard-index/);
  assert.match(consumer, /--shard-index \$\{\{ matrix\.auto_rotate_shard \}\} --shard-count 2/);
  assert.match(consumer, /integration-test-artifact\.mjs restore/);
  assert.match(consumer, /node --test scripts\/install\.test\.mjs/);
  assert.doesNotMatch(consumer, /cargo (?:test|build)|rust-cache@|sccache-action@/);
  assert.match(producer, /integration-test-artifact\.mjs pack/);
  assert.match(producer, /if-no-files-found: error/);
  const app = job("windows-prodex-app");
  assert.match(app, /native-test-artifact\.mjs run/);
  assert.doesNotMatch(app, /rust-cache@|sccache-action@/);
});
