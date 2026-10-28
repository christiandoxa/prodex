import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { runChecked } from "../lib/checked-subprocess.mjs";
import { assertReleaseAdvance, mergedVersionTagsInReleaseOrder } from "./release-lineage.mjs";

const legacy = { tag: "0.439.5", commit: "ac9aad08f25d6dc8cda96463ce2045b40f35f510" };
test("only the exact reviewed cutover authorizes decreasing release numbers", () => {
  assert.doesNotThrow(() => assertReleaseAdvance("0.165.0", legacy, "rust-v0.165.0"));
  for (const [version, before, upstream] of [
    ["0.164.0", legacy, "rust-v0.165.0"],
    ["0.165.1", legacy, "rust-v0.165.0"],
    ["0.165.0", { ...legacy, tag: "0.439.4" }, "rust-v0.165.0"],
    ["0.165.0", { ...legacy, commit: "a".repeat(40) }, "rust-v0.165.0"],
    ["0.165.0", legacy, "rust-v0.164.0"],
    ["0.165.0", { tag: "0.165.0", commit: "b".repeat(40) }, "rust-v0.165.0"],
    ["0.164.9", { tag: "0.165.0", commit: "b".repeat(40) }, "rust-v0.165.0"],
  ]) assert.throws(() => assertReleaseAdvance(version, before, upstream));
  assert.doesNotThrow(() => assertReleaseAdvance("0.165.1", { tag: "0.165.0" }, "rust-v0.165.0"));
  assert.doesNotThrow(() => assertReleaseAdvance("0.166.0", { tag: "0.165.1" }, "rust-v0.166.0"));
  for (const bad of ["0.0165.0", "v0.165.0", "0.165.0-rc.1", "0.165.0+build", "0.165.0\n", ""]) {
    assert.throws(() => assertReleaseAdvance(bad, null, "rust-v0.165.0"));
  }
});

test("release ordering is topological, handles annotated tags, and excludes unmerged tags", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-release-lineage-"));
  const git = (args) => runChecked("git", args, { cwd: root }).stdout.trim();
  try {
    git(["init", "-q"]); git(["config", "user.name", "Fixture"]);
    git(["config", "user.email", "fixture@example.invalid"]);
    assert.deepEqual(mergedVersionTagsInReleaseOrder(root), []);
  } catch (error) {
    // An unborn branch has no valid revision; create history before testing it.
    assert.match(error.message, /HEAD|valid/u);
  }
  try {
    git(["commit", "--allow-empty", "-m", "initial"]);
    assert.deepEqual(mergedVersionTagsInReleaseOrder(root), []);
    git(["tag", "0.439.5"]); const old = git(["rev-parse", "HEAD"]);
    git(["commit", "--allow-empty", "-m", "aligned"]);
    git(["tag", "-a", "0.165.0", "-m", "aligned"]); const aligned = git(["rev-parse", "HEAD"]);
    git(["checkout", "-q", "-b", "unrelated", old]);
    git(["commit", "--allow-empty", "-m", "unmerged"]); git(["tag", "9.0.0"]);
    assert.deepEqual(mergedVersionTagsInReleaseOrder(root, aligned), [
      { tag: "0.439.5", commit: old }, { tag: "0.165.0", commit: aligned },
    ]);
  } finally { await fs.rm(root, { recursive: true, force: true }); }
});
