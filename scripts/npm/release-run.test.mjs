import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import {
  cargoPublishCommandArgs,
  cargoPublishOrderFromMetadata,
  cargoPublishPlanLines,
  isGhTimeoutError,
  parseArgs,
  pendingStepsForRun,
  releaseSteps,
  releaseSubject,
  selectSteps,
  shouldRequireCleanWorktree,
  shouldResolveGithubRepo,
} from "./release-run.mjs";

const execFileAsync = promisify(execFile);
const SCRIPT_PATH = new URL("./release-run.mjs", import.meta.url).pathname;

async function git(root, args) {
  return execFileAsync("git", args, { cwd: root });
}

async function writeFile(root, relativePath, contents) {
  const filePath = path.join(root, relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, contents);
}

async function pathExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function createReleaseRunFixture() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-release-run-"));
  await git(root, ["init", "-q"]);
  await git(root, ["config", "user.name", "Prodex Fixture"]);
  await git(root, ["config", "user.email", "fixtures@example.invalid"]);
  await git(root, ["remote", "add", "origin", "https://github.com/example/prodex.git"]);
  await writeFile(root, "Cargo.toml", '[package]\nname = "prodex-fixture"\nversion = "0.1.0"\nedition = "2024"\n');
  await fs.mkdir(path.join(root, "crates"), { recursive: true });
  await git(root, ["add", "."]);
  await git(root, ["commit", "-m", "chore: seed fixture"]);
  return root;
}

async function runReleaseRun(root, args) {
  return execFileAsync(process.execPath, [SCRIPT_PATH, ...args], {
    cwd: root,
    env: { ...process.env, PRODEX_REPO_ROOT: root },
    maxBuffer: 4 * 1024 * 1024,
  });
}

function assertIncludesInOrder(contents, expected) {
  let cursor = -1;
  for (const needle of expected) {
    const index = contents.indexOf(needle, cursor + 1);
    assert.notEqual(index, -1, `missing after index ${cursor}: ${needle}`);
    cursor = index;
  }
}

function fakeCargoPackage(name, relativeDir, dependencyDirs = [], extra = {}) {
  const root = "/fixture";
  const manifestPath = relativeDir === "." ? path.join(root, "Cargo.toml") : path.join(root, relativeDir, "Cargo.toml");
  return {
    name,
    version: "0.2.0",
    id: `path+file://${path.join(root, relativeDir)}#0.2.0`,
    manifest_path: manifestPath,
    dependencies: dependencyDirs.map((dependencyDir) => ({
      path: path.join(root, dependencyDir),
    })),
    publish: null,
    ...extra,
  };
}

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

test("cargoPublishOrderFromMetadata publishes internal crates before dependents and root last", () => {
  const packages = [
    fakeCargoPackage("prodex", ".", ["crates/prodex-app"]),
    fakeCargoPackage("prodex-app", "crates/prodex-app", ["crates/prodex-core", "crates/prodex-state"]),
    fakeCargoPackage("prodex-core", "crates/prodex-core"),
    fakeCargoPackage("prodex-state", "crates/prodex-state"),
    fakeCargoPackage("prodex-app-reports", "crates/prodex-app-reports", ["crates/prodex-state"]),
  ];
  const metadata = {
    workspace_root: "/fixture",
    workspace_members: packages.map((packageMetadata) => packageMetadata.id),
    packages,
  };

  assert.deepEqual(
    cargoPublishOrderFromMetadata(metadata).map((packageMetadata) => packageMetadata.name),
    ["prodex-core", "prodex-state", "prodex-app", "prodex-app-reports", "prodex"],
  );
});

test("cargoPublishOrderFromMetadata rejects publishable crates depending on private workspace crates", () => {
  const packages = [
    fakeCargoPackage("prodex", ".", ["crates/prodex-app"]),
    fakeCargoPackage("prodex-app", "crates/prodex-app", ["crates/prodex-private"]),
    fakeCargoPackage("prodex-private", "crates/prodex-private", [], { publish: false }),
  ];
  const metadata = {
    workspace_root: "/fixture",
    workspace_members: packages.map((packageMetadata) => packageMetadata.id),
    packages,
  };

  assert.throws(
    () => cargoPublishOrderFromMetadata(metadata),
    /prodex-app depends on non-publishable workspace package prodex-private/,
  );
});

test("cargo publish helper plans dry-run before publish without leaking token values", () => {
  assert.deepEqual(cargoPublishCommandArgs("prodex-core", { dryRun: true }), [
    "publish",
    "--locked",
    "-p",
    "prodex-core",
    "--dry-run",
  ]);

  assert.deepEqual(
    cargoPublishPlanLines([{ name: "prodex-core" }, { name: "prodex" }], "publish"),
    [
      "Cargo publish order (2 package(s)):",
      "1. prodex-core",
      "2. prodex",
      "Cargo publish commands:",
      "cargo publish --locked -p prodex-core --dry-run",
      "cargo publish --locked -p prodex-core",
      "cargo publish --locked -p prodex --dry-run",
      "cargo publish --locked -p prodex",
    ],
  );
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
  assert.throws(
    () => parseArgs(["node", "release-run.mjs", "--cargo-publish", "maybe"]),
    /--cargo-publish expects one of/,
  );
});

