import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

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
  assert.equal(allIds.size, 14);
  assert.deepEqual(firstIds.intersection(secondIds), new Set());
  assert.deepEqual(firstIds.union(secondIds), allIds);
});

test("auto-rotate CI shard index must be in range", () => {
  const result = dryRun("--shard-index", "2", "--shard-count", "2");
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--shard-index must be an integer between 0 and 1/);
});

test("Windows CI runs three test partitions with one cache writer", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const block = workflow.match(/\n  windows-workspace:\n([\s\S]*?)\n  macos-workspace:/)?.[1];
  assert.ok(block, "windows-workspace job missing");

  assert.match(block, /- suite: members/);
  assert.match(block, /- suite: root-0/);
  assert.match(block, /- suite: root-1/);
  assert.equal(block.match(/save_cache: true/g)?.length, 1);
  assert.equal(block.match(/save_cache: false/g)?.length, 2);
  assert.equal(block.match(/shell: bash/g)?.length, 2);
  assert.match(block, /--workspace --exclude prodex/);
  assert.match(
    block,
    /-p prodex --lib --bins --examples --test dashboard_control_plane --test enterprise_binaries --test internal_commands/,
  );
  assert.doesNotMatch(block, /-p prodex --all-features -- --test-threads/);
  assert.match(block, /--workspace --exclude prodex --all-features -- --test-threads=4/);
  assert.match(block, /--all-features --jobs 4 --shard-index/);
  assert.match(block, /--shard-index \$\{\{ matrix\.auto_rotate_shard \}\} --shard-count 2/);
});
