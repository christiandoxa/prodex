import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const SCRIPT_PATH = new URL("./changelog.mjs", import.meta.url).pathname;

async function git(root, args) {
  return execFileAsync("git", args, { cwd: root });
}

async function writeFile(root, relativePath, contents) {
  const filePath = path.join(root, relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, contents);
}

async function commit(root, subject) {
  await git(root, ["add", "."]);
  await git(root, ["commit", "-m", subject]);
}

test("changelog omits internal maintenance commits", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-changelog-"));
  try {
    await git(root, ["init", "-q"]);
    await git(root, ["config", "user.name", "Prodex Fixture"]);
    await git(root, ["config", "user.email", "fixtures@example.invalid"]);
    await writeFile(root, "Cargo.toml", '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2024"\n');
    await writeFile(root, "CHANGELOG.md", "# Changelog\n");
    await writeFile(root, "src/lib.rs", "pub fn fixture() {}\n");
    await commit(root, "chore(release): release 0.1.0");
    await git(root, ["tag", "0.1.0"]);

    await writeFile(root, "src/lib.rs", "pub fn fixture() {}\npub fn feature() {}\n");
    await commit(root, "feat: add visible feature");
    await writeFile(root, "src/lib.rs", "pub fn fixture() {}\npub fn feature() {}\npub fn fix() {}\n");
    await commit(root, "fix(runtime): repair visible stream");
    await writeFile(root, "Cargo.toml", '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2024"\n[dependencies]\ntokio = "1"\n');
    await commit(root, "chore(deps): bump tokio");
    await writeFile(root, ".github/workflows/ci.yml", "name: ci\n");
    await commit(root, "ci: update workflow");
    await writeFile(root, "tests/smoke.rs", "#[test]\nfn smoke() {}\n");
    await commit(root, "test: add smoke coverage");
    await writeFile(root, "src/lib.rs", "pub fn fixture() {}\npub fn feature() {}\npub fn fix() {}\npub mod split {}\n");
    await commit(root, "refactor: split runtime modules");
    await writeFile(root, "scripts/guard.mjs", "export const guard = true;\n");
    await commit(root, "chore: tighten guardrails");

    const { stdout } = await execFileAsync(process.execPath, [SCRIPT_PATH, "--print", "--releases", "1"], {
      cwd: root,
      env: { ...process.env, PRODEX_REPO_ROOT: root },
    });

    assert.match(stdout, /Add visible feature/);
    assert.match(stdout, /Repair visible stream/);
    assert.match(stdout, /Bump tokio/);
    assert.doesNotMatch(stdout, /Update workflow/);
    assert.doesNotMatch(stdout, /Add smoke coverage/);
    assert.doesNotMatch(stdout, /Split runtime modules/);
    assert.doesNotMatch(stdout, /Tighten guardrails/);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