test("gh timeout classifier matches transient API failures only", () => {
  assert.equal(isGhTimeoutError("Post https://api.github.com: context deadline exceeded"), true);
  assert.equal(isGhTimeoutError("net/http: TLS handshake timeout"), true);
  assert.equal(isGhTimeoutError("HTTP 504 gateway timeout"), true);
  assert.equal(isGhTimeoutError("HTTP 401 bad credentials"), false);
  assert.equal(isGhTimeoutError("validation failed: ref is required"), false);
});

test("release-run dry-run covers mandatory release order without mutation or network", async () => {
  const root = await createReleaseRunFixture();
  const stateFile = path.join(root, "state.json");
  try {
    const { stdout, stderr } = await runReleaseRun(root, [
      "--dry-run",
      "--version",
      "0.2.0",
      "--state-file",
      stateFile,
      "--poll-seconds",
      "1",
      "--ci-timeout-minutes",
      "1",
      "--publish-timeout-minutes",
      "1",
    ]);

    assert.equal(stderr, "");
    assertIncludesInOrder(stdout, [
      "release-run: bump (0.2.0)",
      "dry-run: bump Cargo.toml 0.1.0 -> 0.2.0",
      "release-run: sync (0.2.0)",
      "dry-run: npm run npm:sync-version",
      "dry-run: npm run changelog -- --release-version 0.2.0",
      "dry-run: node scripts/npm/changelog.mjs --check --release-version 0.2.0",
      "dry-run: cargo update --workspace",
      "release-run: test (0.2.0)",
      "dry-run: npm run release:prepare -- --release-version 0.2.0",
      "release-run: commit (0.2.0)",
      "dry-run: git add -- Cargo.toml",
      "dry-run: git diff --cached --name-only",
      "dry-run: git commit -m chore(release): release 0.2.0",
      "dry-run: node scripts/ci/release-hygiene.mjs --commit HEAD --no-fixtures",
      "dry-run: node scripts/npm/changelog.mjs --check",
      "release-run: push (0.2.0)",
      "dry-run: git push origin HEAD:main",
      "release-run: watch-ci (0.2.0)",
      "dry-run: watch ci.yml for ",
      "release-run: trigger-publish (0.2.0)",
      "dry-run: gh api --method GET /repos/example/prodex/actions/workflows/npm-publish.yml/runs?branch=main&per_page=20",
      "dry-run: gh api --method POST /repos/example/prodex/actions/workflows/npm-publish.yml/dispatches -f ref=main",
      "trigger-publish: would dispatch npm-publish.yml for main (0.2.0)",
      "release-run: watch-publish (0.2.0)",
      "dry-run: watch npm-publish.yml for ",
      "release-run: verify (0.2.0)",
      "dry-run: gh api --method GET /repos/example/prodex/releases/tags/0.2.0",
    ]);
    assert.doesNotMatch(stdout, /npm view|npm publish/);

    assert.equal(await pathExists(stateFile), false);
    assert.match(await fs.readFile(path.join(root, "Cargo.toml"), "utf8"), /version = "0\.1\.0"/);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("release-run dry-run resume skips completed steps without changing state", async () => {
  const root = await createReleaseRunFixture();
  const stateFile = path.join(root, "state.json");
  const state = {
    version: "0.2.0",
    completed: {
      bump: "2026-05-11T00:00:00.000Z",
      sync: "2026-05-11T00:00:01.000Z",
      test: "2026-05-11T00:00:02.000Z",
    },
  };
  try {
    await fs.writeFile(stateFile, `${JSON.stringify(state, null, 2)}\n`);
    const beforeState = await fs.readFile(stateFile, "utf8");
    const { stdout, stderr } = await runReleaseRun(root, [
      "--dry-run",
      "--resume",
      "--state-file",
      stateFile,
    ]);

    assert.equal(stderr, "");
    assertIncludesInOrder(stdout, [
      "bump: already complete; skipping",
      "sync: already complete; skipping",
      "test: already complete; skipping",
      "release-run: commit (0.2.0)",
      "release-run: push (0.2.0)",
      "release-run: watch-ci (0.2.0)",
      "release-run: trigger-publish (0.2.0)",
      "release-run: watch-publish (0.2.0)",
      "release-run: verify (0.2.0)",
    ]);
    assert.doesNotMatch(stdout, /release-run: bump \(0\.2\.0\)/);
    assert.doesNotMatch(stdout, /dry-run: npm run changelog/);
    assert.equal(await fs.readFile(stateFile, "utf8"), beforeState);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
