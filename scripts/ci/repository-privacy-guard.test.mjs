import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { assertSanitizedHistory, selfTest } from "./repository-privacy-guard.mjs";

test("privacy content checks preserve reserved fixtures and vendor attribution", () => {
  selfTest();
});

test("privacy history gate rejects an old-history merge even after its files are removed", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "prodex-privacy-history-"));
  const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
  try {
    git("init", "-b", "legacy");
    git("config", "user.name", "Synthetic Fixture");
    git("config", "user.email", "fixture@example.invalid");
    git("config", "commit.gpgsign", "false");
    fs.writeFileSync(path.join(root, "legacy.txt"), "synthetic legacy data\n");
    git("add", "legacy.txt");
    git("commit", "-m", "legacy fixture");
    git("checkout", "--orphan", "clean");
    git("rm", "-f", "legacy.txt");
    fs.writeFileSync(path.join(root, "clean.txt"), "synthetic current data\n");
    git("add", "clean.txt");
    git("commit", "-m", "clean fixture");
    const approvedRoot = git("rev-parse", "HEAD");
    assert.doesNotThrow(() => assertSanitizedHistory(root, approvedRoot));
    git("merge", "legacy", "--allow-unrelated-histories", "--no-edit");
    git("rm", "-f", "legacy.txt");
    git("commit", "-m", "remove legacy file from tip");
    assert.deepEqual(git("ls-files").split("\n"), ["clean.txt"]);
    assert.throws(() => assertSanitizedHistory(root, approvedRoot), /unreviewed root/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
